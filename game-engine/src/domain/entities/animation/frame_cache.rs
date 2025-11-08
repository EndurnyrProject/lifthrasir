use crate::infrastructure::assets::{RoPaletteAsset, RoSpriteAsset};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Default capacity for the frame cache (1000 frames @ ~100x100 RGBA = ~40MB)
const DEFAULT_FRAME_CACHE_CAPACITY: usize = 1000;

/// Cache key for uniquely identifying a rendered frame
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FrameCacheKey {
    sprite_handle_id: AssetId<RoSpriteAsset>,
    frame_index: usize,
    palette_handle_id: Option<AssetId<RoPaletteAsset>>,
}

impl FrameCacheKey {
    pub fn new(
        sprite: &Handle<RoSpriteAsset>,
        frame_index: usize,
        palette: Option<&Handle<RoPaletteAsset>>,
    ) -> Self {
        Self {
            sprite_handle_id: sprite.id(),
            frame_index,
            palette_handle_id: palette.map(|h| h.id()),
        }
    }
}

/// Global cache for pre-rendered animation frames
#[derive(Resource)]
#[auto_init_resource(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct RoFrameCache {
    cache: LruCache<FrameCacheKey, Handle<Image>>,
}

impl RoFrameCache {
    pub fn new(capacity: usize) -> Self {
        let safe_capacity = std::cmp::max(1, capacity);
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(safe_capacity).expect("capacity is guaranteed to be at least 1"),
            ),
        }
    }

    pub fn get(&mut self, key: &FrameCacheKey) -> Option<Handle<Image>> {
        self.cache.get(key).cloned()
    }

    pub fn insert(&mut self, key: FrameCacheKey, handle: Handle<Image>) {
        self.cache.put(key, handle);
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cache.cap().get()
    }
}

impl Default for RoFrameCache {
    fn default() -> Self {
        Self::new(DEFAULT_FRAME_CACHE_CAPACITY)
    }
}
