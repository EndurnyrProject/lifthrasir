use super::{
    error::SpritePngError,
    renderer::SpriteRenderer,
    types::{SpritePngRequest, SpritePngResponse},
};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Metadata for a cached sprite PNG
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
    file_size: usize,
}

/// Cache metadata stored as JSON
#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMetadata {
    entries: HashMap<String, CacheEntry>,
}

/// Three-tier caching system for sprite PNGs:
/// 1. Memory cache (LRU, ~100 sprites)
/// 2. Disk cache (persistent between runs)
/// 3. Generation (fallback when not cached)
pub struct SpritePngCache {
    /// LRU cache for quick access to recently used sprites
    memory_cache: Arc<Mutex<LruCache<String, SpritePngResponse>>>,

    /// Directory for disk cache storage
    cache_dir: PathBuf,

    /// Metadata about cached files
    metadata: Arc<Mutex<CacheMetadata>>,

    /// Sprite renderer for generating new PNGs
    renderer: Arc<SpriteRenderer>,
}

impl SpritePngCache {
    /// Create a new sprite PNG cache
    ///
    /// # Arguments
    /// * `renderer` - Sprite renderer for generating PNGs
    /// * `cache_dir` - Directory to store cached PNG files
    /// * `memory_capacity` - Number of sprites to keep in memory (default: 100)
    pub fn new(
        renderer: Arc<SpriteRenderer>,
        cache_dir: PathBuf,
        memory_capacity: usize,
    ) -> Result<Self, SpritePngError> {
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                SpritePngError::CacheError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        // Load metadata from disk
        let metadata_path = cache_dir.join("cache.json");
        let metadata = if metadata_path.exists() {
            let metadata_str = fs::read_to_string(&metadata_path).map_err(|e| {
                SpritePngError::CacheError(format!("Failed to read metadata: {}", e))
            })?;
            serde_json::from_str(&metadata_str).unwrap_or_default()
        } else {
            CacheMetadata::default()
        };

        let capacity = NonZeroUsize::new(memory_capacity).ok_or_else(|| {
            SpritePngError::CacheError("Memory capacity must be non-zero".to_string())
        })?;

        Ok(Self {
            memory_cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            cache_dir,
            metadata: Arc::new(Mutex::new(metadata)),
            renderer,
        })
    }

    /// Get or generate a sprite PNG
    ///
    /// # Cache Flow
    /// 1. Check memory cache → return if hit
    /// 2. Check disk cache → promote to memory if hit
    /// 3. Generate via renderer → save to both caches
    pub fn get_or_generate(
        &self,
        request: &SpritePngRequest,
    ) -> Result<SpritePngResponse, SpritePngError> {
        let cache_key = request.cache_key();

        // 1. Check memory cache
        {
            let mut memory_cache = self.memory_cache.lock().map_err(|e| {
                SpritePngError::CacheError(format!("Memory cache lock error: {}", e))
            })?;

            if let Some(response) = memory_cache.get(&cache_key) {
                let mut cached_response = response.clone();
                cached_response.from_cache = true;
                return Ok(cached_response);
            }
        }

        // 2. Check disk cache
        let cache_file_path = self.cache_dir.join(format!("{}.png", cache_key));
        if cache_file_path.exists() {
            // Load from disk
            match self.load_from_disk(&cache_key, &cache_file_path) {
                Ok(response) => {
                    // Promote to memory cache
                    if let Ok(mut memory_cache) = self.memory_cache.lock() {
                        memory_cache.put(cache_key, response.clone());
                    }
                    return Ok(response);
                }
                Err(e) => {
                    // Disk cache corrupted, remove the file and regenerate
                    eprintln!(
                        "Disk cache corrupted for {}: {}, regenerating",
                        cache_key, e
                    );
                    let _ = fs::remove_file(&cache_file_path);

                    // Remove stale metadata entry to prevent unbounded growth
                    if let Ok(mut metadata) = self.metadata.lock() {
                        if metadata.entries.remove(&cache_key).is_some() {
                            let _ = self.save_metadata(&metadata);
                        }
                    }
                }
            }
        }

        // 3. Generate fresh PNG
        let response = self.renderer.render_to_png(request)?;

        // Save to disk cache
        self.save_to_disk(&cache_key, &response, &cache_file_path)?;

        // Save to memory cache
        {
            if let Ok(mut memory_cache) = self.memory_cache.lock() {
                memory_cache.put(cache_key, response.clone());
            }
        }

        Ok(response)
    }

