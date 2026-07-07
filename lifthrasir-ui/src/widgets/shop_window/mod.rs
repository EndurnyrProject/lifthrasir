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
use net_contract::dto::{BuyEntry, SellEntry, ShopBuyItem, ShopResult, ShopSellItem};
use net_contract::events::ShopOpened;

use crate::theme::feathers_theme::install_norse_theme;
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
/// key space `ShopSession`'s cart maps already use. Task 7 only attaches this
/// marker to each interactive node; the handler that reads it lands in Task 8.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopButtonAction {
    SwitchTab(ShopTab),
    Select(Selection),
    IncQty(u32),
    DecQty(u32),
    RemoveLine(u32),
    #[default]
    OpenConfirm,
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
    pub banner: Option<ShopResult>,
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

    /// Empties only the active tab's cart, leaving the other tab intact.
    pub fn clear_active_cart(&mut self) {
        self.active_cart_mut().clear();
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
            rebuild_body
                .run_if(in_state(GameState::InGame).and_then(resource_changed::<ShopSession>)),
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
        banner: None,
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
            banner: None,
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
}
