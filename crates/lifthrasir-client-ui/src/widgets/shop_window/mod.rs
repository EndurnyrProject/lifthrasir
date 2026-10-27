//! Single-use NPC shop drafts, cart controls and authoritative deal feedback.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
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

use crate::widgets::info_modal::{InfoContent, ItemAction, ShowInfoModal, view};
use crate::widgets::npc_dialog::{ActiveNpcDialog, NpcDialogRoot};

mod lifecycle;
pub mod scene;

use lifecycle::{PendingShopDeal, ShopDeal, on_shop_result};

/// Which tab of the shop window is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShopTab {
    #[default]
    Buy,
    Sell,
}

/// The currently selected grid cell, keyed by the tab it belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    /// A Buy-tab cell, keyed by `nameid`.
    Buy(u32),
    /// A Sell-tab cell, keyed by `inventory_index`.
    Sell(u32),
}

/// What a shop button does when activated. The cart-edit variants carry the
/// active tab's cart key (`nameid` on Buy, `inventory_index` on Sell) — the same
/// key space `ShopSession`'s cart maps already use.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopButtonAction {
    SwitchTab(ShopTab),
    Select(Selection),
    IncQty(u32),
    DecQty(u32),
    RemoveLine(u32),
    /// Decrements the detail's pending-quantity stepper (design's "add row").
    PendingDec,
    /// Increments the detail's pending-quantity stepper.
    PendingInc,
    /// Adds `pending_qty` of the selected item to the active cart.
    AddToCart,
    #[default]
    OpenConfirm,
    ConfirmTrade,
    CancelConfirm,
}

/// Window-root marker: the outer chrome (wrapper, titlebar, card). A single
/// instance exists for the lifetime of an open shop.
#[derive(Component, Default, Clone)]
pub struct ShopWindowRoot;

/// The swappable body region (grid, detail, cart, footer), rebuilt on every
/// `ShopSession` change.
#[derive(Component, Default, Clone)]
pub struct ShopWindowBody;

/// The draggable titlebar; the drag observer only moves the window when the
/// drag's target is the titlebar itself, so dragging from the close button is
/// inert.
#[derive(Component, Default, Clone)]
pub struct ShopWindowTitlebar;

const FALLBACK_TITLE: &str = "Shop";

/// Single source of truth for an open shop: the server's buy/sell snapshots, the
/// two cart maps, the active tab and selection. The
/// window is a pure projection of this resource.
#[derive(Resource, Clone, Debug)]
pub struct ShopSession {
    pub unit_id: u64,
    pub buy_items: Vec<ShopBuyItem>,
    pub sell_items: Vec<ShopSellItem>,
    pub tab: ShopTab,
    pub cart_buy: HashMap<u32, u32>,
    pub cart_sell: HashMap<u32, u32>,
    pub selected: Option<Selection>,
    /// The quantity staged in the detail panel's stepper, added to the active
    /// cart by `AddToCart`. Defaults to 1; reset to 1 on `Select`/`SwitchTab`.
    pub pending_qty: u32,
    /// Whether the confirm-trade overlay is showing. `rebuild_body` renders it
    /// as part of the body region on every `ShopSession` change.
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

    /// The active tab's cart map, mutably.
    fn active_cart_mut(&mut self) -> &mut HashMap<u32, u32> {
        match self.tab {
            ShopTab::Buy => &mut self.cart_buy,
            ShopTab::Sell => &mut self.cart_sell,
        }
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

    /// Decrements the sell snapshot's `amount` for each sold slot by the traded
    /// qty, dropping any slot that reaches 0 — keeps the snapshot honest until
    /// the next shop open (design §5.4).
    pub fn apply_sold(&mut self, entries: &[SellEntry]) {
        for entry in entries {
            if let Some(item) = self
                .sell_items
                .iter_mut()
                .find(|item| item.inventory_index == entry.inventory_index)
            {
                item.amount = item.amount.saturating_sub(entry.amount);
            }
        }
        self.sell_items.retain(|item| item.amount > 0);
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
                rebuild_body.run_if(resource_exists::<ShopSession>),
            )
                .chain()
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
        selected: None,
        pending_qty: 1,
        confirm_open: false,
        retired: false,
    });
}

