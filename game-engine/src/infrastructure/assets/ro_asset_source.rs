use super::{AssetConfig, sources::CompositeAssetSource};
use bevy::log::debug;

/// Sets up CompositeAssetSource from configuration.
pub fn setup_composite_source_from_config(
    config: &AssetConfig,
) -> Result<CompositeAssetSource, Box<dyn std::error::Error>> {
    use super::sources::{DataFolderSource, PakSource};
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

    // Add pak sources sorted by priority
    let archives = config.archives_by_priority();
    for archive_config in archives {
        let archive_path = Path::new(&archive_config.path);

        // Try absolute path first, then relative to assets directory
        let potential_paths = vec![
            archive_path.to_path_buf(),
            Path::new("assets").join(archive_path),
            std::env::current_dir()
                .unwrap()
                .join("assets")
                .join(archive_path),
        ];

        let mut archive_loaded = false;
        let mut last_error = None;
        for potential_path in &potential_paths {
            if potential_path.exists() {
                match PakSource::new(potential_path.clone(), archive_config.priority + 1) {
                    // +1 to ensure data folder has priority 0
                    Ok(pak_source) => {
                        debug!(
                            "Loaded pak: {} (priority: {})",
                            potential_path.display(),
                            archive_config.priority + 1
                        );
                        composite.add_source(Box::new(pak_source));
                        archive_loaded = true;
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                }
            }
        }

        if !archive_loaded {
            let attempted = potential_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let reason = last_error.map(|e| format!(": {e}")).unwrap_or_default();
            return Err(format!(
                "Could not find or load pak file '{}' (tried: {attempted}){reason}",
                archive_config.path
            )
            .into());
        }
    }

    Ok(composite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::{ArchiveConfig, AssetsSection};

    fn config_with_archive(path: String) -> AssetConfig {
        AssetConfig {
            assets: AssetsSection {
                data_folder: "/nonexistent-data-folder-xyz".to_string(),
                archive: vec![ArchiveConfig { path, priority: 0 }],
            },
        }
    }

    #[test]
    fn missing_archive_file_fails_loudly() {
        let config = config_with_archive("/nonexistent-archive-xyz.pak".to_string());

        let err = match setup_composite_source_from_config(&config) {
            Ok(_) => panic!("missing archive must not silently degrade"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("nonexistent-archive-xyz.pak"));
    }

    #[test]
    fn corrupt_archive_file_fails_loudly_with_pak_source_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.pak");
        std::fs::write(&path, b"not a zip file").unwrap();

        let config = config_with_archive(path.display().to_string());

        let err = match setup_composite_source_from_config(&config) {
            Ok(_) => panic!("corrupt archive must not silently degrade"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("grf-utils pack"));
    }
}
