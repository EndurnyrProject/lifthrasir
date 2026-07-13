use bevy::prelude::*;
use net_contract::dto::StorageItem;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub struct Storage {
    items: BTreeMap<u32, StorageItem>,
    capacity: u32,
    open: bool,
}

impl Storage {
    pub fn open(&mut self, capacity: u32, items: Vec<StorageItem>) {
        self.items = items.into_iter().map(|item| (item.index, item)).collect();
        self.capacity = capacity;
        self.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn upsert(&mut self, item: StorageItem) {
        self.items.insert(item.index, item);
    }

    pub fn remove_amount(&mut self, index: u32, amount: u32) {
        let Some(item) = self.items.get_mut(&index) else {
            return;
        };
        item.amount = item.amount.saturating_sub(amount);
        if item.amount == 0 {
            self.items.remove(&index);
        }
    }

    pub fn get(&self, index: u32) -> Option<&StorageItem> {
        self.items.get(&index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &StorageItem> {
        self.items.values()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::StorageItem;

    fn item(index: u32, amount: u32) -> StorageItem {
        StorageItem {
            index,
            nameid: 501,
            amount,
            type_: 0,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight: 10,
            identified: true,
            cards: vec![],
        }
    }

    #[test]
    fn opening_replaces_snapshot_capacity_and_marks_storage_open() {
        let mut storage = Storage::default();
        storage.open(100, vec![item(9, 1)]);

        storage.open(40, vec![item(7, 2), item(3, 5)]);

        assert!(storage.is_open());
        assert_eq!(storage.capacity(), 40);
        assert_eq!(storage.len(), 2);
        assert_eq!(
            storage.iter().map(|item| item.index).collect::<Vec<_>>(),
            vec![3, 7]
        );
        assert!(storage.get(9).is_none());
    }

    #[test]
    fn upsert_inserts_and_replaces_server_reported_total() {
        let mut storage = Storage::default();
        storage.open(40, vec![]);

        storage.upsert(item(7, 2));
        storage.upsert(item(7, 9));

        assert_eq!(storage.len(), 1);
        assert_eq!(storage.get(7).unwrap().amount, 9);
    }

    #[test]
    fn remove_amount_decrements_a_stack() {
        let mut storage = Storage::default();
        storage.open(40, vec![item(7, 9)]);

        storage.remove_amount(7, 4);

        assert_eq!(storage.get(7).unwrap().amount, 5);
    }

    #[test]
    fn remove_amount_drops_a_stack_at_zero() {
        let mut storage = Storage::default();
        storage.open(40, vec![item(7, 4)]);

        storage.remove_amount(7, 4);

        assert!(storage.get(7).is_none());
        assert!(storage.is_empty());
    }

    #[test]
    fn close_marks_storage_closed_without_discarding_snapshot() {
        let mut storage = Storage::default();
        storage.open(40, vec![item(7, 4)]);

        storage.close();

        assert!(!storage.is_open());
        assert_eq!(storage.capacity(), 40);
        assert_eq!(storage.get(7).unwrap().amount, 4);
    }

    #[test]
    fn reset_clears_snapshot_capacity_and_open_state() {
        let mut storage = Storage::default();
        storage.open(40, vec![item(7, 4)]);

        storage.reset();

        assert!(!storage.is_open());
        assert_eq!(storage.capacity(), 0);
        assert!(storage.is_empty());
    }
}
