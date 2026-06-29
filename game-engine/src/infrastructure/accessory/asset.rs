use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct AccessoryDataAsset(pub lifthrasir_data::AccessoryData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_accessory_data() {
        let ron = r#"(names:{1:"_고글"})"#;
        let asset = ron::from_str::<AccessoryDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.names[&1], "_고글");
    }
}
