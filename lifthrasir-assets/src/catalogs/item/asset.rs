use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct ItemDataAsset(pub lifthrasir_data::ItemData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_item_data() {
        let ron = r#"(items:{501:(identified_name:"Red Potion",identified_resource:"RED_POTION",identified_description:["restores HP"],unidentified_name:"",unidentified_resource:"",unidentified_description:[],slot_count:0)})"#;
        let asset = ron::from_str::<ItemDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.items[&501].identified_name, "Red Potion");
        assert_eq!(asset.0.items[&501].slot_count, 0);
    }
}
