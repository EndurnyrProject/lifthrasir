//! Effect recipes and procedural presentation definitions.

mod authored;
mod catalog;
pub use authored::{
    AuthoredBlend, AuthoredEffect, AuthoredEffectError, AuthoredKey, AuthoredLayer,
};
pub use catalog::{EffectCatalogError, ResolvedEffectCatalog, resolve_effect_catalog};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Persistent skill-unit visuals, keyed by rAthena skill id, for skills
    /// whose `skills` entry is a one-shot cast. Needed when the server's ruleset
    /// decides whether a skill places a ground unit at all: a pre-renewal song
    /// is a field that follows its performer, the renewal one a party snapshot
    /// with no unit. The cast plays the `skills` entry; the unit group, if the
    /// server places one, plays this. Entries must be `repeating: true`.
    #[serde(default)]
    pub skill_units: BTreeMap<u32, EffectDescriptor>,
    /// Status-aura descriptors, keyed by EFST id. Both the lookup table for
    /// `StatusEffectChanged` and the recipe table [`Visual::Efst`] composes.
    #[serde(default)]
    pub efsts: BTreeMap<u32, EffectDescriptor>,
    /// Procedural shader-fx entries, keyed by [`Visual::Shader`] key.
    #[serde(default)]
    pub shader_fx: BTreeMap<String, ShaderFxEntry>,
}
