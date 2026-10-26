//! Buyer basket: staged listing quantities against one particular shop.
//!
//! Holds no authoritative inventory or zeny: quantities are staged locally and
//! every total is computed from the displayed snapshot, so a purchase is always
//! confirmed against the quantities and quoted prices the buyer actually saw.

use std::collections::HashSet;

use bevy::prelude::*;
use net_contract::dto::{VendingBuyLine, VendingListing, VendingSnapshot};

/// One staged line: the opaque listing identity and its quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasketLine {
    pub index: u32,
    pub amount: u32,
}

/// The buyer's staged quantities, bound to one shop identity at a time.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct BuyerBasket {
    vendor_unit_id: Option<u64>,
    shop_id: Option<String>,
    lines: Vec<BasketLine>,
}

impl BuyerBasket {
    /// Binds the basket to a shop, clearing it when the identity differs.
    pub fn bind(&mut self, vendor_unit_id: u64, shop_id: &str) {
        if self.vendor_unit_id == Some(vendor_unit_id) && self.shop_id.as_deref() == Some(shop_id) {
            return;
        }
        self.vendor_unit_id = Some(vendor_unit_id);
        self.shop_id = Some(shop_id.to_string());
        self.lines.clear();
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn lines(&self) -> &[BasketLine] {
        &self.lines
    }

    pub fn quantity(&self, index: u32) -> u32 {
        self.lines
            .iter()
            .find(|line| line.index == index)
            .map_or(0, |line| line.amount)
    }

    /// The basket total for the displayed shop, or `None` when a line refers to
    /// a listing this snapshot does not carry or the sum overflows.
    pub fn total(&self, snapshot: &VendingSnapshot) -> Option<u64> {
        self.lines.iter().try_fold(0u64, |total, line| {
            let listing = listing(snapshot, line.index)?;
            let line_total = u64::from(listing.price).checked_mul(u64::from(line.amount))?;
            total.checked_add(line_total)
        })
    }

    /// The confirmation rows, carrying each displayed quoted unit price.
    pub fn to_buy_lines(&self, snapshot: &VendingSnapshot) -> Vec<VendingBuyLine> {
        self.lines
            .iter()
            .filter_map(|line| {
                let listing = listing(snapshot, line.index)?;
                Some(VendingBuyLine {
                    index: line.index,
                    amount: line.amount,
                    expected_unit_price: listing.price,
                })
            })
            .collect()
    }

    /// Sets one line's quantity, clamped to the available stock; zero removes it.
    pub fn set_quantity(&mut self, snapshot: &VendingSnapshot, index: u32, amount: u32) {
        let Some(listing) = listing(snapshot, index) else {
            self.lines.retain(|line| line.index != index);
            return;
        };
        let amount = amount.min(listing.amount);
        self.lines.retain(|line| line.index != index);
        if amount > 0 {
            self.lines.push(BasketLine { index, amount });
        }
    }

    /// Steps one line up or down inside `1..=stock`.
    pub fn step(&mut self, snapshot: &VendingSnapshot, index: u32, up: bool) {
        let Some(listing) = listing(snapshot, index) else {
            return;
        };
        let current = self.quantity(index);
        if up {
            let next = current.checked_add(1).unwrap_or(current);
            if next <= listing.amount {
                self.set_quantity(snapshot, index, next);
            }
        } else if current > 0 {
            self.set_quantity(snapshot, index, current - 1);
        }
    }

    /// Re-applies the basket to a freshly loaded snapshot of the same shop:
    /// vanished rows and changed prices are dropped, reduced stock is clamped,
    /// and every change is explained. Nothing is re-priced or re-confirmed.
    pub fn reconcile(
        &mut self,
        previous: &VendingSnapshot,
        current: &VendingSnapshot,
    ) -> Vec<String> {
        let mut messages = Vec::new();
        let mut seen = HashSet::new();
        let mut kept = Vec::new();
        for line in &self.lines {
            let Some(before) = listing(previous, line.index) else {
                continue;
            };
            let Some(after) = listing(current, line.index) else {
                messages.push(format!("{} is no longer for sale.", item_label(before)));
                continue;
            };
            if after.price != before.price {
                messages.push(format!(
                    "{} now costs {}z instead of {}z; it was removed from your basket.",
                    item_label(before),
                    after.price,
                    before.price
                ));
                continue;
            }
            if after.amount == 0 {
                messages.push(format!("{} sold out.", item_label(before)));
                continue;
            }
            if !seen.insert(line.index) {
                continue;
            }
            let amount = line.amount.min(after.amount);
            if amount < line.amount {
                messages.push(format!(
                    "Only {} of {} remain.",
                    after.amount,
                    item_label(before)
                ));
            }
            kept.push(BasketLine {
                index: line.index,
                amount,
            });
        }
        self.lines = kept;
        messages
    }

    /// Short guidance when the basket costs more than the buyer holds.
    pub fn guidance(&self, snapshot: &VendingSnapshot, zeny: u64) -> Option<String> {
        let total = self.total(snapshot)?;
        (total > zeny).then(|| format!("You need {} more zeny.", total - zeny))
    }
}

fn listing(snapshot: &VendingSnapshot, index: u32) -> Option<&VendingListing> {
    snapshot.items.iter().find(|item| item.index == index)
}

fn item_label(item: &VendingListing) -> String {
    format!("Item #{}", item.nameid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::VendingCreatorKind;

    const VENDOR: u64 = 5;
    const SHOP: &str = "shop-1";

    fn listing(index: u32, nameid: u32, amount: u32, price: u32) -> VendingListing {
        VendingListing {
            index,
            nameid,
            type_: 0,
            amount,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            options: vec![],
            price,
            signer_name: String::new(),
            creator_id: 0,
            creator_kind: VendingCreatorKind::None,
            enchant_grade: 0,
        }
    }

    fn snapshot(items: Vec<VendingListing>) -> VendingSnapshot {
        VendingSnapshot {
            vendor_unit_id: VENDOR,
            shop_id: SHOP.into(),
            title: "Potions".into(),
            items,
            total_earned: 0,
        }
    }

    fn shop() -> VendingSnapshot {
        snapshot(vec![
            listing(2, 501, 3, 500),
            listing(3, 501, 1, 500),
            listing(4, 2104, 2, 0),
        ])
    }

    #[test]
    fn basket_identity_uses_the_opaque_listing_index() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();
        basket.step(&shop, 2, true);
        basket.step(&shop, 3, true);

        assert_eq!(basket.lines().len(), 2, "same nameid stays two listings");
        assert_eq!(basket.quantity(2), 1);
        assert_eq!(basket.quantity(3), 1);
    }

    #[test]
    fn quantities_stay_inside_the_available_stock() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();

        for _ in 0..5 {
            basket.step(&shop, 2, true);
        }
        assert_eq!(basket.quantity(2), 3, "capped at the listed amount");

        basket.step(&shop, 2, false);
        assert_eq!(basket.quantity(2), 2);
        basket.set_quantity(&shop, 2, 99);
        assert_eq!(basket.quantity(2), 3);
        basket.set_quantity(&shop, 2, 0);
        assert_eq!(basket.quantity(2), 0, "zero removes the line");
        assert!(basket.is_empty());
    }

