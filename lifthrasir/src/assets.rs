use std::sync::{Arc, RwLock};

use bevy::asset::{AssetApp, io::AssetSourceBuilder, io::AssetSourceId};
use bevy::prelude::*;

use game_engine::infrastructure::assets::{
    AssetConfig, SharedCompositeAssetSource, hierarchical_reader::HierarchicalAssetReader,
    ro_asset_source::setup_composite_source_from_config, sources::CompositeAssetSource,
};

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

pub fn register_ro_asset_source(
    app: &mut App,
    composite_source: Arc<RwLock<CompositeAssetSource>>,
) {
    app.register_asset_source(
        AssetSourceId::Name("ro".into()),
        AssetSourceBuilder::new({
            let composite_clone = composite_source.clone();
            move || Box::new(HierarchicalAssetReader::new(composite_clone.clone()))
        }),
    );

    app.insert_resource(SharedCompositeAssetSource(composite_source));
}
