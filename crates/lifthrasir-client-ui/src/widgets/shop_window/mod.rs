//! Single-use NPC shop drafts, cart controls and authoritative deal feedback.
//!
//! [`ShopSession`] is the only state. [`scene::window`] spawns the frame, the search
//! field and four empty [`ShopMount`]s when a shop opens, and [`rebuild_body`] refills
//! the mounts whenever the session, the bag or the player's zeny and weight change.
//! Each stock row keeps its typed quantity in its own [`ShopQtyField`], so stepping it
//! does not redraw the list.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_client::domain::inventory::Inventory;
use net_contract::commands::{BuyFromShop, CloseNpcShop, SellToShop};
use net_contract::dto::{BuyEntry, SellEntry, ShopBuyItem, ShopSellItem};
use net_contract::events::ShopOpened;

use crate::screens::shared::{light_field_focus, light_search_focus, set_field_text, typed_number};
use crate::widgets::info_modal::view::{inventory_item_view, shop_item_view};
use crate::widgets::info_modal::{InfoContent, ItemAction, ShowInfoModal};
use crate::widgets::npc_dialog::{ActiveNpcDialog, NpcDialogRoot};
use crate::widgets::shop_window::view::FooterView;

mod lifecycle;
pub mod scene;
mod view;

use lifecycle::{PendingShopDeal, ShopDeal, on_shop_result};

/// The item type of plain loot (rAthena `IT_ETC`), which leaves out cards and pet eggs.
const IT_ETC: u8 = 3;

/// Which tab of the shop window is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShopTab {
    #[default]
    Buy,
    Sell,
}

impl ShopTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::Sell => "Sell",
        }
    }
}

/// What a shop button does when activated. The keyed variants carry the active
/// tab's cart key (`nameid` on Buy, `inventory_index` on Sell), the same key space
/// `ShopSession`'s cart maps use.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopButtonAction {
    SwitchTab(ShopTab),
    /// Steps the row's quantity field down.
    RowDec(u32),
    /// Steps the row's quantity field up.
    RowInc(u32),
    /// Adds the row's typed quantity to the active cart.
    AddRow(u32),
    IncQty(u32),
    DecQty(u32),
    RemoveLine(u32),
    ClearCart,
    /// Puts every plain loot stack in the sell cart.
    AddLoot,
    /// Sends the active cart, unless a sale needs the warning first.
    #[default]
    Submit,
    /// "Sell Anyway": sends a sale the warning is up for.
    ConfirmTrade,
    CancelConfirm,
}

/// Window-root marker. A single instance exists for the lifetime of an open shop.
#[derive(Component, Default, Clone)]
pub struct ShopWindowRoot;

/// A region of the window that [`rebuild_body`] refills.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopMount {
    #[default]
    Tabs,
    List,
    Cart,
    Footer,
}

/// A scroll viewport whose offset survives a rebuild.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopScroll {
    #[default]
    List,
    Cart,
}

/// A stock row, carrying its cart key.
#[derive(Component, Default, Clone, Copy)]
pub struct ShopRow(pub u32);

/// The quantity field of the stock row with this cart key.
#[derive(Component, Default, Clone, Copy)]
pub struct ShopQtyField(pub u32);

#[derive(Component, Default, Clone)]
pub struct ShopSearchField;

/// The draggable titlebar; the drag observer only moves the window when the
/// drag's target is the titlebar itself, so dragging from the close button is
/// inert.
#[derive(Component, Default, Clone)]
pub struct ShopWindowTitlebar;

const FALLBACK_TITLE: &str = "Shop";

/// Single source of truth for an open shop: the server's buy/sell snapshots, the
/// two cart maps, the active tab and the search text. The window is a pure
/// projection of this resource.
#[derive(Resource, Clone, Debug)]
pub struct ShopSession {
    pub unit_id: u64,
    pub buy_items: Vec<ShopBuyItem>,
    pub sell_items: Vec<ShopSellItem>,
    pub tab: ShopTab,
    pub cart_buy: HashMap<u32, u32>,
    pub cart_sell: HashMap<u32, u32>,
    /// The trimmed search text.
    pub query: String,
    /// Whether the "Check before you sell" warning is showing. Editing a cart
    /// withdraws it.
    pub confirm_open: bool,
    /// Retirement latch prevents duplicate actions before deferred removal.
    pub(super) retired: bool,
}

impl ShopSession {
    /// Sum of `price(nameid) * qty` over every buy-cart line. A cart nameid not
    /// present in `buy_items` (should not happen) contributes 0.
    pub fn buy_subtotal(&self) -> u64 {
        self.cart_buy
            .iter()
            .map(|(nameid, qty)| {
                let price = self
                    .buy_items
                    .iter()
                    .find(|item| item.nameid == *nameid)
                    .map(|item| item.price)
                    .unwrap_or(0);
                price as u64 * *qty as u64
            })
            .sum()
    }

    /// Sum of `sell_price(inventory_index) * qty` over every sell-cart line.
    pub fn sell_subtotal(&self) -> u64 {
        self.cart_sell
            .iter()
            .map(|(index, qty)| {
                let price = self
                    .sell_items
                    .iter()
                    .find(|item| item.inventory_index == *index)
                    .map(|item| item.sell_price)
                    .unwrap_or(0);
                price as u64 * *qty as u64
            })
            .sum()
    }

    /// Whether the buy cart's subtotal fits within `zeny`. Only meaningful for
    /// the Buy tab; a sell cart is always affordable.
    pub fn can_afford(&self, zeny: u32) -> bool {
        self.buy_subtotal() <= zeny as u64
    }

    /// One `BuyEntry` per buy-cart line with qty > 0.
    pub fn to_buy_entries(&self) -> Vec<BuyEntry> {
        self.cart_buy
            .iter()
            .filter(|(_, qty)| **qty > 0)
            .map(|(nameid, qty)| BuyEntry {
                nameid: *nameid,
                amount: *qty,
            })
            .collect()
    }

    /// One `SellEntry` per sell-cart line with qty > 0.
    pub fn to_sell_entries(&self) -> Vec<SellEntry> {
        self.cart_sell
            .iter()
            .filter(|(_, qty)| **qty > 0)
            .map(|(index, qty)| SellEntry {
                inventory_index: *index,
                amount: *qty,
            })
            .collect()
    }