    /// Preload a batch of sprites into cache
    ///
    /// Useful for preloading commonly used sprites (e.g., all character customization options)
    pub fn preload_batch(
        &self,
        requests: &[SpritePngRequest],
    ) -> Result<Vec<SpritePngResponse>, SpritePngError> {
        requests
            .iter()
            .map(|request| self.get_or_generate(request))
            .collect()
    }

    /// Clear all cached data (memory + disk)
    pub fn clear_all(&self) -> Result<(), SpritePngError> {
        // Clear memory cache
        if let Ok(mut memory_cache) = self.memory_cache.lock() {
            memory_cache.clear();
        }

        // Clear disk cache
        if self.cache_dir.exists() {
            // Remove all .png files
            let entries = fs::read_dir(&self.cache_dir).map_err(|e| {
                SpritePngError::CacheError(format!("Failed to read cache directory: {}", e))
            })?;

            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "png" {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }

        // Clear metadata
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.entries.clear();
            self.save_metadata(&metadata)?;
        }

        Ok(())
    }

    /// Load a PNG from disk cache
    fn load_from_disk(
        &self,
        cache_key: &str,
        path: &PathBuf,
    ) -> Result<SpritePngResponse, SpritePngError> {
        let png_data = fs::read(path)
            .map_err(|e| SpritePngError::CacheError(format!("Failed to read from disk: {}", e)))?;

        // Get dimensions from metadata
        let metadata = self
            .metadata
            .lock()
            .map_err(|e| SpritePngError::CacheError(format!("Metadata lock error: {}", e)))?;

        let entry = metadata
            .entries
            .get(cache_key)
            .ok_or_else(|| SpritePngError::CacheError("Metadata entry not found".to_string()))?;

        Ok(SpritePngResponse::new(
            png_data,
            entry.width,
            entry.height,
            entry.offset_x,
            entry.offset_y,
            true, // From cache
        ))
    }

    /// Save a PNG to disk cache
    fn save_to_disk(
        &self,
        cache_key: &str,
        response: &SpritePngResponse,
        path: &PathBuf,
    ) -> Result<(), SpritePngError> {
        // Write PNG file
        let mut file = fs::File::create(path).map_err(|e| {
            SpritePngError::CacheError(format!("Failed to create cache file: {}", e))
        })?;

        file.write_all(&response.png_data).map_err(|e| {
            SpritePngError::CacheError(format!("Failed to write cache file: {}", e))
        })?;

        // Update metadata
        let mut metadata = self
            .metadata
            .lock()
            .map_err(|e| SpritePngError::CacheError(format!("Metadata lock error: {}", e)))?;

        metadata.entries.insert(
            cache_key.to_string(),
            CacheEntry {
                width: response.width,
                height: response.height,
                offset_x: response.offset_x,
                offset_y: response.offset_y,
                file_size: response.png_data.len(),
            },
        );

        self.save_metadata(&metadata)?;

        Ok(())
    }

