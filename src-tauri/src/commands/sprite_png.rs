use game_engine::infrastructure::sprite_png::{SpritePngCache, SpritePngRequest};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// Response type for sprite PNG Tauri commands
#[derive(Debug, Serialize)]
pub struct SpritePngCommandResponse {
    /// Data URL in format: "data:image/png;base64,..."
    pub data_url: String,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// X offset from ACT layer (for positioning relative to character anchor)
    pub offset_x: i32,
    /// Y offset from ACT layer (for positioning relative to character anchor)
    pub offset_y: i32,
    /// Whether this response came from cache
    pub from_cache: bool,
}

/// Response type for batch preload operations
#[derive(Debug, Serialize)]
pub struct PreloadBatchResponse {
    /// Cache keys for successfully loaded sprites
    pub successful_keys: Vec<String>,
    /// Cache keys for sprites that failed to load
    pub failed_keys: Vec<String>,
    /// Total number of sprites requested
    pub total: usize,
}

/// Get or generate a sprite PNG for a specific action and frame
///
/// # Arguments
/// * `sprite_path` - Path to .spr file (e.g., "data\\sprite\\몬스터\\포링.spr")
/// * `action_index` - Action index (0 = idle, 1 = walk, etc.)
/// * `frame_index` - Frame index within the action
/// * `act_path` - Optional ACT file path (auto-inferred if None)
/// * `palette_path` - Optional custom palette file path
/// * `scale` - Optional scale factor (defaults to 1.0)
/// * `cache` - Shared SpritePngCache instance
///
/// # Returns
/// SpritePngCommandResponse with data URL and metadata
#[tauri::command]
pub async fn get_sprite_png(
    sprite_path: String,
    action_index: usize,
    frame_index: usize,
    act_path: Option<String>,
    palette_path: Option<String>,
    scale: Option<f32>,
    cache: State<'_, Arc<SpritePngCache>>,
) -> Result<SpritePngCommandResponse, String> {
    // Build request with default scale of 1.0
    let request = SpritePngRequest {
        sprite_path,
        act_path,
        action_index,
        frame_index,
        palette_path,
        scale: scale.unwrap_or(1.0),
    };

    // Use spawn_blocking for CPU-intensive PNG generation to avoid blocking async runtime
    let cache_clone = Arc::clone(&cache);
    let response = tokio::task::spawn_blocking(move || cache_clone.get_or_generate(&request))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| format!("Failed to generate sprite PNG: {}", e))?;

    // Convert PNG data to base64 data URL
    let base64_str = response.to_base64();
    let data_url = format!("data:image/png;base64,{}", base64_str);

    Ok(SpritePngCommandResponse {
        data_url,
        width: response.width,
        height: response.height,
        offset_x: response.offset_x,
        offset_y: response.offset_y,
        from_cache: response.from_cache,
    })
}

/// Preload a batch of sprites into the cache
///
/// This is useful for preloading UI sprites before they're needed,
/// reducing latency when switching between UI screens.
///
/// # Arguments
/// * `requests` - Vector of sprite requests to preload
/// * `cache` - Shared SpritePngCache instance
///
/// # Returns
/// PreloadBatchResponse with successful and failed cache keys
#[tauri::command]
pub async fn preload_sprite_batch(
    requests: Vec<SpritePngRequest>,
    cache: State<'_, Arc<SpritePngCache>>,
) -> Result<PreloadBatchResponse, String> {
    let total = requests.len();

    // Use spawn_blocking for CPU-intensive batch processing
    let cache_clone = Arc::clone(&cache);
    let (successful_keys, failed_keys) = tokio::task::spawn_blocking(move || {
        let mut successes = Vec::with_capacity(requests.len());
        let mut failures = Vec::new();

        for request in requests.iter() {
            // Generate cache key
            let cache_key = request.cache_key();

            // Try to load or generate the sprite
            match cache_clone.get_or_generate(request) {
                Ok(_) => successes.push(cache_key),
                Err(e) => {
                    // Log error and track failure
                    eprintln!("Failed to preload sprite {}: {}", cache_key, e);
                    failures.push(cache_key);
                }
            }
        }

        (successes, failures)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    Ok(PreloadBatchResponse {
        successful_keys,
        failed_keys,
        total,
    })
}

/// Clear all sprite PNGs from the cache (both memory and disk)
///
/// This is useful for debugging or when asset files have been updated.
///
/// # Arguments
/// * `cache` - Shared SpritePngCache instance
///
/// # Returns
/// Unit result on success
#[tauri::command]
pub async fn clear_sprite_cache(cache: State<'_, Arc<SpritePngCache>>) -> Result<(), String> {
    // Use spawn_blocking for potentially I/O-intensive cache clearing
    let cache_clone = Arc::clone(&cache);
    tokio::task::spawn_blocking(move || cache_clone.clear_all())
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| format!("Failed to clear sprite cache: {}", e))?;

    Ok(())
}
