use super::{
    hierarchical_reader::HierarchicalAssetReader, sources::CompositeAssetSource, AssetConfig,
};
use bevy::{
    asset::io::{AssetSource, AssetSourceBuilder, AssetSourceId},
    log::{debug, error},
};
use std::sync::{Arc, RwLock};

/// Creates and configures the "ro://" asset source for Ragnarok Online assets.
///
/// This function sets up the custom asset source that uses our hierarchical
/// asset resolution system (data folder > GRF files) while integrating
/// seamlessly with Bevy's asset pipeline.
///
/// # Returns
///
/// Returns a configured `AssetSource` that can be registered with Bevy's
/// asset system using the "ro" source ID.
///
/// # Example
///
/// ```rust
/// // In your app setup:
/// app.register_asset_source(
///     AssetSourceId::from("ro"),
///     create_ro_asset_source(&config)?,
/// );
///
/// // Then load assets with:
/// let handle: Handle<RoGroundAsset> = asset_server.load("ro://data/prontera.gnd");
/// ```
pub fn create_ro_asset_source(
    config: &AssetConfig,
) -> Result<AssetSource, Box<dyn std::error::Error>> {
    debug!("Creating RO asset source with hierarchical resolution");

    // Create the composite source with the same logic as HierarchicalAssetManager
    let composite_source = setup_composite_source_from_config(config)?;
    let composite_arc = Arc::new(RwLock::new(composite_source));

    // Create the asset source using our hierarchical reader
    let asset_source = AssetSourceBuilder::default()
        .with_reader({
            let composite_clone = composite_arc.clone();
            move || Box::new(HierarchicalAssetReader::new(composite_clone.clone()))
        })
        .build(AssetSourceId::Default, false, false);

    debug!("RO asset source created successfully");
    Ok(asset_source.unwrap())
}

/// Sets up CompositeAssetSource from configuration, preserving the exact logic
/// from HierarchicalAssetManager for compatibility.
pub fn setup_composite_source_from_config(
    config: &AssetConfig,
) -> Result<CompositeAssetSource, Box<dyn std::error::Error>> {
    use super::sources::{DataFolderSource, GrfSource};
    use std::path::Path;

    let mut composite = CompositeAssetSource::new();

    // Add data folder source (highest priority - 0)
    let data_folder_path = config.data_folder_path();
    if data_folder_path.exists() {
        let data_source = DataFolderSource::new(data_folder_path.clone());
        debug!("Adding data folder source: {}", data_folder_path.display());
        composite.add_source(Box::new(data_source));
    } else {
        debug!(
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
                        debug!(
                            "Loaded GRF: {} (priority: {})",
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
            error!("Could not find or load GRF file: {}", grf_config.path);
        }
    }

    Ok(composite)
}

/// Create RO asset source from existing HierarchicalAssetManager (migration helper)
///
/// This function helps during the transition period by allowing creation of
/// the new asset source from an existing HierarchicalAssetManager instance.
/// This preserves the existing configuration and sources.
pub fn create_ro_asset_source_from_manager(
    manager: &crate::infrastructure::assets::HierarchicalAssetManager,
) -> AssetSource {
    debug!("Creating RO asset source from existing HierarchicalAssetManager");

    // Extract the composite source from the manager
    // Note: This creates a new Arc pointing to the same CompositeAssetSource
    let composite_source = manager.composite_source().clone();

    let asset_source = AssetSourceBuilder::default()
        .with_reader({
            let composite_clone = composite_source.clone();
            move || Box::new(HierarchicalAssetReader::new(composite_clone.clone()))
        })
        .build(AssetSourceId::Default, false, false);

    debug!("RO asset source created from existing manager");
    asset_source.unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::config::AssetConfig;

    #[test]
    fn test_create_ro_asset_source() {
        // Create a minimal config for testing
        let config = AssetConfig::default();

        // This should not panic, even if no assets are available
        let result = create_ro_asset_source(&config);
        assert!(result.is_ok(), "Should create asset source successfully");
    }
}
