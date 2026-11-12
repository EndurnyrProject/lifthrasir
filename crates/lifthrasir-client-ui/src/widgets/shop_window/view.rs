//! Projects [`ShopSession`] and the bag into what the Shop window draws.
//!
//! The client knows no item weights and cannot tell a stackable from a one-per-slot
//! item by id, so the design's "Weight after" read-out and the Buy tab's "Bag after"
//! are left out.

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::paths::item_icon_path;
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::ShopSellItem;

use crate::screens::shared::{Ring, thousands};
use crate::widgets::inventory_window::scene::CellView;
use crate::widgets::inventory_window::{self, CAPACITY, cell_view};
use crate::widgets::shop_window::{ShopSession, ShopTab};

pub(super) struct TabView {
    pub tab: ShopTab,
    /// Lines in this tab's cart.
    pub lines: usize,
    pub active: bool,
}

/// What the right end of a stock row offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowControl {
    /// The stepper and Add button, starting at this quantity.
    Add(u32),
    /// Why the row cannot be added.
    Locked(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChipTone {
    Cart,
    Cards,
    Rare,
}

/// One stock row. `key` is the cart key: `nameid` on Buy, `inventory_index` on Sell.
pub(super) struct RowView {
    pub key: u32,
    pub cell: CellView,
    pub name: String,
    pub chip: Option<(String, ChipTone)>,
    pub sub: String,
    pub price: u32,
    pub control: RowControl,
}

/// The quick-action strip over the list.
pub(super) struct QuickView {
    pub title: &'static str,
    pub text: String,
    pub label: &'static str,
    pub enabled: bool,
}

pub(super) struct ListView {
    pub buy: bool,
    pub quick: QuickView,
    pub rows: Vec<RowView>,
    /// Shown in place of the rows when there are none.
    pub message: Option<String>,
    pub scroll: Vec2,
}

pub(super) struct CartLineView {
    pub key: u32,
    pub cell: CellView,
    pub name: String,
    pub sub: String,
    pub qty: u32,
    /// The whole stack is in the cart, so it cannot grow.
    pub at_max: bool,
}

pub(super) struct CartView {
    pub buy: bool,
    pub lines: Vec<CartLineView>,
    /// The "Check before you sell" lines, empty unless the warning is up.
    pub warnings: Vec<String>,
    /// Label, value and value colour of each total.
    pub summary: Vec<(&'static str, String, Color)>,
    pub error: Option<&'static str>,
    pub cta: String,
    pub cta_enabled: bool,
    pub scroll: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FooterView {
    pub zeny: u32,
    pub weight: u32,
    pub max_weight: u32,
    /// Bag slots in use.
    pub bag: usize,
}

fn name(item_db: Option<&ItemDb>, nameid: u32, identified: bool) -> String {
    item_db
        .and_then(|db| db.name(nameid, identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{nameid}"))
}

/// A slot showing only the item's icon.
fn plain_cell(nameid: u32, identified: bool, item_db: Option<&ItemDb>) -> CellView {
    CellView {
        index: 0,
        icon: item_db
            .and_then(|db| db.icon_resource(nameid, identified))
            .map(item_icon_path),
        amount: 1,
        refine: 0,
        identified,
        damaged: false,
        favorite: false,
        pips: Vec::new(),
        ring: Ring::None,
    }
}

/// The slot and name of a Sell line, read from the live bag because the server's
/// snapshot carries no identify, refine or card state. A slot that left the bag is
/// drawn unidentified, so a stale snapshot cannot leak a name.
fn bag_cell(
    slot: &ShopSellItem,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> (CellView, String) {
    match inventory.get(slot.inventory_index as u16) {
        Some(item) => (
            cell_view(item, item_db, None),
            inventory_window::item_name(item, item_db),
        ),
        None => (
            plain_cell(slot.nameid, false, item_db),
            name(item_db, slot.nameid, false),
        ),
    }
}

fn cart_qty(session: &ShopSession, key: u32) -> u32 {
    let cart = match session.tab {
        ShopTab::Buy => &session.cart_buy,
        ShopTab::Sell => &session.cart_sell,
    };
    cart.get(&key).copied().unwrap_or(0)
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

pub(super) fn tab_views(session: &ShopSession) -> [TabView; 2] {
    [
        (ShopTab::Buy, &session.cart_buy),
        (ShopTab::Sell, &session.cart_sell),
    ]
    .map(|(tab, cart)| TabView {
        tab,
        lines: cart.values().filter(|qty| **qty > 0).count(),
        active: tab == session.tab,
    })
}

fn buy_rows(
    session: &ShopSession,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<RowView> {
    session
        .buy_items
        .iter()
        .map(|stock| {
            let in_cart = cart_qty(session, stock.nameid);
            let owned: u64 = inventory
                .stackables()
                .filter(|item| item.item_id == stock.nameid)
                .map(|item| u64::from(item.amount))
                .sum();
            RowView {
                key: stock.nameid,
                cell: plain_cell(stock.nameid, true, item_db),
                name: name(item_db, stock.nameid, true),
                chip: (in_cart > 0).then(|| {
                    (
                        format!("{} in cart", thousands(in_cart.into())),
                        ChipTone::Cart,
                    )
                }),
                sub: if owned > 0 {
                    format!("You have {}", thousands(owned))
                } else {
                    String::new()
                },
                price: stock.price,
                control: RowControl::Add(1),
            }
        })
        .collect()
}

fn sell_row(
    slot: &ShopSellItem,
    session: &ShopSession,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> RowView {
    let (cell, name) = bag_cell(slot, inventory, item_db);
    let in_cart = cart_qty(session, slot.inventory_index);
    let left = slot.amount.saturating_sub(in_cart);
    let cards = cell.pips.iter().filter(|filled| **filled).count();
    let chip = if in_cart > 0 {
        Some((
            format!("{} to sell", thousands(in_cart.into())),
            ChipTone::Cart,
        ))
    } else if cards > 0 {
        Some((
            format!("{cards} card{} inside", plural(cards)),
            ChipTone::Cards,
        ))
    } else if cell.ring == Ring::Rare {
        Some(("Rare".to_string(), ChipTone::Rare))
    } else {
        None
    };
    let mut sub = format!("You have {}", thousands(slot.amount.into()));
    if slot.amount > 1 {
        let all = u64::from(slot.sell_price) * u64::from(slot.amount);
        sub.push_str(&format!(" \u{b7} All for {} z", thousands(all)));
    }
    let control = if cell.favorite {
        RowControl::Locked("Favorite")
    } else if left == 0 {
        RowControl::Locked("All added")
    } else {
        RowControl::Add(left)
    };
    RowView {
        key: slot.inventory_index,
        cell,
        name,
        chip,
        sub,
        price: slot.sell_price,
        control,
    }
}

/// The strip over the list. On Sell it offers every plain loot stack in one click.
fn quick_view(session: &ShopSession, inventory: &Inventory, item_db: Option<&ItemDb>) -> QuickView {
    if session.tab == ShopTab::Buy {
        // NOTE: Restock tops the bag up to a saved shopping list, which the client does
        // not keep yet. Enable the strip once that list exists.
        return QuickView {
            title: "RESTOCK",
            text: "No restock list saved yet".to_string(),
            label: "Add to Cart",
            enabled: false,
        };
    }
    let loot: Vec<&ShopSellItem> = session.loot(inventory).collect();
    let text = if loot.is_empty() {
        "No loot left to sell \u{b7} Cards and favorites are skipped".to_string()
    } else {
        let names: Vec<String> = loot
            .iter()
            .map(|slot| bag_cell(slot, inventory, item_db).1)
            .collect();
        let gain: u64 = loot
            .iter()
            .map(|slot| {
                let left = slot
                    .amount
                    .saturating_sub(cart_qty(session, slot.inventory_index));
                u64::from(slot.sell_price) * u64::from(left)
            })
            .sum();
        format!("{} \u{b7} {} z", names.join(", "), thousands(gain))
    };
    QuickView {
        title: "LOOT",
        text,
        label: "Add All Loot",
        enabled: !loot.is_empty(),
    }
}

/// The active tab's rows, filtered by the search text, under the quick strip.
pub(super) fn list_view(
    session: &ShopSession,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    scroll: Vec2,
) -> ListView {
    let needle = session.query.to_lowercase();
    let mut rows = match session.tab {
        ShopTab::Buy => buy_rows(session, inventory, item_db),
        ShopTab::Sell => session
            .sell_items
            .iter()
            .map(|slot| sell_row(slot, session, inventory, item_db))
            .collect(),
    };
    rows.retain(|row| row.name.to_lowercase().contains(&needle));
    let message = match (rows.is_empty(), session.query.is_empty()) {
        (false, _) => None,
        (true, true) => Some("Nothing here".to_string()),
        (true, false) => Some(format!("Nothing matches \u{201c}{}\u{201d}", session.query)),
    };
    ListView {
        buy: session.tab == ShopTab::Buy,
        quick: quick_view(session, inventory, item_db),
        rows,
        message,
        scroll,
    }
}

fn cart_lines(
    session: &ShopSession,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<CartLineView> {
    match session.tab {
        ShopTab::Buy => session
            .buy_items
            .iter()
            .filter_map(|stock| {
                let qty = cart_qty(session, stock.nameid);
                let total = u64::from(stock.price) * u64::from(qty);
                (qty > 0).then(|| CartLineView {
                    key: stock.nameid,
                    cell: plain_cell(stock.nameid, true, item_db),
                    name: name(item_db, stock.nameid, true),
                    sub: format!("{} z", thousands(total)),
                    qty,
                    at_max: false,
                })
            })
            .collect(),
        ShopTab::Sell => session
            .sell_items
            .iter()
            .filter_map(|slot| {
                let qty = cart_qty(session, slot.inventory_index);
                let total = u64::from(slot.sell_price) * u64::from(qty);
                (qty > 0).then(|| {
                    let (cell, name) = bag_cell(slot, inventory, item_db);
                    CartLineView {
                        key: slot.inventory_index,
                        cell,
                        name,
                        sub: format!("+{} z", thousands(total)),
                        qty,
                        at_max: qty >= slot.amount,
                    }
                })
            })
            .collect(),
    }
}

/// Why each Sell cart line deserves a second look: its cards and refine levels go
/// with the item.
pub(super) fn sell_warnings(
    session: &ShopSession,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<String> {
    session
        .sell_items
        .iter()
        .filter(|slot| {
            session
                .cart_sell
                .get(&slot.inventory_index)
                .is_some_and(|qty| *qty > 0)
        })
        .filter_map(|slot| {
            let item = inventory.get(slot.inventory_index as u16)?;
            let cell = cell_view(item, item_db, None);
            let name = inventory_window::item_name(item, item_db);
            let cards = cell.pips.iter().filter(|filled| **filled).count();
            if cards > 0 {
                return Some(format!("{name} has {cards} card{} inside", plural(cards)));
            }
            (item.refine > 0).then(|| format!("{name} is refined to +{}", item.refine))
        })
        .collect()
}

/// Whether the active tab's cart is non-empty and, on the Buy tab, affordable.
/// [`on_shop_button`](super::on_shop_button) checks it again on submit, since a click
/// can race the redraw that disables the button.
pub(super) fn cta_enabled(session: &ShopSession, zeny: u32) -> bool {
    let cart_empty = match session.tab {
        ShopTab::Buy => session.cart_buy.is_empty(),
        ShopTab::Sell => session.cart_sell.is_empty(),
    };
    if cart_empty {
        return false;
    }
    match session.tab {
        ShopTab::Buy => session.can_afford(zeny),
        ShopTab::Sell => true,
    }
}

/// Only the Buy tab can run short: a sell cart earns zeny.
fn short_of_zeny(session: &ShopSession, zeny: u32) -> bool {
    session.tab == ShopTab::Buy && !session.can_afford(zeny)
}

/// The submit label: the bare verb for an empty cart, else the verb and the total.
fn cta_label(buy: bool, lines: usize, total: u64) -> String {
    let verb = if buy { "Buy" } else { "Sell" };
    if lines == 0 {
        return verb.to_string();
    }
    format!("{verb} for {} z", thousands(total))
}

fn summary(
    session: &ShopSession,
    zeny: u32,
    bag: usize,
    sold_out: usize,
) -> Vec<(&'static str, String, Color)> {
    if session.tab == ShopTab::Buy {
        let cost = session.buy_subtotal();
        let (after, ink) = match u64::from(zeny).checked_sub(cost) {
            Some(left) => (format!("{} z", thousands(left)), ro_dark::STRONG),
            None => (
                format!("-{} z", thousands(cost - u64::from(zeny))),
                ro_dark::STATUS_FULL,
            ),
        };
        return vec![
            (
                "Total",
                format!("{} z", thousands(cost)),
                ro_dark::BRONZE_LIGHT,
            ),
            ("Zeny after", after, ink),
        ];
    }
    let gain = session.sell_subtotal();
    vec![
        (
            "You receive",
            format!("+{} z", thousands(gain)),
            ro_dark::STATUS_ONLINE,
        ),
        (
            "Zeny after",
            format!("{} z", thousands(u64::from(zeny) + gain)),
            ro_dark::STRONG,
        ),
        (
            "Bag after",
            format!("{} / {CAPACITY}", bag.saturating_sub(sold_out)),
            ro_dark::STRONG,
        ),
    ]
}

/// The cart column. `pending_deal` holds the submit button while an earlier deal is
/// still unanswered.
pub(super) fn cart_view(
    session: &ShopSession,
    zeny: u32,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    pending_deal: bool,
    scroll: Vec2,
) -> CartView {
    let buy = session.tab == ShopTab::Buy;
    let lines = cart_lines(session, inventory, item_db);
    let total = if buy {
        session.buy_subtotal()
    } else {
        session.sell_subtotal()
    };
    let sold_out = lines.iter().filter(|line| line.at_max).count();
    let warnings = if session.confirm_open {
        sell_warnings(session, inventory, item_db)
    } else {
        Vec::new()
    };
    let cta = if pending_deal {
        "Waiting for previous deal".to_string()
    } else {
        cta_label(buy, lines.len(), total)
    };
    CartView {
        buy,
        summary: summary(session, zeny, inventory.len(), sold_out),
        error: short_of_zeny(session, zeny).then_some("Not enough zeny."),
        cta,
        cta_enabled: cta_enabled(session, zeny) && !pending_deal && !session.confirm_open,
        lines,
        warnings,
        scroll,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use lifthrasir_client::domain::inventory::Item;
    use lifthrasir_data::{ItemData, ItemInfo};
    use net_contract::dto::ShopBuyItem;

    use super::*;

    fn session_with(tab: ShopTab) -> ShopSession {
        ShopSession {
            unit_id: 1,
            buy_items: vec![ShopBuyItem {
                nameid: 501,
                price: 50,
            }],
            sell_items: vec![ShopSellItem {
                inventory_index: 0,
                nameid: 501,
                amount: 5,
                sell_price: 20,
            }],
            tab,
            cart_buy: HashMap::new(),
            cart_sell: HashMap::new(),
            query: String::new(),
            confirm_open: false,
            retired: false,
        }
    }

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Blade".to_string(),
                unidentified_name: "Sword".to_string(),
                slot_count: 3,
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    fn bag_with(item: Item) -> Inventory {
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 0,
            item_id: 501,
            amount: 5,
            identified: true,
            ..item
        });
        inventory
    }

    fn sell_rows(session: &ShopSession, inventory: &Inventory) -> Vec<RowView> {
        list_view(session, inventory, Some(&item_db()), Vec2::ZERO).rows
    }

    #[test]
    fn cta_disabled_when_cart_empty() {
        let session = session_with(ShopTab::Buy);
        assert!(!cta_enabled(&session, 1000));
    }

    #[test]
    fn cta_enabled_when_buy_cart_affordable() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 2);
        assert!(cta_enabled(&session, 100));
    }

    #[test]
    fn cta_disabled_when_buy_cart_unaffordable() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        assert!(!cta_enabled(&session, 100));
    }

    #[test]
    fn cta_enabled_on_sell_regardless_of_zeny() {
        let mut session = session_with(ShopTab::Sell);
        session.cart_sell.insert(0, 5);
        assert!(cta_enabled(&session, 0));
    }

    #[test]
    fn cta_label_names_the_total_once_the_cart_has_lines() {
        assert_eq!(cta_label(true, 0, 0), "Buy");
        assert_eq!(cta_label(false, 0, 0), "Sell");
        assert_eq!(cta_label(true, 2, 1500), "Buy for 1,500 z");
        assert_eq!(cta_label(false, 1, 20), "Sell for 20 z");
    }

    #[test]
    fn only_an_unaffordable_buy_cart_is_short_of_zeny() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        assert!(short_of_zeny(&session, 100));
        assert!(!short_of_zeny(&session, 1000));
        session.tab = ShopTab::Sell;
        session.cart_sell.insert(0, 5);
        assert!(!short_of_zeny(&session, 0));
    }

    #[test]
    fn buy_cart_totals_the_cost_and_flags_a_shortfall() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        let view = cart_view(
            &session,
            100,
            &Inventory::default(),
            None,
            false,
            Vec2::ZERO,
        );
        assert_eq!(view.cta, "Buy for 150 z");
        assert!(!view.cta_enabled);
        assert_eq!(view.error, Some("Not enough zeny."));
        let values: Vec<&str> = view.summary.iter().map(|row| row.1.as_str()).collect();
        assert_eq!(values, ["150 z", "-50 z"]);
    }

    #[test]
    fn sell_cart_counts_emptied_slots_and_waits_for_a_pending_deal() {
        let mut session = session_with(ShopTab::Sell);
        session.cart_sell.insert(0, 5);
        let inventory = bag_with(Item::default());
        let view = cart_view(&session, 10, &inventory, None, true, Vec2::ZERO);
        assert!(view.lines[0].at_max);
        assert!(!view.cta_enabled);
        assert_eq!(view.cta, "Waiting for previous deal");
        let values: Vec<&str> = view.summary.iter().map(|row| row.1.as_str()).collect();
        assert_eq!(values, ["+100 z", "110 z", "0 / 100"]);
    }

    #[test]
    fn sell_row_offers_what_is_left_of_the_stack() {
        let mut session = session_with(ShopTab::Sell);
        let inventory = bag_with(Item::default());
        assert_eq!(
            sell_rows(&session, &inventory)[0].control,
            RowControl::Add(5)
        );

        session.cart_sell.insert(0, 2);
        let row = &sell_rows(&session, &inventory)[0];
        assert_eq!(row.control, RowControl::Add(3));
        assert_eq!(row.chip, Some(("2 to sell".to_string(), ChipTone::Cart)));

        session.cart_sell.insert(0, 5);
        assert_eq!(
            sell_rows(&session, &inventory)[0].control,
            RowControl::Locked("All added")
        );
    }

    #[test]
    fn favorites_cannot_be_added_to_a_sale() {
        let session = session_with(ShopTab::Sell);
        let inventory = bag_with(Item {
            favorite: true,
            ..Default::default()
        });
        assert_eq!(
            sell_rows(&session, &inventory)[0].control,
            RowControl::Locked("Favorite")
        );
    }

    #[test]
    fn sell_row_names_follow_the_bag_and_never_leak_a_stale_slot() {
        let session = session_with(ShopTab::Sell);
        let refined = bag_with(Item {
            refine: 7,
            cards: [4001, 0, 0, 0],
            ..Default::default()
        });
        let row = &sell_rows(&session, &refined)[0];
        assert_eq!(row.name, "+7 Blade [3]");
        assert_eq!(row.cell.ring, Ring::Rare);
        assert_eq!(
            row.chip,
            Some(("1 card inside".to_string(), ChipTone::Cards))
        );

        let gone = Inventory::default();
        let row = &sell_rows(&session, &gone)[0];
        assert_eq!(row.name, "Sword");
        assert!(!row.cell.identified);
    }

    #[test]
    fn search_filters_rows_by_name() {
        let mut session = session_with(ShopTab::Buy);
        session.query = "bla".to_string();
        let view = list_view(
            &session,
            &Inventory::default(),
            Some(&item_db()),
            Vec2::ZERO,
        );
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.message, None);

        session.query = "potion".to_string();
        let view = list_view(
            &session,
            &Inventory::default(),
            Some(&item_db()),
            Vec2::ZERO,
        );
        assert!(view.rows.is_empty());
        assert_eq!(
            view.message.as_deref(),
            Some("Nothing matches \u{201c}potion\u{201d}")
        );
    }

    #[test]
    fn buy_row_shows_what_the_bag_already_holds() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 1200);
        let inventory = bag_with(Item::default());
        let row = &list_view(&session, &inventory, Some(&item_db()), Vec2::ZERO).rows[0];
        assert_eq!(row.sub, "You have 5");
        assert_eq!(
            row.chip,
            Some(("1,200 in cart".to_string(), ChipTone::Cart))
        );
        assert_eq!(row.control, RowControl::Add(1));
    }

    #[test]
    fn sell_warnings_cover_refined_and_carded_gear_in_the_cart_only() {
        let mut session = session_with(ShopTab::Sell);
        let db = item_db();
        let refined = bag_with(Item {
            refine: 4,
            ..Default::default()
        });
        assert!(sell_warnings(&session, &refined, Some(&db)).is_empty());

        session.cart_sell.insert(0, 1);
        assert_eq!(
            sell_warnings(&session, &refined, Some(&db)),
            ["+4 Blade [3] is refined to +4"]
        );
        let carded = bag_with(Item {
            cards: [4001, 4002, 0, 0],
            ..Default::default()
        });
        assert_eq!(
            sell_warnings(&session, &carded, Some(&db)),
            ["Blade [3] has 2 cards inside"]
        );
        assert!(sell_warnings(&session, &bag_with(Item::default()), Some(&db)).is_empty());
    }

    #[test]
    fn loot_strip_is_idle_on_buy_and_lists_plain_loot_on_sell() {
        let mut session = session_with(ShopTab::Buy);
        let mut loot = Inventory::default();
        loot.upsert(Item {
            index: 0,
            item_id: 909,
            item_type: 3,
            amount: 5,
            identified: true,
            ..Default::default()
        });
        session.sell_items[0].nameid = 909;
        let quick = list_view(&session, &loot, None, Vec2::ZERO).quick;
        assert_eq!(quick.title, "RESTOCK");
        assert!(!quick.enabled);

        session.tab = ShopTab::Sell;
        let quick = list_view(&session, &loot, None, Vec2::ZERO).quick;
        assert!(quick.enabled);
        assert_eq!(quick.text, "Item #909 \u{b7} 100 z");

        session.cart_sell.insert(0, 5);
        assert!(!list_view(&session, &loot, None, Vec2::ZERO).quick.enabled);
    }
}
