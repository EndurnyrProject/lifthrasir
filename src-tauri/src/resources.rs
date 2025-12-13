use std::sync::{Arc, RwLock};

use game_engine::infrastructure::assets::{
    ro_asset_source::setup_composite_source_from_config, sources::CompositeAssetSource,
    AssetConfig,
};

use crate::bridge::app_bridge::TauriIncomingEvent;
use crate::bridge::AppBridge;

pub struct PreBevyResources {
    pub app_bridge: AppBridge,
    pub tauri_rx: flume::Receiver<TauriIncomingEvent>,
    pub composite_source: Arc<RwLock<CompositeAssetSource>>,
}

impl PreBevyResources {
    pub fn new() -> Result<Self, String> {
        let (app_bridge, tauri_rx) = AppBridge::new();

        let config = load_asset_config()?;
        let composite_source = setup_composite_source_from_config(&config)
            .map_err(|e| format!("Failed to create composite asset source: {}", e))?;

        Ok(Self {
            app_bridge,
            tauri_rx,
            composite_source: Arc::new(RwLock::new(composite_source)),
        })
    }
}

fn load_asset_config() -> Result<AssetConfig, String> {
    use std::fs;

    let config_path = "assets/loader.data.toml";
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config '{}': {}", config_path, e))?;

    toml::from_str(&content).map_err(|e| format!("Failed to parse config '{}': {}", config_path, e))
}
