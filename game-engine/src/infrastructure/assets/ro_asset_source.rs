use super::{AssetConfig, sources::CompositeAssetSource};
use bevy::log::{debug, error};

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