    /// The snapshot cap for a sell-cart line, or `u32::MAX` (no cap) for the
    /// buy tab.
    fn cap_for(&self, key: u32) -> u32 {
        match self.tab {
            ShopTab::Buy => u32::MAX,
            ShopTab::Sell => self
                .sell_items
                .iter()
                .find(|item| item.inventory_index == key)
                .map(|item| item.amount)
                .unwrap_or(0),
        }
    }

    /// The active tab's cart map, mutably. Every cart edit goes through here, which
    /// is what withdraws the sell warning.
    fn active_cart_mut(&mut self) -> &mut HashMap<u32, u32> {
        self.confirm_open = false;
        match self.tab {
            ShopTab::Buy => &mut self.cart_buy,
            ShopTab::Sell => &mut self.cart_sell,
        }
    }

    /// How many more of `key` a row may add: what is left of a Sell slot, unbounded
    /// on Buy.
    pub fn row_cap(&self, key: u32) -> u32 {
        let cart = match self.tab {
            ShopTab::Buy => &self.cart_buy,
            ShopTab::Sell => &self.cart_sell,
        };
        self.cap_for(key)
            .saturating_sub(cart.get(&key).copied().unwrap_or(0))
    }

    /// Adds a row's typed quantity to the active cart, at least one and at most
    /// what the row has left.
    pub fn add_row(&mut self, key: u32, typed: u32) {
        let qty = clamp_qty(typed, self.row_cap(key));
        self.add_to_cart(key, qty);
    }

    /// The Sell slots holding plain loot that is not wholly in the cart yet: `Etc`
    /// stacks that are neither favorites nor something this shop sells.
    ///
    /// NOTE: the client cannot tell refine materials (Elunium, Oridecon) from other
    /// loot, so they count. Skip them once item data carries that.
    pub fn loot<'a>(&'a self, inventory: &'a Inventory) -> impl Iterator<Item = &'a ShopSellItem> {
        self.sell_items.iter().filter(move |slot| {
            let in_cart = self.cart_sell.get(&slot.inventory_index).copied();
            let plain = inventory
                .get(slot.inventory_index as u16)
                .is_some_and(|item| item.item_type == IT_ETC && !item.favorite);
            let stocked = self
                .buy_items
                .iter()
                .any(|stock| stock.nameid == slot.nameid);
            plain && !stocked && in_cart.unwrap_or(0) < slot.amount
        })
    }

    /// Puts every [`loot`](Self::loot) stack, whole, in the sell cart.
    pub fn add_loot(&mut self, inventory: &Inventory) {
        let stacks: Vec<(u32, u32)> = self
            .loot(inventory)
            .map(|slot| (slot.inventory_index, slot.amount))
            .collect();
        self.confirm_open = false;
        self.cart_sell.extend(stacks);
    }

    /// Adds `qty` to the active tab's cart line for `key`. The Sell tab caps
    /// the resulting quantity at the slot's snapshot `amount`; the Buy tab has
    /// no cap.
    pub fn add_to_cart(&mut self, key: u32, qty: u32) {
        let cap = self.cap_for(key);
        let entry = self.active_cart_mut().entry(key).or_insert(0);
        *entry = entry.saturating_add(qty).min(cap);
    }

    /// Sets the active tab's cart line for `key` to `qty` directly (0 is
    /// allowed here; use `remove_line` to drop a line entirely). The Sell tab
    /// caps at the slot's snapshot `amount`.
    pub fn set_line_qty(&mut self, key: u32, qty: u32) {
        let cap = self.cap_for(key);
        self.active_cart_mut().insert(key, qty.min(cap));
    }

    /// Removes `key` from the active tab's cart.
    pub fn remove_line(&mut self, key: u32) {
        self.active_cart_mut().remove(&key);
    }

    /// Decrements the active tab's cart line for `key` by 1, removing the line
    /// entirely rather than leaving a zero-qty line behind once it would hit 0.
    pub fn dec_line(&mut self, key: u32) {
        let current = self.active_cart_mut().get(&key).copied().unwrap_or(0);
        if current <= 1 {
            self.remove_line(key);
        } else {
            self.set_line_qty(key, current - 1);
        }
    }

    /// Empties only the active tab's cart, leaving the other tab intact.
    pub fn clear_active_cart(&mut self) {
        self.active_cart_mut().clear();
    }
}

pub struct ShopWindowPlugin;

impl Plugin for ShopWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingShopDeal>();
        app.add_systems(
            Update,
            (
                on_shop_result,
                on_shop_opened,
                close_shop.run_if(resource_exists::<ShopSession>),
                sync_search.run_if(resource_exists::<ShopSession>),
                rebuild_body.run_if(resource_exists::<ShopSession>),
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            (
                light_search_focus::<ShopSearchField>,
                light_field_focus::<ShopQtyField>,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            OnExit(GameState::InGame),
            |mut commands: Commands, mut pending: ResMut<PendingShopDeal>| {
                commands.remove_resource::<ShopSession>();
                pending.0 = None;
            },
        );
    }
}

/// The resolved NPC display name, falling back to `"Shop"` when the shop unit
/// hasn't been named yet.
fn title_or_fallback(name: Option<String>) -> String {
    name.unwrap_or_else(|| FALLBACK_TITLE.to_string())
}

/// Consumes the latest [`ShopOpened`]: force-closes any active NPC dialog (RO's
/// `callshop` semantics — dialogue and shop windows never coexist), despawns any
/// existing shop window (a re-talk is idempotent), spawns the chrome, and inserts
/// a fresh [`ShopSession`].
fn on_shop_opened(
    mut events: MessageReader<ShopOpened>,
    mut commands: Commands,
    dialog_roots: Query<Entity, With<NpcDialogRoot>>,
    shop_roots: Query<Entity, With<ShopWindowRoot>>,
    registry: Res<EntityRegistry>,
    names: Query<&EntityName>,
) {
    let Some(event) = events.read().last() else {
        return;
    };

    if let Ok(dialog_root) = dialog_roots.single() {
        commands.entity(dialog_root).despawn();
        commands.remove_resource::<ActiveNpcDialog>();
    }
    if let Ok(shop_root) = shop_roots.single() {
        commands.entity(shop_root).despawn();
    }

    // Gids occupy a 32-bit space regardless of the wire's uint64 encoding; a
    // miss just falls back to the "Shop" title.
    let name = registry
        .get_entity(event.unit_id as u32)
        .and_then(|entity| names.get(entity).ok())
        .map(|entity_name| entity_name.name.clone());
    let title = title_or_fallback(name);

    commands
        .spawn_scene(scene::window(title))
        .insert(DespawnOnExit(GameState::InGame));

    commands.insert_resource(ShopSession {
        unit_id: event.unit_id,
        buy_items: event.buy_items.clone(),
        sell_items: event.sell_items.clone(),
        tab: ShopTab::default(),
        cart_buy: HashMap::new(),
        cart_sell: HashMap::new(),
        query: String::new(),
        confirm_open: false,
        retired: false,
    });
}

