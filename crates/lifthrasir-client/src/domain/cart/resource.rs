use bevy::prelude::*;
use net_contract::dto::CartItem;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub struct Cart {
    items: BTreeMap<u16, CartItem>,
    current_weight: u32,
    max_weight: u32,
    ready: bool,
}

impl Cart {
    pub fn begin(&mut self) {
        self.items.clear();
        self.ready = false;
    }

    pub fn upsert(&mut self, item: CartItem) {
        self.items.insert(item.index as u16, item);
    }

    pub fn finish(&mut self) {
        self.ready = true;
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn iter(&self) -> impl Iterator<Item = &CartItem> {
        self.items.values()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, index: u16) -> Option<&CartItem> {
        self.items.get(&index)
    }

    pub fn remove_amount(&mut self, index: u16, amount: u16) {
        let Some(item) = self.items.get_mut(&index) else {
            return;
        };
        item.amount = item.amount.saturating_sub(amount as u32);
        if item.amount == 0 {
            self.items.remove(&index);
        }
    }

    pub fn current_weight(&self) -> u32 {
        self.current_weight
    }

    pub fn max_weight(&self) -> u32 {
        self.max_weight
    }

    pub fn set_weights(&mut self, current_weight: u32, max_weight: u32) {
        self.current_weight = current_weight;
        self.max_weight = max_weight;
    }

    pub fn add_weight(&mut self, weight: u32) {
        self.current_weight += weight;
    }

    pub fn sub_weight(&mut self, weight: u32) {
        self.current_weight = self.current_weight.saturating_sub(weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(index: u32, amount: u32, weight: u32) -> CartItem {
        CartItem {
            nameid: 501,
            index,
            amount,
            identified: true,
            refine: 0,
            cards: vec![],
            attribute: 0,
            expire_time: 0,
            weight,
        }
    }

    #[test]
    fn begin_clears_items_and_marks_not_ready() {
        let mut cart = Cart::default();
        cart.upsert(item(2, 5, 3));
        cart.finish();
        assert!(cart.is_ready());

        cart.begin();

        assert_eq!(cart.len(), 0);
        assert!(!cart.is_ready());
    }

    #[test]
    fn upsert_inserts_then_overwrites_by_index() {
        let mut cart = Cart::default();
        cart.upsert(item(2, 5, 3));
        assert_eq!(cart.get(2).unwrap().amount, 5);

        cart.upsert(item(2, 9, 3));

        assert_eq!(cart.len(), 1);
        assert_eq!(cart.get(2).unwrap().amount, 9);
    }

    #[test]
    fn finish_marks_ready() {
        let mut cart = Cart::default();
        assert!(!cart.is_ready());

        cart.finish();

        assert!(cart.is_ready());
    }

    #[test]
    fn remove_amount_decrements_leaving_positive_remainder() {
        let mut cart = Cart::default();
        cart.upsert(item(5, 10, 3));

        cart.remove_amount(5, 3);

        assert_eq!(cart.get(5).unwrap().amount, 7);
        assert_eq!(cart.len(), 1);
    }

    #[test]
    fn remove_amount_drops_slot_when_reaching_zero() {
        let mut cart = Cart::default();
        cart.upsert(item(5, 3, 3));

        cart.remove_amount(5, 3);

        assert!(cart.get(5).is_none());
        assert_eq!(cart.len(), 0);
    }

    #[test]
    fn remove_amount_missing_index_is_noop() {
        let mut cart = Cart::default();

        cart.remove_amount(99, 1);

        assert_eq!(cart.len(), 0);
    }

    #[test]
    fn set_weights_updates_current_and_max() {
        let mut cart = Cart::default();

        cart.set_weights(120, 8000);

        assert_eq!(cart.current_weight(), 120);
        assert_eq!(cart.max_weight(), 8000);
    }

    #[test]
    fn add_weight_increments_current_weight() {
        let mut cart = Cart::default();
        cart.set_weights(100, 8000);

        cart.add_weight(50);

        assert_eq!(cart.current_weight(), 150);
    }

    #[test]
    fn sub_weight_saturates_at_zero() {
        let mut cart = Cart::default();
        cart.set_weights(30, 8000);

        cart.sub_weight(100);

        assert_eq!(cart.current_weight(), 0);
    }
}
