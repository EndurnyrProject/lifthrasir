use bevy::asset::LoadState;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// Point-light pop accompanying a shader-fx entry. Drives the `PointLight` +
/// `LightFade` pair `spawn_shader_fx` builds.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShaderFxLight {
    pub color: (f32, f32, f32),
    pub intensity_scale: f32,
    pub fade: f32,
}

/// Tint for the tintable spark garnish child (Task 3).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShaderFxGarnish {
    pub tint: (f32, f32, f32, f32),
}

/// An animated classic GRF texture: the ordered frame paths (each relative to
/// the `ro://` root) plus the playback rate. Used wherever a `SkillFxMaterial`
/// binds a texture — burst or projectile — cycling the bound frame at `fps`,
/// looping. Prefer this over a single `texture` when the source art is a series
/// (most RO effect textures are, e.g. `thunder_ball_0001..0006`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
/// light/garnish children `spawn_shader_fx` builds. `shape`'s
/// meaning is per-`kind`, documented in each kind's wgsl fragment function.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct ShaderFxAsset(pub BTreeMap<String, ShaderFxEntry>);

#[derive(Resource)]
pub struct ShaderFxCatalog {
    entries: HashMap<String, ShaderFxEntry>,
}

impl ShaderFxCatalog {
    pub fn from_entries(data: BTreeMap<String, ShaderFxEntry>) -> Self {
        Self {
            entries: data.into_iter().collect(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&ShaderFxEntry> {
        self.entries.get(key)
    }
}

#[derive(Resource)]
pub struct ShaderFxHandle(Handle<ShaderFxAsset>);

pub fn start_loading_shader_fx(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/shader_fx.ron");
    commands.insert_resource(ShaderFxHandle(handle));
    debug!("Loading shader fx data RON");
}

pub fn process_loaded_shader_fx(
    mut commands: Commands,
    handle: Option<Res<ShaderFxHandle>>,
    shader_fx_assets: Res<Assets<ShaderFxAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/shader_fx.ron: {:?}. It is hand-authored at assets/data/ron/shader_fx.ron.",
            err
        );
        commands.remove_resource::<ShaderFxHandle>();
        return;
    }

    let Some(asset) = shader_fx_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(ShaderFxCatalog::from_entries(asset.0.clone()));
    commands.remove_resource::<ShaderFxHandle>();
    debug!("Shader fx catalog created from RON");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_fx_ron_deserializes_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/shader_fx.ron");
        let asset = ron::from_str::<ShaderFxAsset>(ron).expect("deserialize");
        let catalog = ShaderFxCatalog::from_entries(asset.0);

        let jupitel = catalog
            .get("jupitel_thunder")
            .expect("jupitel_thunder entry");
        assert_eq!(jupitel.kind, 0);
        assert_eq!(jupitel.primary, (3.5, 4.0, 6.0, 1.0));
        assert_eq!(jupitel.secondary, (0.25, 0.55, 3.2, 1.0));
        assert_eq!(jupitel.shape, (2.0, 7.0, 24.0, 0.0));
        assert_eq!(jupitel.duration, 0.7);
        assert_eq!(jupitel.scale, 26.0);

        let light = jupitel.light.as_ref().expect("jupitel_thunder light");
        assert_eq!(light.color, (0.55, 0.65, 1.0));
        assert_eq!(light.intensity_scale, 2.0);
        assert_eq!(light.fade, 0.22);

        let garnish = jupitel.garnish.as_ref().expect("jupitel_thunder garnish");
        assert_eq!(garnish.tint, (1.8, 2.4, 4.5, 1.0));

        assert_eq!(
            jupitel.texture.as_deref(),
            Some("data/texture/effect/thunder_pang.bmp")
        );

        let travel = jupitel.travel.as_ref().expect("jupitel_thunder travel");
        assert_eq!(travel.speed, 100.0);
        assert_eq!(travel.scale, 9.0);
        let frames = travel.frames.as_ref().expect("jupitel projectile frames");
        assert_eq!(frames.paths.len(), 6);
        assert_eq!(frames.fps, 18.0);
        assert_eq!(frames.paths[0], "data/texture/effect/thunder_ball_0001.bmp");

        // A single-orb bolt still travels, with no frame series.
        let fire = catalog.get("fire_bolt").expect("fire_bolt entry");
        let fire_travel = fire.travel.as_ref().expect("fire_bolt travel");
        assert_eq!(
            fire_travel.texture.as_deref(),
            Some("data/texture/effect/fireorb.bmp")
        );
        assert!(fire_travel.frames.is_none());
    }

    #[test]
    fn entry_deserializes_optional_texture() {
        let ron = r#"{
            "textured_fx": (
                kind: 1,
                primary: (1.0, 1.0, 1.0, 1.0),
                secondary: (1.0, 1.0, 1.0, 1.0),
                shape: (0.0, 0.0, 0.0, 0.0),
                duration: 0.5,
                scale: 10.0,
                texture: Some("data/texture/effect/fire_fall_b.bmp"),
            ),
        }"#;
        let asset = ron::from_str::<ShaderFxAsset>(ron).expect("deserialize");
        let entry = asset.0.get("textured_fx").expect("textured_fx entry");
        assert_eq!(
            entry.texture.as_deref(),
            Some("data/texture/effect/fire_fall_b.bmp")
        );
    }

    #[test]
    fn get_returns_none_for_unknown_key() {
        let catalog = ShaderFxCatalog::from_entries(Default::default());

        assert!(catalog.get("unknown_key").is_none());
    }
}
