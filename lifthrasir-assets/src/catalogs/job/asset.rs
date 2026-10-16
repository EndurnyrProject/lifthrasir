use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct JobDataAsset(pub lifthrasir_data::JobData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_job_data() {
        let ron = r#"( npc_sprites: { 45: "1_ETC_01" }, display_names: { 0: "Novice" } )"#;
        let asset = ron::from_str::<JobDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.npc_sprites[&45], "1_ETC_01");
        assert_eq!(asset.0.display_names[&0], "Novice");
    }
}
