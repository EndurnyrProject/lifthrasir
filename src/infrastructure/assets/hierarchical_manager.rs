use super::{
    AssetConfig,
    sources::{AssetSource, AssetSourceError, CompositeAssetSource, DataFolderSource, GrfSource},
};
use bevy::log::{error, info, warn};
use bevy::prelude::*;
use std::path::Path;
use std::sync::{Arc, RwLock};

#[derive(Resource, Clone)]
pub struct HierarchicalAssetManager {
    composite_source: Arc<RwLock<CompositeAssetSource>>,
}

impl HierarchicalAssetManager {
    pub fn new() -> Self {
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
        let mut composite = self
            .composite_source
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        composite.clear_cache();

        // Add data folder source (highest priority - 0)
        let data_folder_path = config.data_folder_path();
        if data_folder_path.exists() {
            let data_source = DataFolderSource::new(data_folder_path.clone());
            info!("Adding data folder source: {}", data_folder_path.display());
            composite.add_source(Box::new(data_source));
        } else {
            info!(
                "Data folder not found, skipping: {}",
                data_folder_path.display()
            );
        }

        // Add GRF sources sorted by priority
        let grf_files = config.grf_files_by_priority();
        for grf_config in grf_files {
            let grf_path = Path::new(&grf_config.path);

            // Try absolute path first, then relative to assets directory
            let potential_paths = vec![
                grf_path.to_path_buf(),
                Path::new("assets").join(grf_path),
                std::env::current_dir()
                    .unwrap()
                    .join("assets")
                    .join(grf_path),
            ];

            let mut grf_loaded = false;
            for potential_path in potential_paths {
                if potential_path.exists() {
                    match GrfSource::new(potential_path.clone(), grf_config.priority + 1) {
                        // +1 to ensure data folder has priority 0
                        Ok(grf_source) => {
                            info!(
                                "Successfully loaded GRF: {} (priority: {})",
                                potential_path.display(),
                                grf_config.priority + 1
                            );
                            composite.add_source(Box::new(grf_source));
                            grf_loaded = true;
                            break;
                        }
                        Err(e) => {
                            error!("Failed to load GRF {}: {}", potential_path.display(), e);
                        }
                    }
                }
            }

            if !grf_loaded {
                warn!("Could not find or load GRF file: {}", grf_config.path);
            }
        }

        info!("Hierarchical asset manager setup complete");
        info!("{}", composite.get_debug_info());

        Ok(())
    }

    pub fn exists(&self, path: &str) -> bool {
        match self.composite_source.read() {
            Ok(composite) => composite.exists(path),
            Err(e) => {
                error!("Failed to acquire read lock for exists check: {}", e);
                false
            }
        }
    }

    pub fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        match self.composite_source.read() {
            Ok(composite) => composite.load(path),
            Err(e) => {
                error!("Failed to acquire read lock for load: {}", e);
                Err(AssetSourceError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Lock error: {}", e),
                )))
            }
        }
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

    pub fn get_source_info(&self, path: &str) -> Option<String> {
        match self.composite_source.read() {
            Ok(composite) => composite.get_source_info(path),
            Err(e) => {
                error!("Failed to acquire read lock for get_source_info: {}", e);
                None
            }
        }
    }

    pub fn list_sources(&self) -> Vec<String> {
        match self.composite_source.read() {
            Ok(composite) => composite.list_sources(),
            Err(e) => {
                error!("Failed to acquire read lock for list_sources: {}", e);
                Vec::new()
            }
        }
    }

    pub fn get_debug_info(&self) -> String {
        match self.composite_source.read() {
            Ok(composite) => composite.get_debug_info(),
            Err(e) => format!("Failed to acquire read lock: {}", e),
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut composite) = self.composite_source.write() {
            composite.clear_cache();
        }
    }

    pub fn warm_cache(&self, common_paths: &[&str]) {
        if let Ok(mut composite) = self.composite_source.write() {
            composite.warm_cache(common_paths);
        }
    }

    pub fn reload_from_config(
        &self,
        config: &AssetConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Reloading hierarchical asset manager from config");
        self.setup_sources_from_config(config)
    }

    /// Get access to the internal composite source for migration purposes
    pub fn composite_source(&self) -> &Arc<RwLock<CompositeAssetSource>> {
        &self.composite_source
    }
}

impl Default for HierarchicalAssetManager {
    fn default() -> Self {
        Self::new()
    }
}
