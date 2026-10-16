pub mod composite;
pub mod config;
pub mod data_folder;
pub mod hierarchical_reader;
pub mod pak_source;
pub mod ro_asset_source;
pub mod ro_assets_plugin;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetSourceError {
    #[error("Asset not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Archive error: {0}")]
    Archive(String),
}

pub trait AssetSource: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u32;
    fn exists(&self, path: &str) -> bool;
    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError>;
    fn list_files(&self) -> Vec<String>;
}

impl std::fmt::Debug for dyn AssetSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AssetSource(name: {}, priority: {})",
            self.name(),
            self.priority()
        )
    }
}

pub use composite::*;
pub use data_folder::*;
pub use pak_source::*;

pub use config::*;
pub use ro_asset_source::setup_composite_source_from_config;
pub use ro_assets_plugin::*;

use bevy::asset::{AssetApp, io::AssetSourceBuilder, io::AssetSourceId};
use bevy::prelude::App;
use hierarchical_reader::HierarchicalAssetReader;
use std::sync::{Arc, RwLock};

/// Register `ro://` before adding Bevy's AssetPlugin.
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
