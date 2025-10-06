//! Static Sprite PNG Generation System
//!
//! This module provides headless PNG generation from Ragnarok Online sprite files (SPR + ACT)
//! for use in React UI components. The system works independently of Bevy's rendering context
//! and includes a three-tier caching system for optimal performance.
//!
//! # Architecture
//!
//! - **Types**: Request/response structures for sprite rendering
//! - **Renderer**: Headless PNG generator using image crate
//! - **Cache**: Three-tier caching (memory LRU + disk + generation)
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use game_engine::infrastructure::{
//!     assets::hierarchical_manager::HierarchicalAssetManager,
//!     sprite_png::{
//!         cache::SpritePngCache,
//!         renderer::SpriteRenderer,
//!         types::SpritePngRequest,
//!     },
//! };
//! use std::{path::PathBuf, sync::Arc};
//!
//! // Initialize asset manager
//! let asset_manager = HierarchicalAssetManager::new();
//!
//! // Create renderer
//! let renderer = Arc::new(SpriteRenderer::new(asset_manager));
//!
//! // Create cache
//! let cache_dir = PathBuf::from(".cache/sprites");
//! let cache = SpritePngCache::new(renderer, cache_dir, 100).unwrap();
//!
//! // Request a sprite PNG
//! let request = SpritePngRequest {
//!     sprite_path: "data/sprite/몬스터/포링.spr".to_string(),
//!     act_path: None, // Auto-inferred
//!     action_index: 0, // Idle
//!     frame_index: 0,
//!     palette_path: None,
//!     scale: 1.0,
//! };
//!
//! // Get or generate PNG (uses cache when available)
//! let response = cache.get_or_generate(&request).unwrap();
//!
//! // Convert to base64 for web transmission
//! let base64_data = response.to_base64();
//! ```
//!
//! # Cache Flow
//!
//! 1. **Memory Cache (LRU)**: Check if sprite is in memory → instant return
//! 2. **Disk Cache**: Load from `.cache/sprites/{hash}.png` → promote to memory
//! 3. **Generation**: Render via SpriteRenderer → save to both caches
//!
//! # Features
//!
//! - Headless rendering (no Bevy context required)
//! - Three-tier caching for optimal performance
//! - Support for custom palettes (hair colors, etc.)
//! - Pixel-perfect scaling with nearest neighbor
//! - Base64 encoding for web transmission
//! - Batch preloading support

pub mod cache;
pub mod error;
pub mod renderer;
pub mod types;

// Re-export commonly used types
pub use cache::{CacheStats, SpritePngCache};
pub use error::SpritePngError;
pub use renderer::SpriteRenderer;
pub use types::{SpriteBatchRequest, SpritePngRequest, SpritePngResponse};
