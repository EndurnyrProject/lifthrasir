//! What each vending body draws, projected from the domain state. Every view is plain
//! owned data, so a body is redrawn only when its view changes and each rule here is
//! testable without spawning a scene.

use std::collections::HashMap;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::VendingSale;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{CartItem, VendingListing, VendingSetup, VendingSnapshot};

use super::buyer::BuyerBasket;
use super::{DraftFieldKind, SellerDraft, SetupReadout};
use crate::screens::shared::thousands;
use crate::widgets::inventory_window::LooseItem;
use crate::widgets::inventory_window::scene::CellView;

/// A basket costing this much or more asks the buyer to confirm it.
pub(super) const CONFIRM_FROM: u64 = 1_000_000;
/// The title count turns amber this close to the limit.
const TITLE_WARNING: usize = 10;

pub(super) fn zeny(amount: u64) -> String {
    format!("{} z", thousands(amount))
}

/// `n` in words once it is long enough to misread, as in `2 million`.
pub(super) fn words(n: u64) -> String {
    let (value, unit) = match n {
        1_000_000_000.. => (n as f64 / 1e9, "billion"),
        1_000_000.. => (n as f64 / 1e6, "million"),
        10_000.. => (n as f64 / 1e3, "thousand"),
        _ => return String::new(),
    };
    let value = format!("{value:.2}");
    format!(
        "{} {unit}",
        value.trim_end_matches('0').trim_end_matches('.')
    )
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// A cart slot as a row draws it: the amount is written beside the slot, not on it.
fn cart_slot(item: &CartItem) -> LooseItem<'_> {
    LooseItem {
        nameid: item.nameid,
        identified: item.identified,
        refine: item.refine,
        cards: &item.cards,
        amount: 1,
    }
}

fn listing_slot(item: &VendingListing) -> LooseItem<'_> {
    LooseItem {
        nameid: item.nameid,
        identified: item.identified,
        refine: item.refine,
        cards: &item.cards,
        amount: 1,
    }
}

/// The cards in an item, by name. An id the catalogue does not know is a maker's
/// mark, not a card, and is left out.
fn card_names(cards: &[u32], item_db: &ItemDb) -> Option<String> {
    let names: Vec<&str> = cards
        .iter()
        .filter_map(|&card| item_db.name(card, true))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// What the seller can do with one cart slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StockState {
    Listable,
    /// The shop already holds as many listings as the skill allows.
    Full,
    Listed,
    /// The server will not let this slot be sold.
    Locked,
}

pub(super) struct StockRowView {
    pub cart_index: u32,
    pub cell: CellView,
    pub name: String,
    pub sub: String,
    pub state: StockState,
}

/// One drafted listing with the text its fields start from.
pub(super) struct ListingRowView {
    pub cart_index: u32,
    pub cell: CellView,
    pub name: String,
    pub amount: String,
    pub price: String,
}

pub(super) struct SetupView {
    pub title: String,
    pub title_limit: u32,
    pub stock: Vec<StockRowView>,
    pub listings: Vec<ListingRowView>,
    pub max_slots: u32,
    pub sp_spent: u32,
}

pub(super) fn setup_view(
    setup: &VendingSetup,
    draft: &SellerDraft,
    cart: &Cart,
    item_db: &ItemDb,
) -> SetupView {
    let full = draft.rows.len() >= setup.max_slots as usize;
    let stock = cart
        .iter()
        .map(|item| {
            let state = if draft.row(item.index).is_some() {
                StockState::Listed
            } else if !setup.sellable_cart_indices.contains(&item.index) {
                StockState::Locked
            } else if full {
                StockState::Full
            } else {
                StockState::Listable
            };
            let slot = cart_slot(item);
            StockRowView {
                cart_index: item.index,
                cell: slot.cell(Some(item_db), false),
                name: slot.name(Some(item_db)),
                sub: format!("{} in cart", thousands(item.amount.into())),
                state,
            }
        })
        .collect();
    let listings = draft
        .rows
        .iter()
        .filter_map(|row| {
            let slot = cart_slot(cart.get(u16::try_from(row.cart_index).ok()?)?);
            Some(ListingRowView {
                cart_index: row.cart_index,
                cell: slot.cell(Some(item_db), false),
                name: slot.name(Some(item_db)),
                amount: row.amount.to_string(),
                price: row.price.map(|price| price.to_string()).unwrap_or_default(),
            })
        })
        .collect();
    SetupView {
        title: draft.title.clone(),
        title_limit: setup.max_title_bytes,
        stock,
        listings,
        max_slots: setup.max_slots,
        sp_spent: setup.sp_spent,
    }
}

