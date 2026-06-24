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

/// Where a skill effect anchors when played.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectPlacement {
    Caster,
    #[default]
    Target,
    Ground,
}

/// Hand-authored skill -> effect mapping entry.
/// `color` is `[f32; 4]` (RGBA) to keep this crate Bevy-free; the runtime
/// converts it to `Color` at its boundary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectDescriptor {
    /// STR effect filename, e.g. "heal.str".
    pub str: String,
    /// Optional sound path relative to `data/wav`, e.g. "effect/_heal_effect.wav".
    pub sound: Option<String>,
    pub placement: EffectPlacement,
    /// RGBA tint multiplied onto the STR's per-frame color.
    pub color: [f32; 4],
    /// One-shot vs persistent (ground) effect.
    pub repeating: bool,
}

/// Skill effect catalog: skill id -> effect descriptor.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillEffectData {
    pub effects: BTreeMap<u32, EffectDescriptor>,
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

    #[test]
    fn skill_effect_data_round_trip() {
        let mut original = SkillEffectData::default();
        original.effects.insert(
            28,
            EffectDescriptor {
                str: "heal.str".to_string(),
                sound: Some("effect/_heal_effect.wav".to_string()),
                placement: EffectPlacement::Target,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: false,
            },
        );
        original.effects.insert(
            89,
            EffectDescriptor {
                str: "stormgust.str".to_string(),
                sound: None,
                placement: EffectPlacement::Ground,
                color: [0.6, 0.7, 1.0, 1.0],
                repeating: true,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: SkillEffectData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.effects, deserialized.effects);
    }

    #[test]
    fn skill_effects_ron_seed_deserializes() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/data/ron/skill_effects.ron"
        );
        let contents = std::fs::read_to_string(path).expect("read skill_effects.ron");
        let data: SkillEffectData = ron::from_str(&contents).expect("deserialize seed");

        let heal = data.effects.get(&28).expect("AL_HEAL entry");
        assert_eq!(heal.placement, EffectPlacement::Target);
        assert_eq!(heal.str, "heal.str");
        assert_eq!(heal.sound.as_deref(), Some("effect/_heal_effect.wav"));
        assert!(!heal.repeating);

        let stormgust = data.effects.get(&89).expect("WZ_STORMGUST entry");
        assert_eq!(stormgust.placement, EffectPlacement::Ground);
        assert_eq!(stormgust.str, "stormgust.str");
        assert!(stormgust.repeating);
    }
}
