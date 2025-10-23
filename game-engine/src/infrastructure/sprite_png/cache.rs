use super::{
    error::SpritePngError,
    renderer::SpriteRenderer,
    types::{SpritePngRequest, SpritePngResponse},
};
use lru::LruCache;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

/// Two-tier caching system for sprite PNGs:
/// 1. Memory cache (LRU, session-based)
/// 2. Generation (fallback when not cached)
pub struct SpritePngCache {
    /// LRU cache for quick access to recently used sprites
    memory_cache: Arc<Mutex<LruCache<String, SpritePngResponse>>>,

    /// Sprite renderer for generating new PNGs
    renderer: Arc<SpriteRenderer>,
}

impl SpritePngCache {
    /// Create a new sprite PNG cache
    ///
    /// # Arguments
    /// * `renderer` - Sprite renderer for generating PNGs
    /// * `memory_capacity` - Number of sprites to keep in memory (default: 100)
    pub fn new(
        renderer: Arc<SpriteRenderer>,
        memory_capacity: usize,
    ) -> Result<Self, SpritePngError> {
        let capacity = NonZeroUsize::new(memory_capacity).ok_or_else(|| {
            SpritePngError::CacheError("Memory capacity must be non-zero".to_string())
        })?;

        Ok(Self {
            memory_cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            renderer,
        })
    }

    /// Get or generate a sprite PNG
    ///
    /// # Cache Flow
    /// 1. Check memory cache → return if hit
    /// 2. Generate via renderer → save to memory cache
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

        // 2. Generate fresh PNG
        let response = self.renderer.render_to_png(request)?;

        // 3. Save to memory cache
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

    /// Clear all cached data (memory only)
    pub fn clear_all(&self) -> Result<(), SpritePngError> {
        if let Ok(mut memory_cache) = self.memory_cache.lock() {
            memory_cache.clear();
        }

        Ok(())
    }


    /// Get cache statistics
    pub fn get_stats(&self) -> Result<CacheStats, SpritePngError> {
        let memory_count = self
            .memory_cache
            .lock()
            .map_err(|e| SpritePngError::CacheError(format!("Memory cache lock error: {}", e)))?
            .len();

        Ok(CacheStats { memory_count })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of sprites in memory cache
    pub memory_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::hierarchical_manager::HierarchicalAssetManager;
    use std::sync::Arc;

    fn create_test_cache() -> Result<SpritePngCache, SpritePngError> {
        let asset_manager = HierarchicalAssetManager::new();
        let renderer = Arc::new(SpriteRenderer::new(asset_manager));

        SpritePngCache::new(renderer, 10)
    }

    #[test]
    fn test_cache_creation() {
        let result = create_test_cache();
        assert!(result.is_ok());
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
    }
}