/// Mirrors the search field into [`ShopSession::query`].
fn sync_search(
    fields: Query<&EditableText, With<ShopSearchField>>,
    mut session: ResMut<ShopSession>,
) {
    let Ok(text) = fields.single() else {
        return;
    };
    let value = text.value().to_string();
    if session.query != value.trim() {
        session.query = value.trim().to_string();
    }
}

/// The bag, the item catalogue and the player's status, as the shop reads them.
#[derive(SystemParam)]
pub(super) struct ShopData<'w, 's> {
    inventory: Res<'w, Inventory>,
    item_db: Option<Res<'w, ItemDb>>,
    player: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
}

/// What [`rebuild_body`] last drew, to tell what changed.
#[derive(Default)]
struct Shown {
    tab: ShopTab,
    query: String,
    footer: FooterView,
}

/// Refills every [`ShopMount`] when the session, the pending deal, the bag or the
/// shown zeny and weight change, and once when the window is new. `CharacterStatus`
/// changes on every HP tick, so its shown values are compared instead of trusting
/// change detection. The list and the cart keep their scroll offsets unless the tab
/// or the search changed, so adding a row deep in the list does not jump to the top.
///
/// A missing local player is not papered over with a zero-zeny fallback:
/// `LocalPlayer`/`CharacterStatus` exist before any NPC talk, so their absence is a
/// real bug and is logged loudly.
fn rebuild_body(
    mut commands: Commands,
    session: Res<ShopSession>,
    pending: Res<PendingShopDeal>,
    data: ShopData,
    mounts: Query<(Entity, Ref<ShopMount>, Option<&Children>)>,
    scrolls: Query<(&ShopScroll, &ScrollPosition)>,
    mut shown: Local<Shown>,
) {
    let dirty = session.is_changed()
        || pending.is_changed()
        || data.inventory.is_changed()
        || mounts.iter().any(|(_, mount, _)| mount.is_added());
    let Ok(status) = data.player.single() else {
        if dirty {
            warn!(
                "shop window rebuild skipped: no LocalPlayer/CharacterStatus while a shop is open"
            );
        }
        return;
    };
    let footer = FooterView {
        zeny: status.zeny,
        weight: status.weight,
        max_weight: status.max_weight,
        bag: data.inventory.len(),
    };
    if !dirty && shown.footer == footer {
        return;
    }
    let same_tab = shown.tab == session.tab;
    let same_list = same_tab && shown.query == session.query;
    let scroll_of = |which: ShopScroll, keep: bool| {
        scrolls
            .iter()
            .find(|(scroll, _)| keep && **scroll == which)
            .map(|(_, position)| position.0)
            .unwrap_or_default()
    };
    let (list_scroll, cart_scroll) = (
        scroll_of(ShopScroll::List, same_list),
        scroll_of(ShopScroll::Cart, same_tab),
    );
    *shown = Shown {
        tab: session.tab,
        query: session.query.clone(),
        footer,
    };

    let item_db = data.item_db.as_deref();
    for (mount, kind, children) in &mounts {
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        let mut content = match *kind {
            ShopMount::Tabs => commands.spawn_scene(scene::tabs(view::tab_views(&session))),
            ShopMount::List => commands.spawn_scene(scene::list(view::list_view(
                &session,
                &data.inventory,
                item_db,
                list_scroll,
            ))),
            ShopMount::Cart => commands.spawn_scene(scene::cart(view::cart_view(
                &session,
                status.zeny,
                &data.inventory,
                item_db,
                pending.0.is_some(),
                cart_scroll,
            ))),
            ShopMount::Footer => commands.spawn_scene(scene::footer(footer)),
        };
        content.insert(ChildOf(mount));
    }
}

