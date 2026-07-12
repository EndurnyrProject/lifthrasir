use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct StatusIconDataAsset(pub lifthrasir_data::StatusIconData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_status_icon_data() {
        let ron = r#"(icons:{10:(image:"",name:"Blessing"),140:(image:"ravensflight.tga",name:"Raven's Flight")})"#;
        let asset = ron::from_str::<StatusIconDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.icons[&10].name, "Blessing");
        assert_eq!(asset.0.icons[&10].image, "");
        assert_eq!(asset.0.icons[&140].image, "ravensflight.tga");
        assert_eq!(asset.0.icons[&140].name, "Raven's Flight");
    }
}
