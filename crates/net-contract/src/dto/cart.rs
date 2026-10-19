//! Protocol-neutral cart item type.

/// One cart slot, mirroring the inventory item shape plus a per-unit `weight`
/// so the `Cart` resource can adjust its weight aggregate on removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartItem {
    pub nameid: u32,
    pub index: u32,
    pub amount: u32,
    pub identified: bool,
    pub refine: u32,
    pub cards: Vec<u32>,
    pub attribute: u32,
    pub expire_time: u64,
    pub weight: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cart_item_round_trips_through_clone_and_equality() {
        let item = CartItem {
            nameid: 501,
            index: 3,
            amount: 2,
            identified: true,
            refine: 4,
            cards: vec![4001, 4002],
            attribute: 0,
            expire_time: 0,
            weight: 10,
        };

        let cloned = item.clone();

        assert_eq!(item, cloned);
    }

    #[test]
    fn cart_item_differs_when_weight_differs() {
        let base = CartItem {
            nameid: 501,
            index: 3,
            amount: 2,
            identified: true,
            refine: 4,
            cards: vec![],
            attribute: 0,
            expire_time: 0,
            weight: 10,
        };
        let heavier = CartItem {
            weight: 20,
            ..base.clone()
        };

        assert_ne!(base, heavier);
    }
}
