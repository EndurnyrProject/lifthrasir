use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct SkillDataAsset(pub lifthrasir_data::SkillData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_skill_data() {
        let ron = r#"( skills: { 5: ( name: "SM_BASH", display_name: "Bash", description: [], max_level: 10, sp_cost: [8], attack_range: [1] ) } )"#;
        let asset = ron::from_str::<SkillDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.skills[&5].name, "SM_BASH");
        assert_eq!(asset.0.skills[&5].display_name, "Bash");
        assert_eq!(asset.0.skills[&5].max_level, 10);
    }
}