/// Retire synchronously before queuing removal, so duplicate activations cannot send twice.
fn close_shop_window(
    commands: &mut Commands,
    roots: &Query<Entity, With<ShopWindowRoot>>,
    session: &mut ShopSession,
) -> bool {
    if session.retired {
        return false;
    }
    session.retired = true;
    for root in roots {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<ShopSession>();
    true
}

/// ESC ends the shop exactly like the titlebar close button: despawn the window
/// and clear `ShopSession`. Gated on `ShopSession` existing, so it only fires
/// while a shop is open; `settings_window`'s own Escape toggle is separately
/// gated to skip while this resource is present (mirrors `npc_dialog`).
fn close_shop(
    keys: Res<ButtonInput<KeyCode>>,
    roots: Query<Entity, With<ShopWindowRoot>>,
    mut session: Option<ResMut<ShopSession>>,
    mut close: MessageWriter<CloseNpcShop>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let Some(session) = session.as_deref_mut() else {
        return;
    };
    if close_shop_window(&mut commands, &roots, session) {
        close.write(CloseNpcShop {
            unit_id: session.unit_id,
        });
    }
}

/// Titlebar close button: same effect as Escape.
pub(super) fn on_shop_close_button(
    _: On<Activate>,
    roots: Query<Entity, With<ShopWindowRoot>>,
    mut session: Option<ResMut<ShopSession>>,
    mut close: MessageWriter<CloseNpcShop>,
    mut commands: Commands,
) {
    let Some(session) = session.as_deref_mut() else {
        return;
    };
    if close_shop_window(&mut commands, &roots, session) {
        close.write(CloseNpcShop {
            unit_id: session.unit_id,
        });
    }
}

/// Clamps a row's quantity into `[1, cap]`. `cap` is unbounded on the Buy tab and
/// what is left of the slot on the Sell tab.
fn clamp_qty(qty: u32, cap: u32) -> u32 {
    qty.clamp(1, cap.max(1))
}

/// The quantity typed in a row's field. An empty field counts as one.
fn typed_qty(field: &EditableText) -> u32 {
    typed_number(field).unwrap_or(1)
}

/// Steps the quantity field of row `key` by `step`, within `[1, cap]`.
fn step_row(fields: &mut Query<(&ShopQtyField, &mut EditableText)>, key: u32, step: i32, cap: u32) {
    let Some((_, mut field)) = fields.iter_mut().find(|(row, _)| row.0 == key) else {
        return;
    };
    let qty = clamp_qty(typed_qty(&field).saturating_add_signed(step), cap);
    set_field_text(&mut field, &qty.to_string());
}

#[derive(SystemParam)]
pub(super) struct ShopRequests<'w, 's> {
    pending: ResMut<'w, PendingShopDeal>,
    buy: MessageWriter<'w, BuyFromShop>,
    sell: MessageWriter<'w, SellToShop>,
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, Entity, With<ShopWindowRoot>>,
}

/// Sends the active cart as one deal, records it as pending and closes the window.
fn send_deal(session: &mut ShopSession, requests: &mut ShopRequests) {
    let unit_id = session.unit_id;
    let buy = session.tab == ShopTab::Buy;
    let lines = match session.tab {
        ShopTab::Buy => {
            let items = session.to_buy_entries();
            let lines = items
                .iter()
                .map(|entry| (entry.nameid, entry.amount))
                .collect();
            requests.buy.write(BuyFromShop { unit_id, items });
            lines
        }
        ShopTab::Sell => {
            let items = session.to_sell_entries();
            let lines = items
                .iter()
                .filter_map(|entry| {
                    session
                        .sell_items
                        .iter()
                        .find(|item| item.inventory_index == entry.inventory_index)
                        .map(|item| (item.nameid, entry.amount))
                })
                .collect();
            requests.sell.write(SellToShop { unit_id, items });
            lines
        }
    };
    requests.pending.0 = Some(ShopDeal {
        unit_id,
        buy,
        lines,
    });
    close_shop_window(&mut requests.commands, &requests.roots, session);
}

/// Shared handler for every shop button. The row stepper edits its row's
/// [`ShopQtyField`] and leaves `ShopSession` unchanged, so the list is not redrawn
/// under the pointer. `AddRow` and the cart controls edit the active cart. `Submit`
/// sends a purchase at once and a sale unless [`view::sell_warnings`] has something
/// to say, in which case it raises the warning and `ConfirmTrade` sends the sale.
/// Both re-check [`view::cta_enabled`], since a click can race the redraw that
/// disables the button. A sent deal closes the window, and a replacement draft
/// cannot submit until that deal settles.
///
/// `bevy_ui_widgets`' `Button` fires `Activate` on release of *any* pointer
/// button (`button_on_pointer_click` has no button check), so a right-click on a
/// row's button, meant only to open the info modal, reaches this handler too. The
/// pointer's own release always lands in the same frame as the `Activate` it
/// triggers, so gating on that frame's `ButtonInput<MouseButton>` is deterministic.
pub(super) fn on_shop_button(
    activate: On<Activate>,
    actions: Query<&ShopButtonAction>,
    session: Option<ResMut<ShopSession>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut fields: Query<(&ShopQtyField, &mut EditableText)>,
    data: ShopData,
    mut requests: ShopRequests,
) {
    if mouse.just_released(MouseButton::Right) {
        return;
    }
    let (Ok(action), Some(mut session)) = (actions.get(activate.entity), session) else {
        return;
    };
    let sends = matches!(
        action,
        ShopButtonAction::Submit | ShopButtonAction::ConfirmTrade
    );
    if session.retired || (sends && requests.pending.0.is_some()) {
        return;
    }
    match *action {
        ShopButtonAction::RowDec(key) => step_row(&mut fields, key, -1, session.row_cap(key)),
        ShopButtonAction::RowInc(key) => step_row(&mut fields, key, 1, session.row_cap(key)),
        ShopButtonAction::SwitchTab(tab) => {
            session.tab = tab;
            session.confirm_open = false;
        }
        ShopButtonAction::AddRow(key) => {
            let typed = fields
                .iter()
                .find(|(row, _)| row.0 == key)
                .map_or(1, |(_, field)| typed_qty(field));
            session.add_row(key, typed);
        }
        ShopButtonAction::IncQty(key) => session.add_to_cart(key, 1),
        ShopButtonAction::DecQty(key) => session.dec_line(key),
        ShopButtonAction::RemoveLine(key) => session.remove_line(key),
        ShopButtonAction::ClearCart => session.clear_active_cart(),
        ShopButtonAction::AddLoot => {
            if session.tab == ShopTab::Sell {
                session.add_loot(&data.inventory);
            }
        }
        ShopButtonAction::CancelConfirm => session.confirm_open = false,
        ShopButtonAction::Submit | ShopButtonAction::ConfirmTrade => {
            let Ok(status) = data.player.single() else {
                warn!("shop deal requested with no LocalPlayer/CharacterStatus present");
                return;
            };
            if !view::cta_enabled(&session, status.zeny) {
                return;
            }
            let confirmed = *action == ShopButtonAction::ConfirmTrade;
            if confirmed && !session.confirm_open {
                return;
            }
            let risky = session.tab == ShopTab::Sell
                && !view::sell_warnings(&session, &data.inventory, data.item_db.as_deref())
                    .is_empty();
            if risky && !confirmed {
                session.confirm_open = true;
                return;
            }
            send_deal(&mut session, &mut requests);
        }
    }
}

/// Secondary-click on a stock row opens the info modal for its item: a Buy row
/// shows the stock item with its price, a Sell row shows the bag item (the sell
/// snapshot mirrors a live bag slot).
pub(super) fn on_shop_row_secondary_click(
    click: On<PointerClick>,
    rows: Query<&ShopRow>,
    session: Option<Res<ShopSession>>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    option_db: Option<Res<RandomOptionDb>>,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }
    let (Ok(row), Some(session)) = (rows.get(click.entity), session) else {
        return;
    };
    let Some(item_db) = item_db.as_deref() else {
        warn!("shop: ItemDb not loaded yet, ignoring inspect");
        return;
    };
    let content = match session.tab {
        ShopTab::Buy => {
            let price = session
                .buy_items
                .iter()
                .find(|stock| stock.nameid == row.0)
                .map(|stock| stock.price);
            InfoContent::Item {
                view: shop_item_view(row.0, price, item_db),
                action: None,
            }
        }
        ShopTab::Sell => {
            let Some(item) = inventory.get(row.0 as u16) else {
                return;
            };
            InfoContent::Item {
                view: inventory_item_view(item, item_db, option_db.as_deref()),
                action: ItemAction::for_bag_item(item),
            }
        }
    };
    info_writer.write(ShowInfoModal { content });
}

/// Resolves a display name for `nameid` via `ItemDb`, always as identified, since
/// the trade summary is a concise confirmation line. Falls back to `#nameid` when
/// the db or the entry is missing.
fn resolved_name(item_db: Option<&ItemDb>, nameid: u32) -> String {
    item_db
        .and_then(|db| db.name(nameid, true))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{nameid}"))
}

