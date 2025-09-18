use super::{
    AssetConfig,
    HierarchicalAssetManager,
    hierarchical_reader::HierarchicalAssetReader,
    loading_states::ConfigAssets,
    ro_asset_source::setup_composite_source_from_config,
};
use bevy::{
    app::{App, Plugin},
    asset::{
        AssetApp,
        io::{AssetSourceBuilder, AssetSourceId},
    },
    log::{error, info},
    prelude::*,
};
use std::sync::{Arc, RwLock};
use toml;

/// Enhanced RO Assets plugin that optionally sets up the unified asset source
pub struct RoAssetsPlugin {
    /// Whether to register the unified "ro://" asset source
    pub enable_unified_source: bool,
}

impl Default for RoAssetsPlugin {
    fn default() -> Self {
        Self {
            enable_unified_source: false, // Start with false for backward compatibility
        }
    }
}

impl RoAssetsPlugin {
    /// Create plugin with unified asset source enabled
    pub fn with_unified_source() -> Self {
        Self {
            enable_unified_source: true,
        }
    }

    /// Load configuration from file during app setup
    fn load_config_from_file(&self) -> Result<AssetConfig, Box<dyn std::error::Error>> {
        use std::fs;

        let config_path = "assets/loader.data.toml";
        info!("Loading asset configuration from: {}", config_path);

        let config_content = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config file '{}': {}", config_path, e))?;

        let config: AssetConfig = toml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file '{}': {}", config_path, e))?;

        info!(
            "Successfully loaded asset configuration with {} GRF sources",
            config.assets.grf.len()
        );
        Ok(config)
    }
}

impl Plugin for RoAssetsPlugin {
    fn build(&self, app: &mut App) {
        // Register unified asset source if enabled
        if self.enable_unified_source {
            info!("Registering unified RO asset source as 'ro://'");

            // Load the configuration from file during app setup
            let config = self.load_config_from_file().expect(
                "Failed to load asset config - unified source requires valid configuration",
            );

            let composite_source = setup_composite_source_from_config(&config).expect(
                "Failed to create composite asset source - check GRF files and configuration",
            );

            let composite_arc = Arc::new(RwLock::new(composite_source));

            // Register the "ro://" asset source
            app.register_asset_source(
                AssetSourceId::Name("ro".into()),
                AssetSourceBuilder::default().with_reader({
                    let composite_clone = composite_arc.clone();
                    move || Box::new(HierarchicalAssetReader::new(composite_clone.clone()))
                }),
            );

            // Create and register HierarchicalAssetManager as a resource
            let manager = HierarchicalAssetManager::from_config(&config).expect(
                "Failed to create HierarchicalAssetManager from config",
            );
            app.insert_resource(manager);

            info!("Successfully registered 'ro://' asset source and HierarchicalAssetManager");
        }
    }
}

/// Resource to track if unified asset source has been registered
#[derive(Resource, Default)]
pub struct UnifiedAssetSourceRegistered(pub bool);

/// System to register the unified asset source once configuration is loaded
fn register_unified_asset_source(
    commands: Commands,
    config_assets: Res<ConfigAssets>,
    configs: Res<Assets<AssetConfig>>,
    app: Commands,
    mut registered: Local<bool>,
) {
    if *registered {
        return; // Already registered
    }

    if let Some(config) = configs.get(&config_assets.config) {
        info!("Registering unified RO asset source as 'ro://' with loaded config");

        match setup_composite_source_from_config(&config) {
            Ok(composite_source) => {
                let composite_arc = Arc::new(RwLock::new(composite_source));

                // Note: Unfortunately, we can't dynamically register asset sources after app setup
                // This is a limitation of Bevy's asset system architecture
                // The asset source must be registered during app building phase

                info!(
                    "Composite source created successfully, but asset source registration must happen during app setup"
                );
                *registered = true;
            }
            Err(e) => {
                error!("Failed to create composite asset source: {}", e);
            }
        }
    }
}

// Re-export all the asset types from the main loaders module