    /// Save metadata to disk atomically
    ///
    /// Uses a write-then-rename pattern to ensure atomicity.
    /// If the process crashes during write, the original file remains intact.
    fn save_metadata(&self, metadata: &CacheMetadata) -> Result<(), SpritePngError> {
        let metadata_path = self.cache_dir.join("cache.json");
        let temp_path = self.cache_dir.join("cache.json.tmp");

        let metadata_str = serde_json::to_string_pretty(metadata).map_err(|e| {
            SpritePngError::CacheError(format!("Failed to serialize metadata: {}", e))
        })?;

        // Write to temporary file first
        fs::write(&temp_path, metadata_str).map_err(|e| {
            SpritePngError::CacheError(format!("Failed to write temporary metadata: {}", e))
        })?;

        // Atomically rename temporary file to final destination
        // This ensures the original file is never left in a corrupted state
        fs::rename(&temp_path, &metadata_path).map_err(|e| {
            SpritePngError::CacheError(format!("Failed to rename metadata file: {}", e))
        })?;

        Ok(())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> Result<CacheStats, SpritePngError> {
        let memory_count = self
            .memory_cache
            .lock()
            .map_err(|e| SpritePngError::CacheError(format!("Memory cache lock error: {}", e)))?
            .len();

        let metadata = self
            .metadata
            .lock()
            .map_err(|e| SpritePngError::CacheError(format!("Metadata lock error: {}", e)))?;

        let disk_count = metadata.entries.len();
        let total_size: usize = metadata.entries.values().map(|e| e.file_size).sum();

        Ok(CacheStats {
            memory_count,
            disk_count,
            total_size_bytes: total_size,
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of sprites in memory cache
    pub memory_count: usize,

    /// Number of sprites in disk cache
    pub disk_count: usize,

    /// Total size of disk cache in bytes
    pub total_size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::hierarchical_manager::HierarchicalAssetManager;
    use std::sync::Arc;

    fn create_test_cache() -> Result<SpritePngCache, SpritePngError> {
        let temp_dir = std::env::temp_dir().join("lifthrasir_test_cache");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let asset_manager = HierarchicalAssetManager::new();
        let renderer = Arc::new(SpriteRenderer::new(asset_manager));

        SpritePngCache::new(renderer, temp_dir, 10)
    }

    #[test]
    fn test_cache_creation() {
        let result = create_test_cache();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_directory_creation() {
        let temp_dir = std::env::temp_dir().join("lifthrasir_test_cache_mkdir");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let asset_manager = HierarchicalAssetManager::new();
        let renderer = Arc::new(SpriteRenderer::new(asset_manager));

        let result = SpritePngCache::new(renderer, temp_dir.clone(), 10);
        assert!(result.is_ok());
        assert!(temp_dir.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_clear_all() {
        let cache = create_test_cache();
        assert!(cache.is_ok());

        let cache = cache.unwrap();
        let result = cache.clear_all();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_stats() {
        let cache = create_test_cache();
        assert!(cache.is_ok());

        let cache = cache.unwrap();
        let stats = cache.get_stats();
        assert!(stats.is_ok());

        let stats = stats.unwrap();
        assert_eq!(stats.memory_count, 0);
        assert_eq!(stats.disk_count, 0);
    }

    #[test]
    fn test_metadata_persistence() {
        let temp_dir = std::env::temp_dir().join("lifthrasir_test_cache_metadata");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let asset_manager = HierarchicalAssetManager::new();
        let renderer = Arc::new(SpriteRenderer::new(asset_manager));

        // Create cache and save metadata
        {
            let cache = SpritePngCache::new(renderer.clone(), temp_dir.clone(), 10);
            assert!(cache.is_ok());
            let cache = cache.unwrap();

            // Add a fake entry to metadata
            {
                let mut metadata = cache.metadata.lock().unwrap();
                metadata.entries.insert(
                    "test_key".to_string(),
                    CacheEntry {
                        width: 64,
                        height: 64,
                        offset_x: 0,
                        offset_y: 0,
                        file_size: 1024,
                    },
                );
            }
            let metadata = cache.metadata.lock().unwrap();
            let _ = cache.save_metadata(&metadata);
        }

        // Create new cache instance and verify metadata was loaded
        {
            let cache = SpritePngCache::new(renderer, temp_dir.clone(), 10);
            assert!(cache.is_ok());
            let cache = cache.unwrap();

            let metadata = cache.metadata.lock().unwrap();
            assert!(metadata.entries.contains_key("test_key"));
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
