use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod lif;

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

/// One visual layer of an effect. A descriptor's `visuals` list is the ordered
/// set of layers that make up its look; the dispatch code picks the layer kind
/// it knows how to render (see [`EffectDescriptor`]'s accessors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Visual {
    /// STR effect asset: native GRF `.str` or authored `.strfx.ron` (dispatch by
    /// extension happens in the runtime's `effect_asset_path`).
    Str(String),
    /// SPR/ACT sprite animation stem under `data/sprite/`, e.g.
    /// `"이팩트/firewall"`, for the effects the classic client draws as looping
    /// sprites instead of STR sequences (Fire Wall, Fire Pillar).
    Sprite(String),
    /// Converted glTF prop under `data/models/`, extension-free, e.g.
    /// `"외부소품/트랩02"`, for persistent ground-unit visuals that are models
    /// rather than effects.
    Model(String),
    /// Key into the `shader_fx` table (procedural uber-shader burst). Validated
    /// against that table when the catalog is built.
    Shader(String),
    /// Key dispatched to a bespoke code spawner (`"ice_wall"`, `"smoke"`,
    /// `"emitter"`). Never resolved against `shader_fx`.
    Bespoke(String),
    /// Splice the referenced `efsts` recipe's visual layers in here. Composition
    /// only, one level deep: an `efsts` recipe may not itself carry `Efst`
    /// layers.
    Efst(u32),
}

/// One-shot played when a skill unit leaves its ACTIVE phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerFx {
    pub visual: Visual,
    #[serde(default)]
    pub sound: Option<String>,
}

/// Hand-authored effect entry: an ordered list of visual layers plus the
/// presentation fields shared by all of them.
/// `color` is `[f32; 4]` (RGBA) to keep this crate Bevy-free; the runtime
/// converts it to `Color` at its boundary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectDescriptor {
    /// Visual layers, in authoring order. Empty for sound-only entries: those
    /// still play their `sound` but spawn nothing.
    #[serde(default)]
    pub visuals: Vec<Visual>,
    /// Optional sound path relative to `data/wav`, e.g. "effect/ef_firewall.wav"
    /// or "_heal_effect.wav" (files at the wav root take no `effect/` prefix).
    pub sound: Option<String>,
    #[serde(default)]
    pub on_trigger: Option<TriggerFx>,
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

impl EffectDescriptor {
    /// First `Str` layer's asset name, if any.
    pub fn str_name(&self) -> Option<&str> {
        self.visuals.iter().find_map(|visual| match visual {
            Visual::Str(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// First `Sprite` layer's SPR/ACT stem, if any.
    pub fn sprite_stem(&self) -> Option<&str> {
        self.visuals.iter().find_map(|visual| match visual {
            Visual::Sprite(stem) => Some(stem.as_str()),
            _ => None,
        })
    }

    /// First `Model` layer's path stem, if any.
    pub fn model_path(&self) -> Option<&str> {
        self.visuals.iter().find_map(|visual| match visual {
            Visual::Model(stem) => Some(stem.as_str()),
            _ => None,
        })
    }

    /// First `Shader` layer's `shader_fx` key, if any.
    pub fn shader_key(&self) -> Option<&str> {
        self.visuals.iter().find_map(|visual| match visual {
            Visual::Shader(key) => Some(key.as_str()),
            _ => None,
        })
    }

    /// First `Bespoke` layer's spawner key, if any.
    pub fn bespoke_key(&self) -> Option<&str> {
        self.visuals.iter().find_map(|visual| match visual {
            Visual::Bespoke(key) => Some(key.as_str()),
            _ => None,
        })
    }
}

/// Point-light pop accompanying a shader-fx entry. Drives the `PointLight` +
/// `LightFade` pair `spawn_shader_fx` builds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderFxLight {
    pub color: (f32, f32, f32),
    pub intensity_scale: f32,
    pub fade: f32,
}

/// Tint for the tintable spark garnish child.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderFxGarnish {
    pub tint: (f32, f32, f32, f32),
}

/// An animated classic GRF texture: the ordered frame paths (each relative to
/// the `ro://` root) plus the playback rate. Used wherever a `SkillFxMaterial`
/// binds a texture — burst or projectile — cycling the bound frame at `fps`,
/// looping. Prefer this over a single `texture` when the source art is a series
/// (most RO effect textures are, e.g. `thunder_ball_0001..0006`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextureFrames {
    pub paths: Vec<String>,
    pub fps: f32,
}

/// Caster→target travel config. When present on an entry (and the caster is
/// resolvable), the effect first flies a projectile billboard from the caster to
/// the target, then plays the burst on arrival. `texture` is the projectile's own
/// classic GRF orb sprite (e.g. `data/texture/effect/fireorb.bmp`); when `None`
/// the projectile falls back to the entry's burst `texture`. Each skill supplies
/// its own orb, so every projectile looks like its skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderFxTravel {
    /// World units per second the projectile advances toward the target.
    pub speed: f32,
    /// Uniform world-space scale of the in-flight projectile billboard.
    pub scale: f32,
    #[serde(default)]
    pub texture: Option<String>,
    /// Animated projectile art; wins over `texture` when set.
    #[serde(default)]
    pub frames: Option<TextureFrames>,
    /// Launch one projectile per hit (the classic bolt behavior: a level-N bolt
    /// throws N orbs in sequence). `false` (default) launches a single orb
    /// regardless of hit count (e.g. Jupitel Thunder's one ball).
    #[serde(default)]
    pub per_hit: bool,
    /// Seconds between successive per-hit launches. `0` (default) fires them all
    /// at once; ignored when `per_hit` is false.
    #[serde(default)]
    pub stagger: f32,
}

