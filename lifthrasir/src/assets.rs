pub use lifthrasir_assets::source::register_ro_asset_source;
use lifthrasir_assets::source::{
    AssetConfig, CompositeAssetSource, setup_composite_source_from_config,
};
use std::sync::{Arc, RwLock};

pub fn load_composite_source() -> Arc<RwLock<CompositeAssetSource>> {
    let config = load_asset_config();
    let composite_source = setup_composite_source_from_config(&config)
        .expect("Failed to create composite asset source");

    Arc::new(RwLock::new(composite_source))
}

fn load_asset_config() -> AssetConfig {
    let config_path = "assets/loader.toml";
    let content = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("Failed to read config '{}': {}", config_path, e));

    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse config '{}': {}", config_path, e))
}
