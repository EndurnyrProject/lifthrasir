#[derive(Debug, Clone, Default)]
pub struct Item {
    /// Client index (already +2 from server) — the map key.
    pub index: u16,
    /// Server nameid.
    pub item_id: u32,
    /// Client IT_* enum, raw.
    pub item_type: u8,
    /// Equippables → 1.
    pub amount: u16,
    /// Allowed equip slots (stackables → 0).
    pub location: u32,
    /// Worn bitmask (0 = in bag).
    pub wear_state: u32,
    /// Stackables → 0.
    pub refine: u8,
    pub cards: [u32; 4],
    pub options: Vec<ItemOption>,
    pub expire_time: u32,
    /// Equip 'sprite' view id; stackables → 0.
    pub view_sprite: u16,
    pub identified: bool,
    pub damaged: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ItemOption {
    pub index: u16,
    pub value: u16,
    pub param: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemCategory {
    #[default]
    Use,
    Etc,
    Equip,
}

pub fn item_category(item_type: u32) -> ItemCategory {
    match item_type {
        0 | 2 | 11 | 18 => ItemCategory::Use,
        4 | 5 | 8 | 12 => ItemCategory::Equip,
        _ => ItemCategory::Etc,
    }
}

impl Item {
    pub fn is_equipped(&self) -> bool {
        self.wear_state != 0
    }

    pub fn category(&self) -> ItemCategory {
        item_category(self.item_type.into())
    }

    pub fn type_label(&self) -> &'static str {
        match self.category() {
            ItemCategory::Use => "Usable",
            ItemCategory::Equip => "Equipment",
            ItemCategory::Etc => "Etc",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_with_type(item_type: u8) -> Item {
        Item {
            item_type,
            ..Default::default()
        }
    }

    #[test]
    fn category_use_types() {
        for t in [0u8, 2, 11, 18] {
            assert!(
                matches!(item_with_type(t).category(), ItemCategory::Use),
                "item_type {t}"
            );
        }
    }

    #[test]
    fn category_equip_types() {
        for t in [4u8, 5, 8, 12] {
            assert!(
                matches!(item_with_type(t).category(), ItemCategory::Equip),
                "item_type {t}"
            );
        }
    }

    #[test]
    fn category_etc_types() {
        for t in [3u8, 6, 7, 10, 99] {
            assert!(
                matches!(item_with_type(t).category(), ItemCategory::Etc),
                "item_type {t}"
            );
        }
    }

    #[test]
    fn type_label_non_empty() {
        for t in [0u8, 4, 3, 99] {
            assert!(!item_with_type(t).type_label().is_empty(), "item_type {t}");
        }
    }

    #[test]
    fn shared_item_category_accepts_storage_item_types() {
        assert_eq!(item_category(18), ItemCategory::Use);
        assert_eq!(item_category(12), ItemCategory::Equip);
        assert_eq!(item_category(300), ItemCategory::Etc);
    }
}