/// One shader-fx catalog entry: the `SkillFxParams` payload plus the optional
/// light/garnish children `spawn_shader_fx` builds. `shape`'s meaning is
/// per-`kind`, documented in each kind's wgsl fragment function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderFxEntry {
    pub kind: u32,
    pub primary: (f32, f32, f32, f32),
    pub secondary: (f32, f32, f32, f32),
    pub shape: (f32, f32, f32, f32),
    pub duration: f32,
    /// Uniform world-space scale of the billboard quad.
    pub scale: f32,
    #[serde(default)]
    pub light: Option<ShaderFxLight>,
    #[serde(default)]
    pub garnish: Option<ShaderFxGarnish>,
    /// Optional classic GRF effect texture, a path relative to the `ro://` asset
    /// source root (e.g. `data/texture/effect/fire_fall_b.bmp`). `spawn_shader_fx`
    /// loads it as `ro://{path}`; `None` binds the fallback image.
    #[serde(default)]
    pub texture: Option<String>,
    /// Animated burst art; wins over `texture` when set.
    #[serde(default)]
    pub frames: Option<TextureFrames>,
    /// Optional caster→target travel. `None` plays the burst straight at the
    /// target, exactly as before.
    #[serde(default)]
    pub travel: Option<ShaderFxTravel>,
}

