use anyhow::Result;
use bevy::prelude::*;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    source::{AssetConfig, register_ro_asset_source, setup_composite_source_from_config},
};
use lifthrasir_runtime::{RuntimeCorePlugin, WorldRenderPlugin};
use std::sync::{Arc, RwLock};

// This is the explicit integration surface consumed by the editor's viewport.
// Reusing these components avoids duplicating glTF, prop and emitter loading.
pub(crate) use lifthrasir_runtime::world::{
    LifMapData, LifMapNodeIndex, LifPropRef, MapSceneRequest, PropSceneContext,
};

/// Must run before DefaultPlugins installs AssetPlugin.
pub(crate) fn register_source(app: &mut App) -> Result<()> {
    let config: AssetConfig = toml::from_str(&std::fs::read_to_string("assets/loader.toml")?)?;
    let source = setup_composite_source_from_config(&config)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    register_ro_asset_source(app, Arc::new(RwLock::new(source)));
    Ok(())
}

pub(crate) struct EditorContentPlugin;

impl Plugin for EditorContentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, WorldRenderPlugin));
    }
}
