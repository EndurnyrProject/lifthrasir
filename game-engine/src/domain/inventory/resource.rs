use super::item::Item;
use bevy::prelude::*;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub struct Inventory {
    items: BTreeMap<u16, Item>,
    ready: bool,
}

impl Inventory {
    pub fn begin(&mut self) {
        self.items.clear();
        self.ready = false;
    }

    pub fn upsert(&mut self, item: Item) {
        self.items.insert(item.index, item);
    }

    pub fn finish(&mut self) {
        self.ready = true;
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn equipped(&self) -> impl Iterator<Item = &Item> {
        self.items.values().filter(|item| item.is_equipped())
    }

    pub fn stackables(&self) -> impl Iterator<Item = &Item> {
        self.items.values().filter(|item| !item.is_equipped())
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, index: u16) -> Option<&Item> {
        self.items.get(&index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equip(index: u16) -> Item {
        Item {
            index,
            wear_state: 1,
            amount: 1,
            ..Default::default()
        }
    }

    fn stackable(index: u16, amount: u16) -> Item {
        Item {
            index,
            wear_state: 0,
            amount,
            ..Default::default()
        }
    }

    #[test]
    fn begin_clears_items_and_marks_not_ready() {
        let mut inv = Inventory::default();
        inv.upsert(stackable(2, 5));
        inv.finish();
        assert!(inv.is_ready());

        inv.begin();

        assert_eq!(inv.len(), 0);
        assert!(!inv.is_ready());
    }

    #[test]
    fn upsert_inserts_then_overwrites_by_index() {
        let mut inv = Inventory::default();
        inv.upsert(stackable(2, 5));
        assert_eq!(inv.get(2).unwrap().amount, 5);

        inv.upsert(stackable(2, 9));

        assert_eq!(inv.len(), 1);
        assert_eq!(inv.get(2).unwrap().amount, 9);
    }

    #[test]
    fn finish_marks_ready() {
        let mut inv = Inventory::default();
        assert!(!inv.is_ready());

        inv.finish();

        assert!(inv.is_ready());
    }

    #[test]
    fn equipped_and_stackables_partition_by_wear_state() {
        let mut inv = Inventory::default();
        inv.upsert(equip(2));
        inv.upsert(stackable(3, 1));
        inv.upsert(equip(4));

        let equipped: Vec<u16> = inv.equipped().map(|i| i.index).collect();
        let stackables: Vec<u16> = inv.stackables().map(|i| i.index).collect();

        assert_eq!(equipped, vec![2, 4]);
        assert_eq!(stackables, vec![3]);
    }
}