/// What a listing's typed quantity and price amount to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ListingCheck {
    /// Why the listing cannot open as typed.
    pub problem: Option<String>,
    pub bad_amount: bool,
    pub bad_price: bool,
    /// What the listing earns if all of it sells, once both numbers are typed.
    pub total: Option<u64>,
}

pub(super) fn check_listing(
    amount: Option<u32>,
    price: Option<u32>,
    stock: u32,
    setup: &VendingSetup,
) -> ListingCheck {
    let bad_amount = !amount.is_some_and(|amount| (1..=stock).contains(&amount));
    let price_problem = match price {
        None => Some("Set a price".to_string()),
        Some(price) if price < setup.min_unit_price => {
            Some(format!("At least {}", zeny(setup.min_unit_price.into())))
        }
        Some(price) if price > setup.max_unit_price => {
            Some(format!("Max {}", zeny(setup.max_unit_price.into())))
        }
        Some(_) => None,
    };
    let bad_price = price_problem.is_some();
    let problem = if bad_amount {
        Some(format!("Quantity must be 1 to {}", thousands(stock.into())))
    } else {
        price_problem
    };
    ListingCheck {
        problem,
        bad_amount,
        bad_price,
        total: amount
            .zip(price)
            .map(|(amount, price)| u64::from(amount) * u64::from(price)),
    }
}

/// Every live read-out of the setup body, the fields that are wrong and whether the
/// shop can open.
pub(super) struct SetupLive {
    pub texts: HashMap<SetupReadout, (String, Color)>,
    pub bad_fields: Vec<(u32, DraftFieldKind)>,
    pub bad_title: bool,
    pub ready: bool,
}

/// What the seller has typed: the title, then each listing's cart slot, quantity and
/// price. An empty or unreadable field is `None`.
pub(super) struct TypedSetup<'a> {
    pub title: &'a str,
    pub rows: &'a [(u32, Option<u32>, Option<u32>)],
}

pub(super) fn setup_live(
    typed: TypedSetup,
    setup: &VendingSetup,
    cart: &Cart,
    item_db: &ItemDb,
    error: Option<&str>,
) -> SetupLive {
    let mut texts = HashMap::new();
    let mut bad_fields = Vec::new();
    let mut potential = 0u64;
    let mut bad_rows = 0;
    for &(cart_index, amount, price) in typed.rows {
        let item = u16::try_from(cart_index)
            .ok()
            .and_then(|slot| cart.get(slot));
        let stock = item.map_or(0, |item| item.amount);
        let check = check_listing(amount, price, stock, setup);
        if check.bad_amount {
            bad_fields.push((cart_index, DraftFieldKind::Amount));
        }
        if check.bad_price {
            bad_fields.push((cart_index, DraftFieldKind::Price));
        }
        bad_rows += usize::from(check.problem.is_some());
        potential += check.total.unwrap_or(0);

        let cards = item.and_then(|item| card_names(&item.cards, item_db));
        let in_cart = format!("{} in cart", thousands(stock.into()));
        let note = match (check.problem, cards) {
            (Some(problem), _) => (problem, ro_dark::STATUS_FULL),
            (None, Some(cards)) => (format!("{cards} \u{b7} {in_cart}"), ro_dark::MUTED),
            (None, None) => (in_cart, ro_dark::MUTED),
        };
        texts.insert(SetupReadout::Note(cart_index), note);
        texts.insert(
            SetupReadout::Words(cart_index),
            (words(price.unwrap_or(0).into()), ro_dark::MUTED),
        );
        let total = match check.total {
            Some(total) => (zeny(total), ro_dark::STRONG),
            None => ("\u{2014}".to_string(), ro_dark::MUTED),
        };
        texts.insert(SetupReadout::Total(cart_index), total);
    }

    let title = typed.title.trim();
    let limit = setup.max_title_bytes as usize;
    let bad_title = title.is_empty() || title.len() > limit;
    let count_color = if title.len() > limit {
        ro_dark::STATUS_FULL
    } else if title.len() + TITLE_WARNING > limit {
        ro_dark::STATUS_BUSY
    } else {
        ro_dark::MUTED
    };
    texts.insert(
        SetupReadout::TitleCount,
        (format!("{} / {limit}", title.len()), count_color),
    );
    let sign = if title.is_empty() {
        "Your shop name"
    } else {
        title
    };
    texts.insert(SetupReadout::Sign, (sign.to_string(), ro_dark::STRONG));
    texts.insert(
        SetupReadout::Potential,
        (zeny(potential), ro_dark::BRONZE_LIGHT),
    );

    let waiting = if title.is_empty() {
        Some("Give your shop a name".to_string())
    } else if title.len() > limit {
        Some("The shop name is too long".to_string())
    } else if typed.rows.is_empty() {
        Some("List at least one item".to_string())
    } else if bad_rows == 1 {
        Some("1 listing needs fixing".to_string())
    } else if bad_rows > 1 {
        Some(format!("{bad_rows} listings need fixing"))
    } else {
        None
    };
    let ready = waiting.is_none();
    let status = match (error, waiting) {
        (Some(error), _) => (error.to_string(), ro_dark::STATUS_FULL),
        (None, Some(waiting)) => (waiting, ro_dark::STATUS_FULL),
        (None, None) => ("Ready to open".to_string(), ro_dark::STATUS_ONLINE),
    };
    texts.insert(SetupReadout::Status, status);

    SetupLive {
        texts,
        bad_fields,
        bad_title,
        ready,
    }
}