/// Unified effect catalog. The three descriptor sections are distinct id
/// namespaces that overlap numerically but mean different things (skill 86 is
/// WZ_WATERBALL, EFST 86 is Fury), so they never merge into one map.
///
/// `efsts` entries double as reusable recipes: a `skills`/`special` entry can
/// compose one with a [`Visual::Efst`] layer, which is spliced in at catalog
/// build time. Composition is reuse only — it does not trigger anything. The
/// server still drives status visuals directly by EFST id through
/// `StatusEffectChanged`.
///
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectData {
    /// Skill-cast descriptors, keyed by rAthena skill id.
    pub skills: BTreeMap<u32, EffectDescriptor>,
    /// The rAthena `e_special_effects` (`EF_*`) namespace, serving BOTH the RSW
    /// map `EFFECT` objects and aesir's `SpecialEffect` packets — they key on
    /// the same ids.
    pub special: BTreeMap<u32, EffectDescriptor>,
    /// Status-aura descriptors, keyed by EFST id. Both the lookup table for
    /// `StatusEffectChanged` and the recipe table [`Visual::Efst`] composes.
    #[serde(default)]
    pub efsts: BTreeMap<u32, EffectDescriptor>,
    /// Procedural shader-fx entries, keyed by [`Visual::Shader`] key.
    #[serde(default)]
    pub shader_fx: BTreeMap<String, ShaderFxEntry>,
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
                visuals: vec![Visual::Str("heal.str".to_string())],
                sound: Some("_heal_effect.wav".to_string()),
                on_trigger: None,
                placement: EffectPlacement::Target,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: false,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.special.insert(
            89,
            EffectDescriptor {
                visuals: vec![Visual::Str("stormgust.str".to_string())],
                sound: None,
                on_trigger: None,
                placement: EffectPlacement::Ground,
                color: [0.6, 0.7, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.efsts.insert(
            157,
            EffectDescriptor {
                visuals: vec![Visual::Str("energycoat.str".to_string())],
                sound: None,
                on_trigger: None,
                placement: EffectPlacement::Caster,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.skills, deserialized.skills);
        assert_eq!(original.special, deserialized.special);
        assert_eq!(original.efsts, deserialized.efsts);

        let energy_coat = deserialized.efsts.get(&157).expect("EFST 157 entry");
        assert_eq!(energy_coat.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn every_visual_kind_round_trips() {
        let original = EffectDescriptor {
            visuals: vec![
                Visual::Sprite("이팩트/firewall".to_string()),
                Visual::Str("firewall.str".to_string()),
                Visual::Shader("fire_bolt".to_string()),
                Visual::Bespoke("ice_wall".to_string()),
                Visual::Efst(31),
            ],
            ..Default::default()
        };

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectDescriptor = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn model_visual_and_trigger_fx_round_trip() {
        let original = EffectDescriptor {
            visuals: vec![Visual::Model("외부소품/트랩02".to_string())],
            on_trigger: Some(TriggerFx {
                visual: Visual::Str("skidtrap.str".to_string()),
                sound: Some("effect/hunter_skidtrap.wav".to_string()),
            }),
            ..Default::default()
        };

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectDescriptor = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn descriptor_without_on_trigger_deserializes() {
        let ron = r#"(
            visuals: [Str("firewall.str")],
            sound: None,
            placement: Ground,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: true,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert_eq!(descriptor.on_trigger, None);
    }

    #[test]
    fn model_path_only_reads_model_layers() {
        let model_only = EffectDescriptor {
            visuals: vec![Visual::Model("외부소품/트랩02".to_string())],
            ..Default::default()
        };
        let sprite_only = EffectDescriptor {
            visuals: vec![Visual::Sprite("이팩트/firewall".to_string())],
            ..Default::default()
        };

        assert_eq!(model_only.model_path(), Some("외부소품/트랩02"));
        assert_eq!(model_only.sprite_stem(), None);
        assert_eq!(sprite_only.model_path(), None);
    }

    #[test]
    fn accessors_return_the_first_layer_of_their_kind() {
        let descriptor = EffectDescriptor {
            visuals: vec![
                Visual::Sprite("이팩트/firewall".to_string()),
                Visual::Bespoke("ice_wall".to_string()),
            ],
            ..Default::default()
        };

        assert_eq!(descriptor.sprite_stem(), Some("이팩트/firewall"));
        assert_eq!(descriptor.bespoke_key(), Some("ice_wall"));
        assert_eq!(descriptor.str_name(), None);
        assert_eq!(descriptor.shader_key(), None);
    }

    #[test]
    fn ground_anchor_defaults_to_group_when_absent() {
        let ron = r#"(
            visuals: [Str("firewall.str")],
            sound: None,
            placement: Ground,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: true,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert_eq!(descriptor.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn effect_data_without_efsts_section_deserializes() {
        let ron = r#"(
            skills: {},
            special: {},
        )"#;

        let data: EffectData = ron::from_str(ron).expect("deserialize");

        assert!(data.efsts.is_empty());
    }

    #[test]
    fn descriptor_without_visuals_deserializes_as_sound_only() {
        let ron = r#"(
            sound: Some("effect/ef_bash.wav"),
            placement: Target,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: false,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert!(descriptor.visuals.is_empty());
        assert_eq!(descriptor.sound.as_deref(), Some("effect/ef_bash.wav"));
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
        assert_eq!(heal.str_name(), Some("heal.strfx.ron"));
        // Sound is relative to `data/wav/`; `_heal_effect.wav` lives at the root
        // (no `effect/` prefix), so the old `effect/_heal_effect.wav` was broken.
        assert_eq!(heal.sound.as_deref(), Some("_heal_effect.wav"));
        assert!(!heal.repeating);

        let stormgust = data.skills.get(&89).expect("WZ_STORMGUST entry");
        assert_eq!(stormgust.placement, EffectPlacement::Ground);
        assert_eq!(stormgust.str_name(), Some("stormgust.str"));
        // `effect/stormgust.wav` does not exist in the GRF; the real sound is
        // `effect/wizard_stormgust.wav`.
        assert_eq!(
            stormgust.sound.as_deref(),
            Some("effect/wizard_stormgust.wav")
        );
        assert!(stormgust.repeating);

        // SM_BASH uses an authored slash because the official effect is hardcoded.
        let bash = data.skills.get(&5).expect("SM_BASH entry");
        assert_eq!(bash.str_name(), Some("bash.strfx.ron"));
        assert_eq!(bash.sound.as_deref(), Some("effect/ef_bash.wav"));

        let special_stormgust = data.special.get(&89).expect("EF_STORMGUST entry");
        assert_eq!(special_stormgust.str_name(), Some("stormgust.str"));
        assert!(special_stormgust.repeating);

        let magnus = data.special.get(&113).expect("EF_MAGNUS entry");
        assert_eq!(magnus.str_name(), Some("magnus.str"));

        assert_eq!(heal.ground_anchor, GroundAnchor::Group);
        assert_eq!(stormgust.ground_anchor, GroundAnchor::Group);

        // 31 EFST_ENERGYCOAT: the persistent Energy Coat aura, migrated out of
        // `skills:` (157 there stays as the one-shot cast flash).
        let energy_coat_aura = data.efsts.get(&31).expect("EFST_ENERGYCOAT entry");
        assert_eq!(energy_coat_aura.str_name(), Some("energycoat.str"));
        assert!(!energy_coat_aura.repeating);

        // The shipped composition example: the cast flash composes the EFST 31
        // recipe unresolved at this layer (the catalog splices it at load).
        let energy_coat_cast = data.skills.get(&157).expect("MG_ENERGYCOAT entry");
        assert_eq!(energy_coat_cast.visuals, vec![Visual::Efst(31)]);
        assert!(!energy_coat_cast.repeating);
    }
}
