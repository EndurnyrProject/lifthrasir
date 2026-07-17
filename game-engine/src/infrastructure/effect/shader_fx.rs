use bevy::asset::LoadState;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// Point-light pop accompanying a shader-fx entry. Mirrors the hardcoded
/// `PointLight` + `LightFade` pair `spawn_jupitel_burst` builds today.
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

/// One shader-fx catalog entry: the `SkillFxParams` payload plus the optional
/// light/garnish children `spawn_shader_fx` (Task 4) will build. `shape`'s
/// meaning is per-`kind`, documented in each kind's wgsl fragment function.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShaderFxEntry {
    pub kind: u32,
    pub primary: (f32, f32, f32, f32),
    pub secondary: (f32, f32, f32, f32),
    pub shape: (f32, f32, f32, f32),
    pub duration: f32,
    #[serde(default)]
    pub light: Option<ShaderFxLight>,
    #[serde(default)]
    pub garnish: Option<ShaderFxGarnish>,
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

        let light = jupitel.light.as_ref().expect("jupitel_thunder light");
        assert_eq!(light.color, (0.55, 0.65, 1.0));
        assert_eq!(light.intensity_scale, 2.0);
        assert_eq!(light.fade, 0.22);

        let garnish = jupitel.garnish.as_ref().expect("jupitel_thunder garnish");
        assert_eq!(garnish.tint, (1.8, 2.4, 4.5, 1.0));
    }

    #[test]
    fn get_returns_none_for_unknown_key() {
        let catalog = ShaderFxCatalog::from_entries(Default::default());

        assert!(catalog.get("unknown_key").is_none());
    }
}
