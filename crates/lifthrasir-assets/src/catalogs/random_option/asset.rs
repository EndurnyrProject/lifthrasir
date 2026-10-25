use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct RandomOptionDataAsset(pub lifthrasir_data::RandomOptionData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_random_option_data() {
        let ron = r#"(names:{3:"STR + %d",9:"MHP + %d%%"})"#;
        let asset = ron::from_str::<RandomOptionDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.names[&3], "STR + %d");
        assert_eq!(asset.0.names[&9], "MHP + %d%%");
    }
}