    #[test]
    fn zero_price_stock_totals_zero_and_is_affordable() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();
        basket.step(&shop, 4, true);

        assert_eq!(basket.total(&shop), Some(0));
        assert_eq!(basket.guidance(&shop, 0), None);
    }

    #[test]
    fn totals_and_quotes_use_the_displayed_prices() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();
        basket.step(&shop, 2, true);
        basket.step(&shop, 2, true);
        basket.step(&shop, 4, true);

        assert_eq!(basket.total(&shop), Some(1_000));
        let lines = basket.to_buy_lines(&shop);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            (
                lines[0].index,
                lines[0].amount,
                lines[0].expected_unit_price
            ),
            (2, 2, 500)
        );
        assert_eq!(
            (
                lines[1].index,
                lines[1].amount,
                lines[1].expected_unit_price
            ),
            (4, 1, 0)
        );
    }

    #[test]
    fn a_missing_listing_or_overflowing_total_is_not_a_valid_total() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let expensive = snapshot(vec![listing(2, 501, 1, u32::MAX)]);
        basket.set_quantity(&expensive, 2, 1);
        assert_eq!(basket.total(&expensive), Some(u64::from(u32::MAX)));

        let missing = snapshot(vec![listing(9, 501, 1, 500)]);
        assert_eq!(basket.total(&missing), None, "line is not in this shop");
        assert!(basket.to_buy_lines(&missing).is_empty());
    }

    #[test]
    fn guidance_explains_how_much_more_zeny_is_needed() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();
        basket.step(&shop, 2, true);

        assert_eq!(basket.guidance(&shop, 500), None);
        assert_eq!(
            basket.guidance(&shop, 200),
            Some("You need 300 more zeny.".to_string())
        );
    }

    #[test]
    fn binding_to_another_shop_never_carries_a_basket_across() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let shop = shop();
        basket.step(&shop, 2, true);

        basket.bind(VENDOR, SHOP);
        assert_eq!(basket.quantity(2), 1, "same shop keeps the basket");

        basket.bind(VENDOR, "shop-2");
        assert!(basket.is_empty(), "another shop starts empty");
    }

    #[test]
    fn reconcile_drops_changed_rows_clamps_stock_and_explains() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let before = shop();
        basket.set_quantity(&before, 2, 3);
        basket.set_quantity(&before, 3, 1);
        basket.set_quantity(&before, 4, 2);

        let after = snapshot(vec![
            listing(2, 501, 1, 500),
            listing(3, 501, 1, 750),
            listing(4, 2104, 0, 0),
        ]);
        let messages = basket.reconcile(&before, &after);

        assert_eq!(basket.quantity(2), 1, "clamped to remaining stock");
        assert_eq!(basket.quantity(3), 0, "changed price drops the row");
        assert_eq!(basket.quantity(4), 0, "sold-out row is dropped");
        assert_eq!(messages.len(), 3);
        assert!(messages[0].contains("Only 1 of Item #501 remain"));
        assert!(messages[1].contains("now costs 750z instead of 500z"));
        assert!(messages[2].contains("Item #2104 sold out"), "{messages:?}");
    }

    #[test]
    fn reconcile_is_silent_when_nothing_changed() {
        let mut basket = BuyerBasket::default();
        basket.bind(VENDOR, SHOP);
        let before = shop();
        basket.set_quantity(&before, 2, 2);

        let messages = basket.reconcile(&before, &before);
        assert!(messages.is_empty());
        assert_eq!(basket.quantity(2), 2);
    }
}