/// Rebuilds the swappable [`ShopWindowBody`] region on every [`ShopSession`]
/// change: despawns its existing children and respawns the tab strip, grid,
/// detail panel, cart, and footer from the session snapshot. A missing body
/// entity (the first open frame, before `on_shop_opened`'s spawn lands) skips
/// this pass silently; change detection retries on the next `ShopSession`
/// write. A missing local player is not papered over with a zero-zeny
/// fallback — `LocalPlayer`/`CharacterStatus` are inserted atomically at
/// character spawn and are guaranteed present before any NPC talk, so their
/// absence here is a real bug, not an expected transient; it's logged loudly
/// rather than rendering a fabricated balance.
fn rebuild_body(
    mut commands: Commands,
    session: Res<ShopSession>,
    pending: Res<PendingShopDeal>,
    bodies: Query<(Entity, Option<&Children>), With<ShopWindowBody>>,
    item_db: Option<Res<ItemDb>>,
    inventory: Res<Inventory>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
) {
    if !session.is_changed() && !pending.is_changed() {
        return;
    }
    let Ok((body, children)) = bodies.single() else {
        return;
    };
    let Ok(status) = player.single() else {
        warn!("shop window rebuild skipped: no LocalPlayer/CharacterStatus while a shop is open");
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::body(
            &session,
            status.zeny,
            item_db.as_deref(),
            &inventory,
            pending.0.is_some(),
        ))
        .insert(ChildOf(body));
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

/// The key (`nameid` on Buy, `inventory_index` on Sell) of the currently
/// selected item, or `None` when nothing is selected.
fn selected_key(selection: Option<Selection>) -> Option<u32> {
    match selection? {
        Selection::Buy(nameid) => Some(nameid),
        Selection::Sell(index) => Some(index),
    }
}

/// Clamps a pending-quantity stepper value into `[1, cap]`. `cap` is
/// `u32::MAX` on the Buy tab (unbounded) and the selected Sell slot's
/// snapshot `amount` on the Sell tab.
fn clamp_pending(qty: u32, cap: u32) -> u32 {
    qty.clamp(1, cap.max(1))
}

#[derive(SystemParam)]
pub(super) struct ShopRequests<'w, 's> {
    pending: ResMut<'w, PendingShopDeal>,
    buy: MessageWriter<'w, BuyFromShop>,
    sell: MessageWriter<'w, SellToShop>,
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, Entity, With<ShopWindowRoot>>,
}

/// Shared handler for every interactive shop-window node: tab switch, select,
/// cart +/-/remove mutate `ShopSession` directly; `PendingDec`/`PendingInc`
/// step `pending_qty` (clamped via [`clamp_pending`], capped at the selected
/// Sell slot's snapshot `amount`); `AddToCart` adds `pending_qty` of the
/// selected item to the active cart; `OpenConfirm` re-checks the same guard
/// the CTA's disabled look already encodes (a race between render and click
/// is possible, since the CTA has no `InteractionDisabled`) before opening the
/// overlay; `ConfirmTrade` records one pending deal and closes the window.
/// A replacement draft cannot submit until that deal settles.
///
/// `bevy_ui_widgets`' `Button` fires `Activate` on release of *any* pointer
/// button (`button_on_pointer_click` has no button check), so a right-click
/// on a row — meant only to open the info modal — reaches this handler too.
/// The pointer's own release always lands in the same frame as the `Activate`
/// it triggers, so gating on that frame's `ButtonInput<MouseButton>` is
/// deterministic frame-scoped state, not a bet on observer-execution order
/// between this handler and the row's separate secondary-click observer.
pub(super) fn on_shop_button(
    activate: On<Activate>,
    actions: Query<&ShopButtonAction>,
    mut session: Option<ResMut<ShopSession>>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut requests: ShopRequests,
) {
    if mouse.just_released(MouseButton::Right) {
        return;
    }
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let Some(session) = session.as_deref_mut() else {
        return;
    };
    if session.retired
        || (requests.pending.0.is_some()
            && matches!(
                action,
                ShopButtonAction::OpenConfirm | ShopButtonAction::ConfirmTrade
            ))
    {
        return;
    }

    match *action {
        ShopButtonAction::SwitchTab(tab) => {
            session.tab = tab;
            session.selected = None;
            session.pending_qty = 1;
        }
        ShopButtonAction::Select(selection) => {
            session.selected = Some(selection);
            session.pending_qty = 1;
        }
        ShopButtonAction::IncQty(key) => {
            session.add_to_cart(key, 1);
        }
        ShopButtonAction::DecQty(key) => {
            session.dec_line(key);
        }
        ShopButtonAction::RemoveLine(key) => {
            session.remove_line(key);
        }
        ShopButtonAction::PendingDec => {
            if let Some(key) = selected_key(session.selected) {
                let cap = session.cap_for(key);
                session.pending_qty = clamp_pending(session.pending_qty.saturating_sub(1), cap);
            }
        }
        ShopButtonAction::PendingInc => {
            if let Some(key) = selected_key(session.selected) {
                let cap = session.cap_for(key);
                session.pending_qty = clamp_pending(session.pending_qty.saturating_add(1), cap);
            }
        }
        ShopButtonAction::AddToCart => {
            if let Some(key) = selected_key(session.selected) {
                let qty = session.pending_qty;
                session.add_to_cart(key, qty);
            }
        }
        ShopButtonAction::OpenConfirm => {
            let Ok(status) = player.single() else {
                warn!("shop confirm requested with no LocalPlayer/CharacterStatus present");
                return;
            };
            if !scene::cta_enabled(session, status.zeny) {
                return;
            }
            session.confirm_open = true;
        }
        ShopButtonAction::ConfirmTrade => {
            let Ok(status) = player.single() else {
                return;
            };
            if !session.confirm_open || !scene::cta_enabled(session, status.zeny) {
                return;
            }
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
        ShopButtonAction::CancelConfirm => {
            session.confirm_open = false;
        }
    }
}

/// Secondary-click on a stock row opens the info modal for that item instead of
/// selecting it: a Buy row shows the stock item with its price, a Sell row shows
/// the bag item (the sell snapshot mirrors a live bag slot). Only fires for `Select` rows — the row builder is the only shop
/// button carrying that action, but the guard keeps this safe if it's ever
/// attached elsewhere; other shop buttons (tab switch, qty steppers, cart
/// controls) stay untouched.
pub(super) fn on_shop_row_secondary_click(
    click: On<PointerClick>,
    actions: Query<&ShopButtonAction>,
    session: Option<Res<ShopSession>>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    option_db: Option<Res<RandomOptionDb>>,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }
    let Ok(ShopButtonAction::Select(selection)) = actions.get(click.entity) else {
        return;
    };
    let Some(item_db) = item_db.as_deref() else {
        warn!("shop: ItemDb not loaded yet, ignoring inspect");
        return;
    };
    let content = match *selection {
        Selection::Buy(nameid) => {
            let price = session
                .as_deref()
                .and_then(|s| s.buy_items.iter().find(|i| i.nameid == nameid))
                .map(|i| i.price);
            InfoContent::Item {
                view: view::shop_item_view(nameid, price, item_db),
                action: None,
            }
        }
        Selection::Sell(index) => {
            let Some(item) = inventory.get(index as u16) else {
                return;
            };
            InfoContent::Item {
                view: view::inventory_item_view(item, item_db, option_db.as_deref()),
                action: ItemAction::for_bag_item(item),
            }
        }
    };
    info_writer.write(ShowInfoModal { content });
}

/// Resolves a display name for `nameid` via `ItemDb`, always as identified — the
/// trade summary is a concise confirmation line, not a detail panel — falling
/// back to `#nameid` when the db or the entry is missing.
fn resolved_name(item_db: Option<&ItemDb>, nameid: u32) -> String {
    item_db
        .and_then(|db| db.name(nameid, true))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{nameid}"))
}

/// Builds the chat-line summary for a successful trade, e.g. "Purchased 30x Red
/// Potion, 3x White Potion" / "Sold 200x Jellopy". `lines` is `(nameid, qty)`
/// pairs; empty input still yields a verb-only sentence, which should not occur
/// in practice since `ConfirmTrade` only ever fires on a non-empty cart.
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
        events::{ChatHeard, ShopBuyResulted, ShopSellResulted},
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
            selected: None,
            pending_qty: 1,
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
    fn apply_sold_decrements_and_drops_exhausted_slots() {
        let mut s = session(ShopTab::Sell);
        s.apply_sold(&[SellEntry {
            inventory_index: 0,
            amount: 5,
        }]);
        assert!(!s.sell_items.iter().any(|item| item.inventory_index == 0));
        assert!(s.sell_items.iter().any(|item| item.inventory_index == 1));
    }

    #[test]
    fn apply_sold_leaves_a_partial_slot_with_the_remainder() {
        let mut s = session(ShopTab::Sell);
        s.apply_sold(&[SellEntry {
            inventory_index: 0,
            amount: 2,
        }]);
        let item = s
            .sell_items
            .iter()
            .find(|item| item.inventory_index == 0)
            .unwrap();
        assert_eq!(item.amount, 3);
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
                .add_message::<ChatHeard>()
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
                .resource::<Messages<ChatHeard>>()
                .iter_current_update_messages()
                .collect();
            assert_eq!(lines.len(), 1);
            if result == ShopResult::Ok {
                assert_eq!(lines[0].message, "Purchased 3x #501");
            }
            app.world_mut().remove_resource::<ShopSession>();
            app.world_mut().write_message(ShopBuyResulted { result });
            app.update();
            assert_eq!(app.world().resource::<Messages<ChatHeard>>().len(), 1);
        }
    }

    #[test]
    fn selected_key_resolves_buy_and_sell() {
        assert_eq!(selected_key(Some(Selection::Buy(501))), Some(501));
        assert_eq!(selected_key(Some(Selection::Sell(3))), Some(3));
        assert_eq!(selected_key(None), None);
    }

    #[test]
    fn clamp_pending_floors_at_one() {
        assert_eq!(clamp_pending(0, 100), 1);
    }

    #[test]
    fn clamp_pending_caps_at_provided_cap() {
        assert_eq!(clamp_pending(50, 5), 5);
    }

    #[test]
    fn clamp_pending_passes_through_in_range_values() {
        assert_eq!(clamp_pending(3, 10), 3);
    }

    #[test]
    fn clamp_pending_treats_zero_cap_as_one() {
        assert_eq!(clamp_pending(5, 0), 1);
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
        app.add_message::<ChatHeard>();
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

    fn row_click_app() -> App {
        let mut app = App::new();
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
        let mut app = row_click_app();
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopButtonAction::Select(Selection::Buy(501)))
            .observe(on_shop_row_secondary_click)
            .id();

        app.world_mut()
            .trigger(click_event(row, window, PointerButton::Secondary));

        let contents = contents(&app);
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: None } if view.item_id == 501 && view.meta.is_empty()
        ));
    }

    #[test]
    fn secondary_click_on_a_sell_row_opens_the_info_modal_for_inventory() {
        let mut app = row_click_app();
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopButtonAction::Select(Selection::Sell(3)))
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
    fn secondary_click_on_a_non_select_button_writes_nothing() {
        let mut app = row_click_app();
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
        let mut app = row_click_app();
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn(ShopButtonAction::Select(Selection::Buy(501)))
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

    #[test]
    fn real_secondary_click_pipeline_opens_the_modal_without_touching_the_session() {
        use bevy::ui_widgets::Button;

        let mut s = session(ShopTab::Sell);
        s.selected = Some(Selection::Sell(1));
        s.pending_qty = 7;
        let mut app = real_button_pipeline_app(s);
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn((Button, ShopButtonAction::Select(Selection::Sell(0))))
            .observe(on_shop_button)
            .observe(on_shop_row_secondary_click)
            .id();

        real_click(
            &mut app,
            row,
            window,
            PointerButton::Secondary,
            MouseButton::Right,
        );

        let contents = contents(&app);
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: Some(ItemAction { index: 0, .. }) }
                if view.item_id == 501
        ));

        let session = app.world().resource::<ShopSession>();
        assert_eq!(session.selected, Some(Selection::Sell(1)));
        assert_eq!(session.pending_qty, 7);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<SellToShop>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn real_primary_click_pipeline_still_selects_and_does_not_open_the_modal() {
        use bevy::ui_widgets::Button;

        let mut s = session(ShopTab::Sell);
        s.selected = Some(Selection::Sell(1));
        s.pending_qty = 7;
        let mut app = real_button_pipeline_app(s);
        let window = app.world_mut().spawn_empty().id();
        let row = app
            .world_mut()
            .spawn((Button, ShopButtonAction::Select(Selection::Sell(0))))
            .observe(on_shop_button)
            .observe(on_shop_row_secondary_click)
            .id();

        real_click(
            &mut app,
            row,
            window,
            PointerButton::Primary,
            MouseButton::Left,
        );

        let session = app.world().resource::<ShopSession>();
        assert_eq!(session.selected, Some(Selection::Sell(0)));
        assert_eq!(session.pending_qty, 1);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }
}