/// One listing of the seller's open shop: what it held when the window first saw it
/// and how much of that is gone.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ShopStockView {
    pub index: u32,
    pub cell: CellView,
    pub name: String,
    pub sold: u32,
    pub total: u32,
    pub price: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SaleView {
    pub buyer: String,
    pub what: String,
    pub zeny: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct SellerView {
    pub title: String,
    pub stock: Vec<ShopStockView>,
    /// The latest reported sales, newest first.
    pub sales: Vec<SaleView>,
    pub earned: u64,
    pub zeny: u32,
    pub closing: bool,
    pub ask_close: bool,
    pub error: Option<String>,
}

impl SellerView {
    pub fn left(&self) -> u32 {
        self.stock.iter().map(|row| row.total - row.sold).sum()
    }

    pub fn selling(&self) -> usize {
        self.stock.iter().filter(|row| row.total > row.sold).count()
    }
}

/// The stock and sales of the seller's shop. `opening` holds each listing's amount
/// when the window first saw the shop, so what sold since can be told apart.
pub(super) fn seller_view(
    shop: &VendingSnapshot,
    opening: &[(u32, u32)],
    sales: &[VendingSale],
    item_db: &ItemDb,
) -> SellerView {
    let stock = shop
        .items
        .iter()
        .map(|item| {
            let total = opening
                .iter()
                .find(|(index, _)| *index == item.index)
                .map_or(item.amount, |(_, amount)| (*amount).max(item.amount));
            let slot = listing_slot(item);
            ShopStockView {
                index: item.index,
                cell: slot.cell(Some(item_db), false),
                name: slot.name(Some(item_db)),
                sold: total - item.amount,
                total,
                price: item.price,
            }
        })
        .collect();
    let sales = sales
        .iter()
        .rev()
        .filter(|sale| sale.shop_id == shop.shop_id)
        .map(|sale| {
            let name = item_db
                .name(sale.nameid, true)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Item #{}", sale.nameid));
            let what = if sale.amount > 1 {
                format!("{name} \u{d7}{}", thousands(sale.amount.into()))
            } else {
                name
            };
            SaleView {
                buyer: sale.buyer_name.clone(),
                what,
                zeny: sale.zeny_gained,
            }
        })
        .collect();
    SellerView {
        title: shop.title.clone(),
        stock,
        sales,
        earned: shop.total_earned,
        ..default()
    }
}

/// One listing of the shop being browsed.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BuyRowView {
    pub index: u32,
    pub cell: CellView,
    pub name: String,
    pub sub: String,
    pub price: u32,
    pub left: u32,
    pub in_cart: u32,
}

