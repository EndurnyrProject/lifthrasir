use bevy::prelude::*;
use lifthrasir_data::AccessoryData;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub struct AccessoryDb {
    names: BTreeMap<u16, String>,
}

impl AccessoryDb {
    pub fn from_accessory_data(data: AccessoryData) -> Self {
        Self { names: data.names }
    }

    pub fn accname(&self, view_id: u16) -> Option<&str> {
        self.names.get(&view_id).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> AccessoryDb {
        let mut data = AccessoryData::default();
        data.names.insert(1, "_고글".to_string());
        data.names.insert(11, "_비레타".to_string());
        AccessoryDb::from_accessory_data(data)
    }

    #[test]
    fn accname_returns_known_sprite_name() {
        let db = fixture();
        assert_eq!(db.accname(1), Some("_고글"));
        assert_eq!(db.accname(11), Some("_비레타"));
    }

    #[test]
    fn absent_view_id_returns_none() {
        let db = fixture();
        assert_eq!(db.accname(9999), None);
    }
}
