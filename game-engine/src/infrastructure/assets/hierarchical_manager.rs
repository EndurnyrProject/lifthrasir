use super::{
    AssetConfig,
    sources::{AssetSource, CompositeAssetSource},
};
use bevy::log::error;
use bevy::prelude::*;
use std::sync::{Arc, RwLock};

#[derive(Resource, Clone)]
pub struct HierarchicalAssetManager {
    composite_source: Arc<RwLock<CompositeAssetSource>>,
}

impl HierarchicalAssetManager {
    fn new() -> Self {
        Self {
            composite_source: Arc::new(RwLock::new(CompositeAssetSource::new())),
        }
    }

    pub fn from_config(config: &AssetConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Self::new();
        manager.setup_sources_from_config(config)?;
        Ok(manager)
    }

    pub fn setup_sources_from_config(
        &self,
        config: &AssetConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let composite = super::ro_asset_source::setup_composite_source_from_config(config)?;

        let mut guard = self
            .composite_source
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        *guard = composite;

        debug!("Hierarchical asset manager setup complete");
        debug!("{}", guard.get_debug_info());

        Ok(())
    }

    pub fn list_files(&self) -> Vec<String> {
        match self.composite_source.read() {
            Ok(composite) => composite.list_files(),
            Err(e) => {
                error!("Failed to acquire read lock for list_files: {}", e);
                Vec::new()
            }
        }
    }
}

impl Default for HierarchicalAssetManager {
    fn default() -> Self {
        Self::new()
    }
}
