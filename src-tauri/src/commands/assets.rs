use game_engine::infrastructure::assets::sources::{AssetSource, CompositeAssetSource};
use std::sync::{Arc, RwLock};
use tauri::State;

/// Tauri command to get raw asset bytes from the hierarchical asset source
///
/// This command respects the configured hierarchy:
/// 1. Data folder (priority 0)
/// 2. GRF files (by configured priority)
///
/// # Arguments
/// * `path` - Asset path using forward slashes (will be normalized)
/// * `composite` - Shared CompositeAssetSource from Bevy
///
/// # Returns
/// Raw asset bytes as Vec<u8>
#[tauri::command]
pub async fn get_asset(
    path: String,
    composite: State<'_, Arc<RwLock<CompositeAssetSource>>>,
) -> Result<Vec<u8>, String> {
    let source = composite
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

    // Dereference the RwLockReadGuard to access CompositeAssetSource methods
    (*source)
        .load(&path)
        .map_err(|e| format!("Failed to load asset '{}': {}", path, e))
}