/// Builds the chat-line summary for a successful trade, e.g. "Purchased 30x Red
/// Potion, 3x White Potion" / "Sold 200x Jellopy". `lines` is `(nameid, qty)`
/// pairs; empty input still yields a verb-only sentence, which should not occur
/// in practice since a deal is only sent for a non-empty cart.
fn trade_summary(buy: bool, lines: &[(u32, u32)], item_db: Option<&ItemDb>) -> String {
    let verb = if buy { "Purchased" } else { "Sold" };
    let parts: Vec<String> = lines
        .iter()
        .map(|(nameid, qty)| format!("{qty}x {}", resolved_name(item_db, *nameid)))
        .collect();
    format!("{verb} {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::{
        dto::ShopResult,
        events::{ChatNotice, ShopBuyResulted, ShopSellResulted},
    };

    fn session(tab: ShopTab) -> ShopSession {
        ShopSession {
            unit_id: 1,
            buy_items: vec![
                ShopBuyItem {
                    nameid: 501,
                    price: 10,
                },
                ShopBuyItem {
                    nameid: 502,
                    price: 25,
                },
            ],
            sell_items: vec![
                ShopSellItem {
                    inventory_index: 0,
                    nameid: 501,
                    amount: 5,
                    sell_price: 4,
                },
                ShopSellItem {
                    inventory_index: 1,
                    nameid: 502,
                    amount: 2,
                    sell_price: 12,
                },
            ],
            tab,
            cart_buy: HashMap::new(),
            cart_sell: HashMap::new(),
            query: String::new(),
            confirm_open: false,
            retired: false,
        }
    }

    #[test]
    fn escape_closes_the_server_shop_exactly_once() {
        let mut app = App::new();
        app.insert_resource(session(ShopTab::Buy));
        app.add_message::<net_contract::commands::CloseNpcShop>();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        app.insert_resource(keys);
        app.add_systems(Update, close_shop);
        let root = app.world_mut().spawn(ShopWindowRoot).id();
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(!app.world().contains_resource::<ShopSession>());
        let requests: Vec<_> = app
            .world()
            .resource::<Messages<net_contract::commands::CloseNpcShop>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].unit_id, 1);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<net_contract::commands::CloseNpcShop>>()
                .len(),
            1
        );
    }

    #[test]
    fn buy_subtotal_sums_multiple_lines() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.cart_buy.insert(502, 2);
        assert_eq!(s.buy_subtotal(), 10 * 3 + 25 * 2);
    }

    #[test]
    fn sell_subtotal_sums_multiple_lines() {
        let mut s = session(ShopTab::Sell);
        s.cart_sell.insert(0, 5);
        s.cart_sell.insert(1, 2);
        assert_eq!(s.sell_subtotal(), 4 * 5 + 12 * 2);
    }

    #[test]
    fn can_afford_exact_boundary() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        let subtotal = s.buy_subtotal();
        assert!(s.can_afford(subtotal as u32));
        assert!(!s.can_afford(subtotal as u32 - 1));
    }

    #[test]
    fn add_to_cart_on_sell_caps_at_snapshot_amount() {
        let mut s = session(ShopTab::Sell);
        s.add_to_cart(0, 100);
        assert_eq!(s.cart_sell.get(&0), Some(&5));
    }

    #[test]
    fn add_to_cart_on_buy_does_not_cap() {
        let mut s = session(ShopTab::Buy);
        s.add_to_cart(501, 100);
        assert_eq!(s.cart_buy.get(&501), Some(&100));
    }

    #[test]
    fn set_line_qty_and_remove_line() {
        let mut s = session(ShopTab::Buy);
        s.set_line_qty(501, 7);
        assert_eq!(s.cart_buy.get(&501), Some(&7));
        s.remove_line(501);
        assert_eq!(s.cart_buy.get(&501), None);
    }

    #[test]
    fn set_line_qty_on_sell_caps_at_snapshot_amount() {
        let mut s = session(ShopTab::Sell);
        s.set_line_qty(0, 100);
        assert_eq!(s.cart_sell.get(&0), Some(&5));
    }

    #[test]
    fn remove_line_only_touches_active_tab() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.cart_sell.insert(0, 2);
        s.remove_line(501);
        assert_eq!(s.cart_buy.get(&501), None);
        assert_eq!(s.cart_sell.get(&0), Some(&2));
    }

    #[test]
    fn to_buy_entries_skips_zero_qty_lines() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.cart_buy.insert(502, 0);
        let mut entries = s.to_buy_entries();
        entries.sort_by_key(|e| e.nameid);
        assert_eq!(
            entries,
            vec![BuyEntry {
                nameid: 501,
                amount: 3
            }]
        );
    }

    #[test]
    fn to_sell_entries_skips_zero_qty_lines() {
        let mut s = session(ShopTab::Sell);
        s.cart_sell.insert(0, 5);
        s.cart_sell.insert(1, 0);
        let mut entries = s.to_sell_entries();
        entries.sort_by_key(|e| e.inventory_index);
        assert_eq!(
            entries,
            vec![SellEntry {
                inventory_index: 0,
                amount: 5
            }]
        );
    }

    #[test]
    fn title_or_fallback_uses_resolved_name() {
        assert_eq!(
            title_or_fallback(Some("Bennit Bard".to_string())),
            "Bennit Bard"
        );
    }

    #[test]
    fn title_or_fallback_defaults_when_unresolved() {
        assert_eq!(title_or_fallback(None), FALLBACK_TITLE);
    }

    #[test]
    fn clear_active_cart_empties_only_active_tab() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.cart_sell.insert(0, 2);
        s.clear_active_cart();
        assert!(s.cart_buy.is_empty());
        assert_eq!(s.cart_sell.get(&0), Some(&2));
    }

    #[test]
    fn dec_line_decrements_above_one() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.dec_line(501);
        assert_eq!(s.cart_buy.get(&501), Some(&2));
    }

    #[test]
    fn dec_line_removes_line_at_one() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 1);
        s.dec_line(501);
        assert_eq!(s.cart_buy.get(&501), None);
    }

    #[test]
    fn dec_line_on_absent_key_is_a_no_op() {
        let mut s = session(ShopTab::Buy);
        s.dec_line(999);
        assert_eq!(s.cart_buy.get(&999), None);
    }

    #[test]
    fn resolved_name_falls_back_to_nameid_without_db() {
        assert_eq!(resolved_name(None, 501), "#501");
    }

    #[test]
    fn trade_summary_formats_buy_lines() {
        assert_eq!(
            trade_summary(true, &[(501, 30), (502, 3)], None),
            "Purchased 30x #501, 3x #502"
        );
    }

    #[test]
    fn trade_summary_formats_sell_lines() {
        assert_eq!(trade_summary(false, &[(501, 200)], None), "Sold 200x #501");
    }

    #[test]
    fn pending_result_reports_after_close_without_touching_a_replacement() {
        for result in [ShopResult::Ok, ShopResult::NotEnoughZeny] {
            let mut app = App::new();
            let mut replacement = session(ShopTab::Sell);
            replacement.unit_id = 9;
            replacement.cart_buy.insert(502, 2);
            replacement.cart_sell.insert(1, 1);
            app.insert_resource(replacement);
            app.insert_resource(PendingShopDeal(Some(ShopDeal {
                unit_id: 1,
                buy: true,
                lines: vec![(501, 3)],
            })));
            app.add_message::<ShopBuyResulted>()
                .add_message::<ShopSellResulted>()
                .add_message::<ChatNotice>()
                .add_systems(Update, on_shop_result);
            app.world_mut().write_message(ShopSellResulted {
                result: ShopResult::Ok,
            });
            app.update();
            assert!(app.world().resource::<PendingShopDeal>().0.is_some());
            app.world_mut().write_message(ShopBuyResulted { result });
            app.update();
            let draft = app.world().resource::<ShopSession>();
            assert_eq!(draft.unit_id, 9);
            assert_eq!(draft.cart_buy.get(&502), Some(&2));
            assert_eq!(draft.cart_sell.get(&1), Some(&1));
            assert!(!draft.retired);
            assert!(!draft.confirm_open);
            assert!(app.world().resource::<PendingShopDeal>().0.is_none());
            let lines: Vec<_> = app
                .world()
                .resource::<Messages<ChatNotice>>()
                .iter_current_update_messages()
                .collect();
            assert_eq!(lines.len(), 1);
            if result == ShopResult::Ok {
                assert_eq!(lines[0].message, "Purchased 3x #501");
            }
            app.world_mut().remove_resource::<ShopSession>();
            app.world_mut().write_message(ShopBuyResulted { result });
            app.update();
            assert_eq!(app.world().resource::<Messages<ChatNotice>>().len(), 1);
        }
    }

    #[test]
    fn row_cap_is_what_the_cart_has_not_taken_from_a_sell_slot() {
        let mut s = session(ShopTab::Sell);
        assert_eq!(s.row_cap(0), 5);
        s.add_row(0, 2);
        assert_eq!(s.row_cap(0), 3);
        s.add_row(0, 99);
        assert_eq!(s.cart_sell.get(&0), Some(&5));
        assert_eq!(s.row_cap(0), 0);
    }

    #[test]
    fn editing_a_cart_withdraws_the_sell_warning() {
        let mut s = session(ShopTab::Sell);
        s.cart_sell.insert(0, 1);
        s.confirm_open = true;
        s.dec_line(0);
        assert!(!s.confirm_open);
    }

    #[test]
    fn loot_is_plain_etc_stacks_the_shop_does_not_sell() {
        use lifthrasir_client::domain::inventory::Item;

        let mut s = session(ShopTab::Sell);
        // Slots 0 and 1 hold 501 and 502, which this shop sells.
        s.sell_items.extend([2, 3, 4].map(|index| ShopSellItem {
            inventory_index: index,
            nameid: 900 + index,
            amount: 10,
            sell_price: 3,
        }));
        let mut inventory = Inventory::default();
        for (index, item_type, favorite) in [
            (0, IT_ETC, false),
            (2, IT_ETC, false),
            (3, 6, false),
            (4, IT_ETC, true),
        ] {
            inventory.upsert(Item {
                index,
                item_type,
                favorite,
                ..Default::default()
            });
        }

        s.add_loot(&inventory);

        assert_eq!(s.cart_sell, HashMap::from([(2, 10)]));
        assert_eq!(s.loot(&inventory).count(), 0);
    }

    #[test]
    fn clamp_qty_floors_at_one() {
        assert_eq!(clamp_qty(0, 100), 1);
    }

    #[test]
    fn clamp_qty_caps_at_provided_cap() {
        assert_eq!(clamp_qty(50, 5), 5);
    }

    #[test]
    fn clamp_qty_passes_through_in_range_values() {
        assert_eq!(clamp_qty(3, 10), 3);
    }

    #[test]
    fn clamp_qty_treats_zero_cap_as_one() {
        assert_eq!(clamp_qty(5, 0), 1);
    }

    /// Regression: the `ShopSession`-driven systems must short-circuit when the
    /// resource is absent (the normal, shop-closed state). Before the fix,
    /// `rebuild_body`'s `resource_changed::<ShopSession>` run condition read
    /// `Res<ShopSession>` and panicked on the missing resource the moment the
    /// game entered `InGame` without a shop open; the `resource_exists`-based
    /// guards must skip cleanly instead.
    #[test]
    fn shop_systems_skip_cleanly_without_session() {
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_message::<ShopBuyResulted>();
        app.add_message::<ShopSellResulted>();
        app.add_message::<ChatNotice>();
        app.add_systems(
            Update,
            rebuild_body.run_if(
                in_state(GameState::InGame).and_then(resource_exists_and_changed::<ShopSession>),
            ),
        );
        app.add_systems(
            Update,
            on_shop_result
                .run_if(in_state(GameState::InGame).and_then(resource_exists::<ShopSession>)),
        );
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);

        // No `ShopSession` inserted: two updates in `InGame` must not panic.
        app.update();
        app.update();
    }

    fn click_event(target: Entity, window: Entity, button: PointerButton) -> PointerClick {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        PointerClick {
            entity: target,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Primary.normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
            ),
            button,
            hit: HitData::new(target, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        }
    }

    fn row_click_app(tab: ShopTab) -> App {
        let mut app = App::new();
        app.insert_resource(session(tab));
        app.add_message::<ShowInfoModal>();
        app.init_resource::<ItemDb>();
        app.init_resource::<Inventory>();
        app.world_mut().resource_mut::<Inventory>().upsert(
            lifthrasir_client::domain::inventory::Item {
                index: 3,
                item_id: 501,
                identified: true,
                ..lifthrasir_client::domain::inventory::Item::default()
            },
        );
        app
    }

    fn contents(app: &App) -> Vec<InfoContent> {
        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        reader.read(messages).map(|m| m.content.clone()).collect()
    }

    #[test]
    fn secondary_click_on_a_buy_row_opens_the_info_modal_for_shop_buy() {
        let mut app = row_click_app(ShopTab::Buy);
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopRow(501))
            .observe(on_shop_row_secondary_click)
            .id();

        app.world_mut()
            .trigger(click_event(row, window, PointerButton::Secondary));

        let contents = contents(&app);
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: None }
                if view.item_id == 501 && view.meta == [("Price".to_string(), "10z".to_string())]
        ));
    }

    #[test]
    fn secondary_click_on_a_sell_row_opens_the_info_modal_for_inventory() {
        let mut app = row_click_app(ShopTab::Sell);
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopRow(3))
            .observe(on_shop_row_secondary_click)
            .id();

        app.world_mut()
            .trigger(click_event(row, window, PointerButton::Secondary));

        let contents = contents(&app);
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: Some(ItemAction { index: 3, .. }) }
                if view.item_id == 501
        ));
    }

    #[test]
    fn secondary_click_off_a_row_writes_nothing() {
        let mut app = row_click_app(ShopTab::Buy);
        let window = app.world_mut().spawn_empty().id();
        let button = app
            .world_mut()
            .spawn(ShopButtonAction::SwitchTab(ShopTab::Sell))
            .observe(on_shop_row_secondary_click)
            .id();

        app.world_mut()
            .trigger(click_event(button, window, PointerButton::Secondary));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn primary_click_on_a_row_does_not_open_the_info_modal() {
        let mut app = row_click_app(ShopTab::Buy);
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopRow(501))
            .observe(on_shop_row_secondary_click)
            .id();

        app.world_mut()
            .trigger(click_event(row, window, PointerButton::Primary));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }

    fn press_event(target: Entity, window: Entity, button: PointerButton) -> PointerPress {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        PointerPress {
            entity: target,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Primary.normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
            ),
            button,
            hit: HitData::new(target, 0.0, None, None),
            count: 1,
        }
    }

    /// Drives a real press-then-click through `bevy_ui_widgets`' actual `ButtonPlugin`
    /// observers (not a hand-rolled `Activate` trigger), and updates
    /// `ButtonInput<MouseButton>` the way that plugin's own frame would: the mouse
    /// button that drove the click is pressed then released before the click fires,
    /// mirroring `on_shop_button`'s guard precondition.
    fn real_click(
        app: &mut App,
        row: Entity,
        window: Entity,
        pointer: PointerButton,
        mouse: MouseButton,
    ) {
        app.world_mut().trigger(press_event(row, window, pointer));
        app.world_mut().flush();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(mouse);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(mouse);
        app.world_mut().trigger(click_event(row, window, pointer));
        app.world_mut().flush();
    }

    fn real_button_pipeline_app(session: ShopSession) -> App {
        use bevy::ui_widgets::ButtonPlugin;

        let mut app = App::new();
        app.add_plugins(ButtonPlugin);
        app.init_resource::<PendingShopDeal>();
        app.add_message::<ShowInfoModal>();
        app.add_message::<BuyFromShop>();
        app.add_message::<SellToShop>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.init_resource::<ItemDb>();
        app.init_resource::<Inventory>();
        app.world_mut().resource_mut::<Inventory>().upsert(
            lifthrasir_client::domain::inventory::Item {
                index: 0,
                item_id: 501,
                identified: true,
                ..Default::default()
            },
        );
        app.insert_resource(session);
        app
    }

    #[test]
    fn submitting_a_deal_closes_its_window_and_blocks_resubmission() {
        let mut draft = session(ShopTab::Buy);
        draft.cart_buy.insert(501, 3);
        draft.confirm_open = true;
        let mut app = real_button_pipeline_app(draft);
        app.world_mut().spawn((
            LocalPlayer,
            CharacterStatus {
                zeny: 100,
                ..default()
            },
        ));
        let root = app.world_mut().spawn(ShopWindowRoot).id();
        let button = app
            .world_mut()
            .spawn(ShopButtonAction::ConfirmTrade)
            .observe(on_shop_button)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
        assert!(!app.world().contains_resource::<ShopSession>());
        assert!(app.world().get_entity(root).is_err());
        assert_eq!(app.world().resource::<Messages<BuyFromShop>>().len(), 1);
        let pending = app
            .world()
            .resource::<PendingShopDeal>()
            .0
            .as_ref()
            .unwrap();
        assert_eq!(pending.unit_id, 1);
        assert_eq!(pending.lines, [(501, 3)]);
        let mut replacement = session(ShopTab::Buy);
        replacement.cart_buy.insert(502, 1);
        replacement.confirm_open = true;
        app.world_mut().insert_resource(replacement);
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
        assert_eq!(app.world().resource::<Messages<BuyFromShop>>().len(), 1);
    }

    /// A row's Add button with its quantity field holding `typed`.
    fn add_button(app: &mut App, key: u32, typed: &str) -> Entity {
        use bevy::ui_widgets::Button;

        app.world_mut()
            .spawn((ShopQtyField(key), EditableText::new(typed)));
        app.world_mut()
            .spawn((Button, ShopButtonAction::AddRow(key)))
            .observe(on_shop_button)
            .id()
    }

    fn press(app: &mut App, action: ShopButtonAction) {
        let button = app.world_mut().spawn(action).observe(on_shop_button).id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
    }

    fn player_with(app: &mut App, zeny: u32) {
        app.world_mut()
            .spawn((LocalPlayer, CharacterStatus { zeny, ..default() }));
    }

    #[test]
    fn a_right_click_on_a_row_button_does_not_activate_it() {
        let mut app = real_button_pipeline_app(session(ShopTab::Sell));
        let window = app.world_mut().spawn_empty().id();
        let add = add_button(&mut app, 0, "2");

        real_click(
            &mut app,
            add,
            window,
            PointerButton::Secondary,
            MouseButton::Right,
        );
        assert!(app.world().resource::<ShopSession>().cart_sell.is_empty());

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        real_click(
            &mut app,
            add,
            window,
            PointerButton::Primary,
            MouseButton::Left,
        );
        assert_eq!(
            app.world().resource::<ShopSession>().cart_sell.get(&0),
            Some(&2)
        );
    }

    #[test]
    fn adding_a_row_takes_the_typed_quantity_up_to_the_stack() {
        let mut app = real_button_pipeline_app(session(ShopTab::Sell));
        let add = add_button(&mut app, 0, "99");

        app.world_mut().trigger(Activate { entity: add });

        assert_eq!(
            app.world().resource::<ShopSession>().cart_sell.get(&0),
            Some(&5)
        );
    }

    #[test]
    fn the_row_stepper_edits_its_field_and_leaves_the_session_unchanged() {
        let mut app = real_button_pipeline_app(session(ShopTab::Sell));
        let field = app
            .world_mut()
            .spawn((ShopQtyField(0), EditableText::new("4")))
            .id();
        let other = app
            .world_mut()
            .spawn((ShopQtyField(1), EditableText::new("1")))
            .id();
        app.update();

        let value = |app: &App, field: Entity| {
            let text = app.world().get::<EditableText>(field).unwrap();
            text.value().to_string()
        };
        press(&mut app, ShopButtonAction::RowInc(0));
        assert_eq!(value(&app, field), "5");
        // Slot 0 holds five, so the stepper stops there.
        press(&mut app, ShopButtonAction::RowInc(0));
        assert_eq!(value(&app, field), "5");
        press(&mut app, ShopButtonAction::RowDec(1));
        assert_eq!(value(&app, other), "1");

        // A changed session would redraw the list and drop the typed quantities.
        assert!(!app.world().is_resource_changed::<ShopSession>());
    }

    #[test]
    fn a_purchase_is_sent_without_a_warning() {
        let mut draft = session(ShopTab::Buy);
        draft.cart_buy.insert(501, 3);
        let mut app = real_button_pipeline_app(draft);
        player_with(&mut app, 100);

        press(&mut app, ShopButtonAction::Submit);

        assert!(!app.world().contains_resource::<ShopSession>());
        assert_eq!(app.world().resource::<Messages<BuyFromShop>>().len(), 1);
    }

    #[test]
    fn selling_refined_gear_warns_first_and_sell_anyway_sends_it() {
        let mut draft = session(ShopTab::Sell);
        draft.cart_sell.insert(0, 1);
        let mut app = real_button_pipeline_app(draft);
        player_with(&mut app, 0);
        app.world_mut().resource_mut::<Inventory>().upsert(
            lifthrasir_client::domain::inventory::Item {
                index: 0,
                item_id: 501,
                identified: true,
                refine: 7,
                ..Default::default()
            },
        );

        // "Sell Anyway" only works once the warning is up.
        press(&mut app, ShopButtonAction::ConfirmTrade);
        assert_eq!(app.world().resource::<Messages<SellToShop>>().len(), 0);

        press(&mut app, ShopButtonAction::Submit);
        assert!(app.world().resource::<ShopSession>().confirm_open);
        assert_eq!(app.world().resource::<Messages<SellToShop>>().len(), 0);

        press(&mut app, ShopButtonAction::CancelConfirm);
        assert!(!app.world().resource::<ShopSession>().confirm_open);

        press(&mut app, ShopButtonAction::Submit);
        press(&mut app, ShopButtonAction::ConfirmTrade);
        assert!(!app.world().contains_resource::<ShopSession>());
        assert_eq!(app.world().resource::<Messages<SellToShop>>().len(), 1);
    }

    #[test]
    fn plain_goods_sell_without_a_warning_and_loot_only_fills_the_sell_cart() {
        let mut draft = session(ShopTab::Buy);
        draft.sell_items[0].nameid = 909;
        let mut app = real_button_pipeline_app(draft);
        player_with(&mut app, 0);
        app.world_mut().resource_mut::<Inventory>().upsert(
            lifthrasir_client::domain::inventory::Item {
                index: 0,
                item_id: 909,
                item_type: IT_ETC,
                identified: true,
                ..Default::default()
            },
        );

        press(&mut app, ShopButtonAction::AddLoot);
        assert!(app.world().resource::<ShopSession>().cart_sell.is_empty());

        press(&mut app, ShopButtonAction::SwitchTab(ShopTab::Sell));
        press(&mut app, ShopButtonAction::AddLoot);
        assert_eq!(
            app.world().resource::<ShopSession>().cart_sell.get(&0),
            Some(&5)
        );

        press(&mut app, ShopButtonAction::Submit);
        assert_eq!(app.world().resource::<Messages<SellToShop>>().len(), 1);
    }

    fn list_viewport(app: &mut App) -> (Entity, Vec2) {
        let world = app.world_mut();
        world
            .query::<(Entity, &ShopScroll, &ScrollPosition)>()
            .iter(world)
            .find(|(_, scroll, _)| **scroll == ShopScroll::List)
            .map(|(entity, _, position)| (entity, position.0))
            .expect("one list viewport")
    }

    #[test]
    fn window_fills_its_mounts_redraws_on_change_and_keeps_the_list_scroll() {
        let mut draft = session(ShopTab::Sell);
        draft.cart_sell.insert(0, 2);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<PendingShopDeal>();
        app.init_resource::<Inventory>();
        app.insert_resource(draft);
        app.add_systems(Update, rebuild_body);
        player_with(&mut app, 1000);
        app.world_mut()
            .commands()
            .spawn_scene(scene::window("Tool Dealer".to_string()));
        app.world_mut().flush();
        app.update();

        let world = app.world_mut();
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        for expected in [
            "Tool Dealer",
            "Buy",
            "Sell",
            "LOOT",
            "PAYS",
            "TO SELL",
            "1 line",
            "2 to sell",
            "Sell for 8 z",
            "1,008 z",
            "Bag 0 / 100",
        ] {
            assert!(
                texts.iter().any(|text| text == expected),
                "{expected:?} missing from {texts:?}"
            );
        }
        assert_eq!(world.query::<&ShopRow>().iter(world).count(), 2);
        assert_eq!(world.query::<&ShopQtyField>().iter(world).count(), 2);
        assert_eq!(
            world
                .query::<(&ShopMount, &Children)>()
                .iter(world)
                .filter(|(_, children)| children.len() == 1)
                .count(),
            4
        );

        let (list, _) = list_viewport(&mut app);
        app.world_mut()
            .entity_mut(list)
            .insert(ScrollPosition(Vec2::new(0.0, 40.0)));
        app.update();
        assert_eq!(list_viewport(&mut app).0, list, "nothing changed");

        app.world_mut().resource_mut::<ShopSession>().dec_line(0);
        app.update();
        let (redrawn, scroll) = list_viewport(&mut app);
        assert_ne!(redrawn, list);
        assert_eq!(scroll, Vec2::new(0.0, 40.0));

        app.world_mut().resource_mut::<ShopSession>().query = "x".to_string();
        app.update();
        assert_eq!(list_viewport(&mut app).1, Vec2::ZERO);
    }
}
