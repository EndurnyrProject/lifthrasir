use crate::{domain::assets::*, infrastructure::assets::*};
use bevy::prelude::*;
use bevy_common_assets::toml::TomlAssetPlugin;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        // Ensure default config exists before starting
        if let Err(e) = ensure_default_config() {
            error!("Failed to ensure default config: {}", e);
        }

        app.add_plugins((
            RoAssetsPlugin,     // This already includes TOML plugin and asset loaders
            AssetLoadingPlugin, // Our new loading state management
        ))
        .add_systems(Startup, setup_debug_systems)
        .add_systems(Update, debug_asset_sources);
    }
}

fn setup_debug_systems() {
    info!("Hierarchical asset loading system initialized");
    info!("Press F1 to display asset source debug information");
}

fn debug_asset_sources(
    manager: Option<Res<HierarchicalAssetManager>>,
    input: Res<ButtonInput<KeyCode>>,
    state: Res<State<AssetLoadingState>>,
) {
    if input.just_pressed(KeyCode::F1) {
        info!("=== Asset Loading Debug Info ===");
        info!("Current loading state: {:?}", state.get());

        if let Some(ref manager) = manager {
            info!("{}", manager.get_debug_info());

            let sources = manager.list_sources();
            info!("Available sources:");
            for (idx, source) in sources.iter().enumerate() {
                info!("  [{}] {}", idx, source);
            }
        } else {
            warn!("HierarchicalAssetManager not available yet");
        }
    }

    if input.just_pressed(KeyCode::F2) {
        if let Some(ref manager) = manager {
            info!("=== Asset Files Sample ===");
            let files = manager.list_files();
            info!("Total files available: {}", files.len());

            // Show first 20 files as sample
            for (idx, file) in files.iter().take(20).enumerate() {
                if let Some(source_info) = manager.get_source_info(file) {
                    info!("  [{}] {} -> {}", idx, file, source_info);
                } else {
                    info!("  [{}] {} -> NOT FOUND", idx, file);
                }
            }

            if files.len() > 20 {
                info!("  ... and {} more files", files.len() - 20);
            }
        }
    }
}