impl BuyRowView {
    /// How many more the buyer can still add.
    pub fn room(&self) -> u32 {
        self.left.saturating_sub(self.in_cart)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct BasketLineView {
    pub index: u32,
    pub cell: CellView,
    pub name: String,
    pub qty: u32,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct BuyerView {
    pub title: String,
    pub vendor: Option<String>,
    pub rows: Vec<BuyRowView>,
    pub lines: Vec<BasketLineView>,
    pub total: u64,
    pub zeny: u32,
    pub weight: u32,
    pub max_weight: u32,
    /// What a refreshed shop took out of the basket.
    pub change: Option<String>,
    pub error: Option<String>,
    /// The "check the zeros" prompt is up.
    pub confirm: bool,
    pub pending: bool,
}

impl BuyerView {
    pub fn short(&self) -> bool {
        self.total > u64::from(self.zeny)
    }

    pub fn can_buy(&self) -> bool {
        !self.lines.is_empty() && !self.short() && !self.confirm && !self.pending
    }
}

/// The listings and the basket of the shop being browsed.
pub(super) fn buyer_view(
    shop: &VendingSnapshot,
    basket: &BuyerBasket,
    item_db: &ItemDb,
) -> BuyerView {
    let rows = shop
        .items
        .iter()
        .map(|item| {
            let slot = listing_slot(item);
            let left = if item.amount == 0 {
                "Sold out".to_string()
            } else {
                format!("{} left", thousands(item.amount.into()))
            };
            let sub = match card_names(&item.cards, item_db) {
                Some(cards) => format!("{cards} \u{b7} {left}"),
                None => left,
            };
            BuyRowView {
                index: item.index,
                cell: slot.cell(Some(item_db), false),
                name: slot.name(Some(item_db)),
                sub,
                price: item.price,
                left: item.amount,
                in_cart: basket.quantity(item.index),
            }
        })
        .collect();
    let lines = basket
        .lines()
        .iter()
        .filter_map(|line| {
            let item = shop.items.iter().find(|item| item.index == line.index)?;
            let slot = listing_slot(item);
            Some(BasketLineView {
                index: line.index,
                cell: slot.cell(Some(item_db), false),
                name: slot.name(Some(item_db)),
                qty: line.amount,
                total: u64::from(item.price) * u64::from(line.amount),
            })
        })
        .collect();
    BuyerView {
        title: shop.title.clone(),
        rows,
        lines,
        total: basket.total(shop).unwrap_or(0),
        ..default()
    }
}

/// The count beside a list's caption, as in `7 listings`.
pub(super) fn counted(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", plural(count))
}

/// What a shop left behind when it closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClosedView {
    pub earned: u64,
    pub unsold: u32,
}

pub(super) fn closed_view(shop: &VendingSnapshot) -> ClosedView {
    ClosedView {
        earned: shop.total_earned,
        unsold: shop.items.iter().map(|item| item.amount).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(min: u32, max: u32) -> VendingSetup {
        VendingSetup {
            setup_id: "setup-1".into(),
            max_slots: 12,
            min_unit_price: min,
            max_unit_price: max,
            max_title_bytes: 79,
            sellable_cart_indices: vec![2],
            sp_spent: 30,
        }
    }

    #[test]
    fn words_spell_out_prices_long_enough_to_misread() {
        assert_eq!(words(9_999), "");
        assert_eq!(words(85_000), "85 thousand");
        assert_eq!(words(9_500_000), "9.5 million");
        assert_eq!(words(2_000_000), "2 million");
        assert_eq!(words(1_250_000_000), "1.25 billion");
    }

    #[test]
    fn a_listing_needs_a_typed_price_and_a_quantity_within_its_stock() {
        let live = setup(100, 5_000);
        let check = |amount, price| check_listing(amount, price, 3, &live);

        let unpriced = check(Some(3), None);
        assert_eq!(unpriced.problem.as_deref(), Some("Set a price"));
        assert!(unpriced.bad_price && !unpriced.bad_amount);
        assert_eq!(unpriced.total, None);

        assert_eq!(
            check(Some(1), Some(99)).problem.as_deref(),
            Some("At least 100 z")
        );
        assert_eq!(
            check(Some(1), Some(5_001)).problem.as_deref(),
            Some("Max 5,000 z")
        );
        for amount in [None, Some(0), Some(4)] {
            let check = check(amount, Some(500));
            assert_eq!(check.problem.as_deref(), Some("Quantity must be 1 to 3"));
            assert!(check.bad_amount && !check.bad_price);
        }

        let fine = check(Some(3), Some(500));
        assert_eq!(fine.problem, None);
        assert_eq!(fine.total, Some(1_500));
    }

    #[test]
    fn the_shop_is_ready_only_with_a_name_and_sound_listings() {
        let live = setup(0, 1_000_000_000);
        let mut cart = Cart::default();
        cart.upsert(CartItem {
            nameid: 501,
            index: 2,
            amount: 3,
            identified: true,
            refine: 0,
            cards: vec![],
            options: vec![],
            attribute: 0,
            expire_time: 0,
            weight: 10,
        });
        let item_db = ItemDb::default();
        let status = |title: &str, rows: &[(u32, Option<u32>, Option<u32>)], error| {
            let live = setup_live(TypedSetup { title, rows }, &live, &cart, &item_db, error);
            (live.texts[&SetupReadout::Status].0.clone(), live.ready)
        };
        let priced = [(2, Some(3), Some(500))];

        assert_eq!(
            status(" ", &priced, None),
            ("Give your shop a name".to_string(), false)
        );
        assert_eq!(
            status(&"x".repeat(80), &priced, None),
            ("The shop name is too long".to_string(), false)
        );
        assert_eq!(
            status("Potions", &[], None),
            ("List at least one item".to_string(), false)
        );
        assert_eq!(
            status("Potions", &[(2, Some(3), None)], None),
            ("1 listing needs fixing".to_string(), false)
        );
        assert_eq!(
            status("Potions", &priced, None),
            ("Ready to open".to_string(), true)
        );
        assert_eq!(
            status(
                "Potions",
                &priced,
                Some("You cannot open a shop right now.")
            ),
            ("You cannot open a shop right now.".to_string(), true),
            "a rejection is shown but does not lock the button"
        );
    }
}
