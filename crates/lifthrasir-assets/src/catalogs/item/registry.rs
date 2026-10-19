use bevy::prelude::*;
use lifthrasir_data::ItemData;
use std::collections::BTreeMap;

pub use lifthrasir_data::ItemInfo;

#[derive(Resource, Default)]
pub struct ItemDb {
    items: BTreeMap<u32, ItemInfo>,
}

impl ItemDb {
    pub fn from_item_data(data: ItemData) -> Self {
        Self { items: data.items }
    }

    pub fn get(&self, id: u32) -> Option<&ItemInfo> {
        self.items.get(&id)
    }

    pub fn name(&self, id: u32, identified: bool) -> Option<&str> {
        let item = self.items.get(&id)?;
        let s = if identified {
            &item.identified_name
        } else {
            &item.unidentified_name
        };
        Some(s.as_str())
    }

    pub fn icon_resource(&self, id: u32, identified: bool) -> Option<&str> {
        let item = self.items.get(&id)?;
        let s = if identified {
            &item.identified_resource
        } else {
            &item.unidentified_resource
        };
        Some(s.as_str())
    }

    pub fn description(&self, id: u32, identified: bool) -> Option<&[String]> {
        let item = self.items.get(&id)?;
        let v = if identified {
            &item.identified_description
        } else {
            &item.unidentified_description
        };
        Some(v.as_slice())
    }

    pub fn slot_count(&self, id: u32) -> Option<u8> {
        self.items.get(&id).map(|i| i.slot_count)
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

    fn fixture() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                identified_description: vec![
                    "Restores 45 HP.".to_string(),
                    "Brewed from red herbs.".to_string(),
                ],
                unidentified_name: "Unknown Potion".to_string(),
                unidentified_resource: "UNKNOWN_POTION".to_string(),
                unidentified_description: vec!["An unidentified potion.".to_string()],
                slot_count: 1,
            },
        );
        data.items.insert(
            2104,
            ItemInfo {
                identified_name: "Buckler".to_string(),
                identified_resource: "BUCKLER".to_string(),
                identified_description: vec!["A small round shield.".to_string()],
                unidentified_name: "Round Shield".to_string(),
                unidentified_resource: "ROUND_SHIELD".to_string(),
                unidentified_description: vec!["An unidentified shield.".to_string()],
                slot_count: 0,
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn name_returns_identified_or_unidentified() {
        let db = fixture();
        assert_eq!(db.name(501, true), Some("Red Potion"));
        assert_eq!(db.name(501, false), Some("Unknown Potion"));
        assert_eq!(db.name(2104, true), Some("Buckler"));
        assert_eq!(db.name(2104, false), Some("Round Shield"));
    }

    #[test]
    fn icon_resource_returns_correct_variant() {
        let db = fixture();
        assert_eq!(db.icon_resource(501, true), Some("RED_POTION"));
        assert_eq!(db.icon_resource(501, false), Some("UNKNOWN_POTION"));
        assert_eq!(db.icon_resource(2104, true), Some("BUCKLER"));
        assert_eq!(db.icon_resource(2104, false), Some("ROUND_SHIELD"));
    }

    #[test]
    fn description_returns_slice() {
        let db = fixture();
        assert_eq!(
            db.description(501, true),
            Some(
                [
                    "Restores 45 HP.".to_string(),
                    "Brewed from red herbs.".to_string()
                ]
                .as_slice()
            )
        );
        assert_eq!(
            db.description(501, false),
            Some(["An unidentified potion.".to_string()].as_slice())
        );
    }

    #[test]
    fn slot_count_correct() {
        let db = fixture();
        assert_eq!(db.slot_count(501), Some(1));
        assert_eq!(db.slot_count(2104), Some(0));
    }

    #[test]
    fn absent_id_returns_none() {
        let db = fixture();
        assert_eq!(db.get(9999), None);
        assert_eq!(db.name(9999, true), None);
        assert_eq!(db.slot_count(9999), None);
    }

    #[test]
    fn len_and_is_empty() {
        let db = fixture();
        assert_eq!(db.len(), 2);
        assert!(!db.is_empty());

        let empty = ItemDb::default();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
