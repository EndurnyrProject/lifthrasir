use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

/// Accessory (headgear) sprite-name table decoded from `accessoryid.lub` + `accname.lub`.
/// Maps a view id to its sprite name (EUC-KR decoded, leading separator preserved verbatim).
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessoryData {
    pub names: BTreeMap<u16, String>,
}

/// Weapon sprite/SFX metadata decoded from `weapontable.lub`.
/// Keyed by `BTreeMap`/`BTreeSet` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponData {
    /// weapon view id -> sprite suffix (leading `_` included)
    pub names: BTreeMap<u16, String>,
    /// weapon view id -> hit wav filename
    pub hit_sounds: BTreeMap<u16, String>,
    /// weapon view ids that are bow-type
    pub bow_types: BTreeSet<u16>,
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

/// Where a ground skill's persistent visual anchors: once at the skill-unit
/// group center, or once per skill-unit cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundAnchor {
    #[default]
    Group,
    Cell,
}

/// Hand-authored skill -> effect mapping entry.
/// `color` is `[f32; 4]` (RGBA) to keep this crate Bevy-free; the runtime
/// converts it to `Color` at its boundary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectDescriptor {
    /// STR effect filename, e.g. "heal.str". `None` for skills with no visual
    /// STR effect (e.g. sound-only skills like Bash); such entries still play
    /// their `sound` but spawn no STR.
    pub str: Option<String>,
    /// Procedural (non-STR) VFX key, e.g. "bash". Resolved by the presentation
    /// VFX layer to a spawn function. Mutually exclusive with `str`. `None` for
    /// skills with no procedural effect.
    #[serde(default)]
    pub vfx: Option<String>,
    /// Optional sound path relative to `data/wav`, e.g. "effect/ef_firewall.wav"
    /// or "_heal_effect.wav" (files at the wav root take no `effect/` prefix).
    pub sound: Option<String>,
    pub placement: EffectPlacement,
    /// RGBA tint multiplied onto the STR's per-frame color.
    pub color: [f32; 4],
    /// One-shot vs persistent (ground) effect.
    pub repeating: bool,
    /// Where a ground skill's persistent visual anchors. Ignored for
    /// non-ground skills.
    #[serde(default)]
    pub ground_anchor: GroundAnchor,
}

/// Unified effect catalog: skill-effect and map-effect descriptors, keyed by
/// their own id namespaces (`skills` by rAthena skill id, `map` by RSW
/// `effect_type` / rAthena `e_special_effects` `EF_*` id). The two sections
/// stay distinct because the id spaces overlap but mean different things.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectData {
    pub skills: BTreeMap<u32, EffectDescriptor>,
    pub map: BTreeMap<u32, EffectDescriptor>,
    /// Persistent status-aura descriptors, keyed by EFST id.
    #[serde(default)]
    pub statuses: BTreeMap<u32, EffectDescriptor>,
}

/// Per-status icon presentation: TGA image name and English display name,
/// decoded from the client's EFST icon tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusIconEntry {
    pub image: String,
    pub name: String,
}

