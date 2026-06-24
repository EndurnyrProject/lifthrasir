use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobData {
    /// Lua JobNameTable: job id -> sprite name (NPC + monster job sprites).
    pub npc_sprites: BTreeMap<u32, String>,
    /// Lua PCJobNameTable: job id -> display name.
    pub display_names: BTreeMap<u32, String>,
}

/// Per-item presentation metadata decoded from `iteminfo.lub`.
/// All strings are valid UTF-8 (EUC-KR decoded by the CLI converter).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemInfo {
    pub identified_name: String,
    pub identified_resource: String,
    pub identified_description: Vec<String>,
    pub unidentified_name: String,
    pub unidentified_resource: String,
    pub unidentified_description: Vec<String>,
    pub slot_count: u8,
}

/// Full item database: item id -> presentation metadata.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemData {
    pub items: BTreeMap<u32, ItemInfo>,
}

/// Per-skill presentation metadata decoded from `skillinfolist.lub` and `skilldescript.lub`.
/// All strings are valid UTF-8 (EUC-KR decoded by the CLI converter).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMeta {
    /// SKID constant, e.g. "SM_BASH". Base for the icon filename.
    pub name: String,
    /// Display name, e.g. "Bash".
    pub display_name: String,
    /// Raw tooltip lines (color codes like ^RRGGBB kept; UI strips at render).
    pub description: Vec<String>,
    /// skillinfolist MaxLv.
    pub max_level: u8,
    /// skillinfolist SpAmount, per level (index 0 = level 1). Empty for passives / absent.
    pub sp_cost: Vec<u16>,
    /// skillinfolist AttackRange, per level (index 0 = level 1). Empty when absent.
    pub attack_range: Vec<u8>,
}

/// Full skill catalog: skill id -> presentation metadata.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillData {
    pub skills: BTreeMap<u32, SkillMeta>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_data_round_trip() {
        let mut original = JobData::default();
        original.npc_sprites.insert(0, "NOVICE".to_string());
        original.npc_sprites.insert(1, "SWORDMAN".to_string());
        original.display_names.insert(0, "Novice".to_string());
        original.display_names.insert(1, "Swordman".to_string());

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: JobData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn item_data_round_trip() {
        let mut original = ItemData::default();
        original.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                identified_description: vec![
                    "A potion that restores 45 HP.".to_string(),
                    "Brewed from red herbs.".to_string(),
                ],
                unidentified_name: "Unknown Potion".to_string(),
                unidentified_resource: "UNKNOWN_POTION".to_string(),
                unidentified_description: vec!["An unidentified potion.".to_string()],
                slot_count: 0,
            },
        );
        original.items.insert(
            2104,
            ItemInfo {
                identified_name: "Buckler".to_string(),
                identified_resource: "BUCKLER".to_string(),
                identified_description: vec![
                    "A small round shield.".to_string(),
                    "DEF +5.".to_string(),
                ],
                unidentified_name: "Round Shield".to_string(),
                unidentified_resource: "ROUND_SHIELD".to_string(),
                unidentified_description: vec!["An unidentified shield.".to_string()],
                slot_count: 1,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: ItemData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.items, deserialized.items);
    }

    #[test]
    fn skill_data_round_trip() {
        let mut original = SkillData::default();
        original.skills.insert(
            5,
            SkillMeta {
                name: "SM_BASH".to_string(),
                display_name: "Bash".to_string(),
                description: vec![
                    "Strike an enemy with your weapon.".to_string(),
                    "ATK +300% at level 10.".to_string(),
                ],
                max_level: 10,
                sp_cost: vec![8, 8, 8, 8, 8, 15, 15, 15, 15, 15],
                attack_range: vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            },
        );
        original.skills.insert(
            8,
            SkillMeta {
                name: "SM_ENDURE".to_string(),
                display_name: "Endure".to_string(),
                description: vec!["Ignore MDEF interruptions.".to_string()],
                max_level: 10,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: SkillData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.skills, deserialized.skills);
    }
}
