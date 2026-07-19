//! NPC shop window: the `ShopSession` resource and its pure cart logic, plus the
//! open/close lifecycle (design `2026-07-07-npc-shops`). `on_shop_opened` spawns
//! the chrome and force-closes any active NPC dialog; `close_shop`/the titlebar
//! close button despawn it locally (no packet). The body projection and cart
//! interactions land in later tasks.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_feathers::FeathersCorePlugin;
use bevy_feathers::FeathersPlugins;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::entities::registry::EntityRegistry;
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;
use net_contract::commands::{BuyFromShop, SellToShop};
use net_contract::dto::{BuyEntry, SellEntry, ShopBuyItem, ShopResult, ShopSellItem};
use net_contract::events::{ChatHeard, ShopBuyResulted, ShopOpened, ShopSellResulted};

use crate::theme::feathers_theme::install_norse_theme;
use crate::widgets::info_modal::{InfoTarget, ItemRef, ShowInfoModal};
use crate::widgets::npc_dialog::{ActiveNpcDialog, NpcDialogRoot};

pub mod scene;

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
/// two cart maps, the active tab, the selection, and any result banner. The
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
    pub banner: Option<ShopResult>,
    /// Whether the confirm-trade overlay is showing. `rebuild_body` renders it
    /// as part of the body region on every `ShopSession` change.
    pub confirm_open: bool,
    /// Set the instant `ConfirmTrade` emits the batched command; cleared when
    /// its result arrives. Models the design's `Await` state (§5.4): while
    /// true, `on_shop_button` ignores every cart-editing/re-confirm action so a
    /// tab switch or a second confirm can't race the in-flight request.
    pub awaiting: bool,
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
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(Update, on_shop_opened.run_if(in_state(GameState::InGame)));
        app.add_systems(
            Update,
            close_shop.run_if(in_state(GameState::InGame).and_then(resource_exists::<ShopSession>)),
        );
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
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ShopSession>()
        });
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
        banner: None,
        confirm_open: false,
        awaiting: false,
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
    bodies: Query<(Entity, Option<&Children>), With<ShopWindowBody>>,
    item_db: Option<Res<ItemDb>>,
    inventory: Res<Inventory>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
) {
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
        ))
        .insert(ChildOf(body));
}

