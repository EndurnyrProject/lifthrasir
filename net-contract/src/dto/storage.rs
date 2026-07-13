//! Protocol-neutral Storage item type.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageItem {
    pub index: u32,
    pub nameid: u32,
    pub amount: u32,
    pub type_: u32,
    pub location: u32,
    pub attribute: u32,
    pub refine: u32,
    pub expire_time: u64,
    pub look: u32,
    pub weight: u32,
    pub identified: bool,
    pub cards: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_item_round_trips_through_clone_and_equality() {
        let item = StorageItem {
            index: 7,
            nameid: 501,
            amount: 20,
            type_: 0,
            location: 0,
            attribute: 1,
            refine: 4,
            expire_time: u32::MAX as u64 + 1,
            look: 2,
            weight: 10,
            identified: true,
            cards: vec![4001, 4002],
        };

        assert_eq!(item, item.clone());
    }
}