/// Full status icon catalog: EFST id -> icon presentation.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusIconData {
    pub icons: BTreeMap<u32, StatusIconEntry>,
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
    fn accessory_data_round_trip() {
        let mut original = AccessoryData::default();
        original.names.insert(1, "_고글".to_string());
        original.names.insert(2, "_고양이머리띠".to_string());

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: AccessoryData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn weapon_data_round_trip() {
        let mut original = WeaponData::default();
        original.names.insert(2, "_검".to_string());
        original.names.insert(3, "_양손검".to_string());
        original.hit_sounds.insert(2, "_hit_sword.wav".to_string());
        original.bow_types.insert(11);

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: WeaponData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
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
    fn effect_data_round_trip() {
        let mut original = EffectData::default();
        original.skills.insert(
            28,
            EffectDescriptor {
                str: Some("heal.str".to_string()),
                vfx: None,
                sound: Some("_heal_effect.wav".to_string()),
                placement: EffectPlacement::Target,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: false,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.map.insert(
            89,
            EffectDescriptor {
                str: Some("stormgust.str".to_string()),
                vfx: None,
                sound: None,
                placement: EffectPlacement::Ground,
                color: [0.6, 0.7, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.statuses.insert(
            157,
            EffectDescriptor {
                str: Some("energycoat.str".to_string()),
                vfx: None,
                sound: None,
                placement: EffectPlacement::Caster,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.skills, deserialized.skills);
        assert_eq!(original.map, deserialized.map);
        assert_eq!(original.statuses, deserialized.statuses);

        let energy_coat = deserialized.statuses.get(&157).expect("EFST 157 entry");
        assert_eq!(energy_coat.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn ground_anchor_defaults_to_group_when_absent() {
        let ron = r#"(
            str: Some("firewall.str"),
            vfx: None,
            sound: None,
            placement: Ground,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: true,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert_eq!(descriptor.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn effect_data_without_statuses_section_deserializes() {
        let ron = r#"(
            skills: {},
            map: {},
        )"#;

        let data: EffectData = ron::from_str(ron).expect("deserialize");

        assert!(data.statuses.is_empty());
    }

    #[test]
    fn status_icon_data_round_trip() {
        let mut original = StatusIconData::default();
        original.icons.insert(
            10,
            StatusIconEntry {
                image: "BLESSING.TGA".to_string(),
                name: "Blessing".to_string(),
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: StatusIconData = ron::from_str(&serialized).expect("deserialize");

        let entry = deserialized.icons.get(&10).expect("efst 10 entry");
        assert_eq!(entry.image, "BLESSING.TGA");
        assert_eq!(entry.name, "Blessing");
    }

    #[test]
    fn effects_ron_seed_deserializes() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/data/ron/effects.ron"
        );
        let contents = std::fs::read_to_string(path).expect("read effects.ron");
        let data: EffectData = ron::from_str(&contents).expect("deserialize seed");

        let heal = data.skills.get(&28).expect("AL_HEAL entry");
        assert_eq!(heal.placement, EffectPlacement::Target);
        assert_eq!(heal.str.as_deref(), Some("heal.str"));
        // Sound is relative to `data/wav/`; `_heal_effect.wav` lives at the root
        // (no `effect/` prefix), so the old `effect/_heal_effect.wav` was broken.
        assert_eq!(heal.sound.as_deref(), Some("_heal_effect.wav"));
        assert!(!heal.repeating);

        let stormgust = data.skills.get(&89).expect("WZ_STORMGUST entry");
        assert_eq!(stormgust.placement, EffectPlacement::Ground);
        assert_eq!(stormgust.str.as_deref(), Some("stormgust.str"));
        // `effect/stormgust.wav` does not exist in the GRF; the real sound is
        // `effect/wizard_stormgust.wav`.
        assert_eq!(
            stormgust.sound.as_deref(),
            Some("effect/wizard_stormgust.wav")
        );
        assert!(stormgust.repeating);

        // SM_BASH is sound-only: no STR effect, but it still plays its sound.
        let bash = data.skills.get(&5).expect("SM_BASH entry");
        assert_eq!(bash.str, None);
        assert_eq!(bash.sound.as_deref(), Some("effect/ef_bash.wav"));

        let map_stormgust = data.map.get(&89).expect("EF_STORMGUST entry");
        assert_eq!(map_stormgust.str.as_deref(), Some("stormgust.str"));
        assert!(map_stormgust.repeating);

        let magnus = data.map.get(&113).expect("EF_MAGNUS entry");
        assert_eq!(magnus.str.as_deref(), Some("magnus.str"));

        assert_eq!(heal.ground_anchor, GroundAnchor::Group);
        assert_eq!(stormgust.ground_anchor, GroundAnchor::Group);

        // 31 EFST_ENERGYCOAT: the persistent Energy Coat aura, migrated out of
        // `skills:` (157 there stays as the one-shot cast flash).
        let energy_coat_aura = data.statuses.get(&31).expect("EFST_ENERGYCOAT entry");
        assert_eq!(energy_coat_aura.str.as_deref(), Some("energycoat.str"));
        assert!(energy_coat_aura.repeating);

        let energy_coat_cast = data.skills.get(&157).expect("MG_ENERGYCOAT entry");
        assert_eq!(energy_coat_cast.str.as_deref(), Some("energycoat.str"));
        assert!(!energy_coat_cast.repeating);
    }
}