/// Despawns the shop window and clears [`ShopSession`]. Shared by the titlebar
/// close button and the Escape key; sends no packet — the server holds no
/// shop-open state to close (design §4).
fn close_shop_window(commands: &mut Commands, roots: &Query<Entity, With<ShopWindowRoot>>) {
    if let Ok(root) = roots.single() {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<ShopSession>();
}

/// ESC ends the shop exactly like the titlebar close button: despawn the window
/// and clear `ShopSession`. Gated on `ShopSession` existing, so it only fires
/// while a shop is open; `settings_window`'s own Escape toggle is separately
/// gated to skip while this resource is present (mirrors `npc_dialog`).
fn close_shop(
    keys: Res<ButtonInput<KeyCode>>,
    roots: Query<Entity, With<ShopWindowRoot>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    close_shop_window(&mut commands, &roots);
}

/// Titlebar close button: same effect as Escape.
pub(super) fn on_shop_close_button(
    _: On<Activate>,
    roots: Query<Entity, With<ShopWindowRoot>>,
    mut commands: Commands,
) {
    close_shop_window(&mut commands, &roots);
}

/// Whether `action` must be ignored while a trade round-trip is in flight
/// (`ShopSession.awaiting`, design §5.4's `Await` state): every cart edit and
/// (re-)confirm is blocked so a tab switch or a second confirm can't race the
/// in-flight request. `Select`, `CancelConfirm`, the titlebar close button,
/// and Escape stay live — closing/looking around while waiting is harmless.
fn blocked_while_awaiting(action: ShopButtonAction) -> bool {
    matches!(
        action,
        ShopButtonAction::SwitchTab(_)
            | ShopButtonAction::IncQty(_)
            | ShopButtonAction::DecQty(_)
            | ShopButtonAction::RemoveLine(_)
            | ShopButtonAction::PendingDec
            | ShopButtonAction::PendingInc
            | ShopButtonAction::AddToCart
            | ShopButtonAction::OpenConfirm
            | ShopButtonAction::ConfirmTrade
    )
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

/// Shared handler for every interactive shop-window node: tab switch, select,
/// cart +/-/remove mutate `ShopSession` directly; `PendingDec`/`PendingInc`
/// step `pending_qty` (clamped via [`clamp_pending`], capped at the selected
/// Sell slot's snapshot `amount`); `AddToCart` adds `pending_qty` of the
/// selected item to the active cart; `OpenConfirm` re-checks the same guard
/// the CTA's disabled look already encodes (a race between render and click
/// is possible, since the CTA has no `InteractionDisabled`) before opening the
/// overlay; `ConfirmTrade` emits the batched command for the active tab, sets
/// `awaiting`, and closes the overlay, leaving the cart intact until the
/// result arrives; `CancelConfirm` just closes the overlay. While `awaiting`
/// is set, [`blocked_while_awaiting`] actions are ignored outright.
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
    mut session: ResMut<ShopSession>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut buy_writer: MessageWriter<BuyFromShop>,
    mut sell_writer: MessageWriter<SellToShop>,
) {
    if mouse.just_released(MouseButton::Right) {
        return;
    }
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if session.awaiting && blocked_while_awaiting(*action) {
        return;
    }

    match *action {
        ShopButtonAction::SwitchTab(tab) => {
            session.tab = tab;
            session.selected = None;
            session.banner = None;
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
            if !scene::cta_enabled(&session, status.zeny) {
                return;
            }
            session.confirm_open = true;
        }
        ShopButtonAction::ConfirmTrade => {
            let unit_id = session.unit_id;
            match session.tab {
                ShopTab::Buy => {
                    buy_writer.write(BuyFromShop {
                        unit_id,
                        items: session.to_buy_entries(),
                    });
                }
                ShopTab::Sell => {
                    sell_writer.write(SellToShop {
                        unit_id,
                        items: session.to_sell_entries(),
                    });
                }
            }
            session.confirm_open = false;
            session.awaiting = true;
        }
        ShopButtonAction::CancelConfirm => {
            session.confirm_open = false;
        }
    }
}

/// Secondary-click on a stock row opens the info modal for that item instead of
/// selecting it: a Buy row resolves to `ItemRef::ShopBuy(nameid)`, a Sell row to
/// `ItemRef::Inventory(inventory_index)` (the sell snapshot mirrors a live bag
/// slot). Only fires for `Select` rows — the row builder is the only shop
/// button carrying that action, but the guard keeps this safe if it's ever
/// attached elsewhere; other shop buttons (tab switch, qty steppers, cart
/// controls) stay untouched.
pub(super) fn on_shop_row_secondary_click(
    click: On<Pointer<Click>>,
    actions: Query<&ShopButtonAction>,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }
    let Ok(ShopButtonAction::Select(selection)) = actions.get(click.entity) else {
        return;
    };
    let item_ref = match *selection {
        Selection::Buy(nameid) => ItemRef::ShopBuy(nameid),
        Selection::Sell(index) => ItemRef::Inventory(index as u16),
    };
    info_writer.write(ShowInfoModal {
        target: InfoTarget::Item(item_ref),
    });
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

/// Applies one buy/sell result to `session`, returning the chat summary to
/// post on success (`None` on error, once `banner` is set). `Ok`: builds the
/// trade summary from the entries about to be cleared, decrements the sell
/// snapshot for a sell, then clears the cart for `buy`'s tab specifically —
/// **not** `clear_active_cart()`/`session.tab`, since the player may have
/// switched tabs while the request was in flight (design §5.4's `Await`
/// state); clearing the active tab instead of the traded one would leave a
/// just-bought/-sold cart line alive for a re-send and wipe the untraded
/// tab's cart instead. Any error sets `banner` to the mapped reason and
/// preserves the cart for retry. Either way, closes the confirm overlay and
/// clears `awaiting` — the request that opened them has now settled.
fn apply_result(
    session: &mut ShopSession,
    buy: bool,
    result: ShopResult,
    item_db: Option<&ItemDb>,
) -> Option<String> {
    session.confirm_open = false;
    session.awaiting = false;
    if result != ShopResult::Ok {
        session.banner = Some(result);
        return None;
    }

    let message = if buy {
        let lines: Vec<(u32, u32)> = session
            .to_buy_entries()
            .into_iter()
            .map(|entry| (entry.nameid, entry.amount))
            .collect();
        trade_summary(true, &lines, item_db)
    } else {
        let sold = session.to_sell_entries();
        let lines: Vec<(u32, u32)> = sold
            .iter()
            .filter_map(|entry| {
                session
                    .sell_items
                    .iter()
                    .find(|item| item.inventory_index == entry.inventory_index)
                    .map(|item| (item.nameid, entry.amount))
            })
            .collect();
        let message = trade_summary(false, &lines, item_db);
        session.apply_sold(&sold);
        message
    };

    if buy {
        session.cart_buy.clear();
    } else {
        session.cart_sell.clear();
    }
    session.banner = None;
    Some(message)
}

/// Reads every `ShopBuyResulted`/`ShopSellResulted` this frame and applies each
/// to `ShopSession` via [`apply_result`], posting a `ChatHeard` line on
/// success. Inventory/zeny themselves refresh via the existing
/// `ItemAdded`/`ItemRemoved`/`ParamChange` paths (design §5.4) — this system
/// never touches `Inventory`/`Status`.
fn on_shop_result(
    mut buy_results: MessageReader<ShopBuyResulted>,
    mut sell_results: MessageReader<ShopSellResulted>,
    mut session: ResMut<ShopSession>,
    item_db: Option<Res<ItemDb>>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for event in buy_results.read() {
        if let Some(message) = apply_result(&mut session, true, event.result, item_db.as_deref()) {
            chat.write(ChatHeard { gid: 0, message });
        }
    }
    for event in sell_results.read() {
        if let Some(message) = apply_result(&mut session, false, event.result, item_db.as_deref()) {
            chat.write(ChatHeard { gid: 0, message });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            banner: None,
            confirm_open: false,
            awaiting: false,
        }
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
    fn apply_result_ok_clears_cart_and_returns_summary() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.confirm_open = true;
        let message = apply_result(&mut s, true, ShopResult::Ok, None);
        assert_eq!(message, Some("Purchased 3x #501".to_string()));
        assert!(s.cart_buy.is_empty());
        assert_eq!(s.banner, None);
        assert!(!s.confirm_open);
    }

    #[test]
    fn apply_result_ok_sell_decrements_snapshot_and_clears_cart() {
        let mut s = session(ShopTab::Sell);
        s.cart_sell.insert(0, 5);
        s.confirm_open = true;
        let message = apply_result(&mut s, false, ShopResult::Ok, None);
        assert_eq!(message, Some("Sold 5x #501".to_string()));
        assert!(s.cart_sell.is_empty());
        assert!(!s.sell_items.iter().any(|item| item.inventory_index == 0));
    }

    #[test]
    fn apply_result_error_sets_banner_and_preserves_cart() {
        let mut s = session(ShopTab::Buy);
        s.cart_buy.insert(501, 3);
        s.confirm_open = true;
        let message = apply_result(&mut s, true, ShopResult::NotEnoughZeny, None);
        assert_eq!(message, None);
        assert_eq!(s.banner, Some(ShopResult::NotEnoughZeny));
        assert_eq!(s.cart_buy.get(&501), Some(&3));
        assert!(!s.confirm_open);
    }

    #[test]
    fn apply_result_ok_clears_by_result_tab_not_active_tab() {
        // Player confirmed a Buy, then switched to Sell before the result
        // arrived: `session.tab` is now Sell, but the result is still `buy`.
        let mut s = session(ShopTab::Sell);
        s.cart_buy.insert(501, 3);
        s.cart_sell.insert(0, 2);
        s.awaiting = true;
        let message = apply_result(&mut s, true, ShopResult::Ok, None);
        assert_eq!(message, Some("Purchased 3x #501".to_string()));
        assert!(s.cart_buy.is_empty());
        assert_eq!(s.cart_sell.get(&0), Some(&2));
    }

    #[test]
    fn apply_result_clears_awaiting_on_both_ok_and_error() {
        let mut ok = session(ShopTab::Buy);
        ok.cart_buy.insert(501, 3);
        ok.awaiting = true;
        apply_result(&mut ok, true, ShopResult::Ok, None);
        assert!(!ok.awaiting);

        let mut err = session(ShopTab::Buy);
        err.cart_buy.insert(501, 3);
        err.awaiting = true;
        apply_result(&mut err, true, ShopResult::NotEnoughZeny, None);
        assert!(!err.awaiting);
    }

    #[test]
    fn blocked_while_awaiting_covers_cart_edits_and_confirm() {
        assert!(blocked_while_awaiting(ShopButtonAction::SwitchTab(
            ShopTab::Sell
        )));
        assert!(blocked_while_awaiting(ShopButtonAction::IncQty(501)));
        assert!(blocked_while_awaiting(ShopButtonAction::DecQty(501)));
        assert!(blocked_while_awaiting(ShopButtonAction::RemoveLine(501)));
        assert!(blocked_while_awaiting(ShopButtonAction::PendingDec));
        assert!(blocked_while_awaiting(ShopButtonAction::PendingInc));
        assert!(blocked_while_awaiting(ShopButtonAction::AddToCart));
        assert!(blocked_while_awaiting(ShopButtonAction::OpenConfirm));
        assert!(blocked_while_awaiting(ShopButtonAction::ConfirmTrade));
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

    #[test]
    fn blocked_while_awaiting_allows_select_and_cancel() {
        assert!(!blocked_while_awaiting(ShopButtonAction::Select(
            Selection::Buy(501)
        )));
        assert!(!blocked_while_awaiting(ShopButtonAction::CancelConfirm));
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

    fn click_event(target: Entity, window: Entity, button: PointerButton) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    fn row_click_app() -> App {
        let mut app = App::new();
        app.add_message::<ShowInfoModal>();
        app
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

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let targets: Vec<InfoTarget> = reader.read(messages).map(|m| m.target).collect();
        assert_eq!(targets, vec![InfoTarget::Item(ItemRef::ShopBuy(501))]);
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

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let targets: Vec<InfoTarget> = reader.read(messages).map(|m| m.target).collect();
        assert_eq!(targets, vec![InfoTarget::Item(ItemRef::Inventory(3))]);
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

    fn press_event(target: Entity, window: Entity, button: PointerButton) -> Pointer<Press> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Press {
                button,
                hit: HitData::new(target, 0.0, None, None),
                count: 1,
            },
            target,
        )
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
        app.add_message::<ShowInfoModal>();
        app.add_message::<BuyFromShop>();
        app.add_message::<SellToShop>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.insert_resource(session);
        app
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

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let targets: Vec<InfoTarget> = reader.read(messages).map(|m| m.target).collect();
        assert_eq!(targets, vec![InfoTarget::Item(ItemRef::Inventory(0))]);

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
