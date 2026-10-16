use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource)]
pub struct SkillCatalog {
    skills: HashMap<u32, lifthrasir_data::SkillMeta>,
}

impl SkillCatalog {
    pub fn from_skill_data(data: lifthrasir_data::SkillData) -> Self {
        Self {
            skills: data.skills.into_iter().collect(),
        }
    }

    pub fn get(&self, id: u32) -> Option<&lifthrasir_data::SkillMeta> {
        self.skills.get(&id)
    }

    pub fn icon_path(&self, id: u32) -> Option<String> {
        let skill = self.skills.get(&id)?;
        Some(format!(
            "ro://data/texture/유저인터페이스/item/{}.bmp",
            skill.name.to_lowercase()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{SkillData, SkillMeta};

    fn fixture() -> SkillData {
        let mut data = SkillData::default();
        data.skills.insert(
            5,
            SkillMeta {
                name: "SM_BASH".to_string(),
                display_name: "Bash".to_string(),
                description: vec![],
                max_level: 10,
                sp_cost: vec![8],
                attack_range: vec![1],
            },
        );
        data
    }

    #[test]
    fn from_skill_data_builds_lookup() {
        let catalog = SkillCatalog::from_skill_data(fixture());

        assert_eq!(catalog.get(5).unwrap().name, "SM_BASH");
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let catalog = SkillCatalog::from_skill_data(fixture());

        assert!(catalog.get(9999).is_none());
    }

    #[test]
    fn icon_path_formats_sm_bash_correctly() {
        let catalog = SkillCatalog::from_skill_data(fixture());

        assert_eq!(
            catalog.icon_path(5),
            Some("ro://data/texture/유저인터페이스/item/sm_bash.bmp".to_string())
        );
    }

    #[test]
    fn icon_path_returns_none_for_unknown_id() {
        let catalog = SkillCatalog::from_skill_data(fixture());

        assert!(catalog.icon_path(9999).is_none());
    }
}
