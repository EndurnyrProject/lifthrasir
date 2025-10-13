use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Request to render a specific sprite frame to PNG
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpritePngRequest {
    /// Path to sprite file (e.g., "data/sprite/몬스터/포링.spr")
    pub sprite_path: String,

    /// Path to ACT animation file (auto-inferred if None by replacing .spr with .act)
    pub act_path: Option<String>,

    /// Action index (0 = idle, 1 = walk, etc.)
    pub action_index: usize,

    /// Frame index within the action
    pub frame_index: usize,

    /// Optional path to custom palette file (1024 bytes = 256 colors × 4 bytes RGBA)
    pub palette_path: Option<String>,

    /// Scale factor (1.0 = original size, 2.0 = double size, etc.)
    pub scale: f32,
}

// Custom Hash implementation that handles f32 by converting to bits
impl Hash for SpritePngRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sprite_path.hash(state);
        self.act_path.hash(state);
        self.action_index.hash(state);
        self.frame_index.hash(state);
        self.palette_path.hash(state);
        // Hash the bit representation of f32 for deterministic hashing
        self.scale.to_bits().hash(state);
    }
}

// Custom Eq implementation
impl Eq for SpritePngRequest {}

impl SpritePngRequest {
    /// Generate a unique cache key for this request
    /// Uses hash-based key generation for consistent caching
    pub fn cache_key(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Auto-infer ACT path from sprite path if not provided
    pub fn get_act_path(&self) -> String {
        if let Some(ref act_path) = self.act_path {
            act_path.clone()
        } else {
            // Replace .spr extension with .act
            self.sprite_path
                .replace(".spr", ".act")
                .replace(".SPR", ".act")
        }
    }
}

/// Response containing rendered PNG data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpritePngResponse {
    /// PNG-encoded image data
    pub png_data: Vec<u8>,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,

    /// X offset from ACT layer (for positioning relative to character anchor)
    pub offset_x: i32,

    /// Y offset from ACT layer (for positioning relative to character anchor)
    /// Note: RO uses Y-negative=up, so this may need negation when applying to CSS
    pub offset_y: i32,

    /// Whether this response came from cache (true) or was freshly generated (false)
    pub from_cache: bool,
}

impl SpritePngResponse {
    /// Encode PNG data to base64 for web transmission
    pub fn to_base64(&self) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &self.png_data)
    }

    /// Create response from PNG data
    pub fn new(
        png_data: Vec<u8>,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
        from_cache: bool,
    ) -> Self {
        Self {
            png_data,
            width,
            height,
            offset_x,
            offset_y,
            from_cache,
        }
    }
}

/// Batch request for multiple sprites (for preloading)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteBatchRequest {
    /// List of sprite requests to process
    pub requests: Vec<SpritePngRequest>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_consistency() {
        let request1 = SpritePngRequest {
            sprite_path: "data/sprite/test.spr".to_string(),
            act_path: None,
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        let request2 = SpritePngRequest {
            sprite_path: "data/sprite/test.spr".to_string(),
            act_path: None,
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        // Same requests should generate same cache key
        assert_eq!(request1.cache_key(), request2.cache_key());
    }

    #[test]
    fn test_cache_key_uniqueness() {
        let request1 = SpritePngRequest {
            sprite_path: "data/sprite/test.spr".to_string(),
            act_path: None,
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        let request2 = SpritePngRequest {
            sprite_path: "data/sprite/test.spr".to_string(),
            act_path: None,
            action_index: 1, // Different action
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        // Different requests should generate different cache keys
        assert_ne!(request1.cache_key(), request2.cache_key());
    }

    #[test]
    fn test_auto_infer_act_path() {
        let request = SpritePngRequest {
            sprite_path: "data/sprite/monster.spr".to_string(),
            act_path: None,
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        assert_eq!(request.get_act_path(), "data/sprite/monster.act");
    }

    #[test]
    fn test_auto_infer_act_path_uppercase() {
        let request = SpritePngRequest {
            sprite_path: "data/sprite/MONSTER.SPR".to_string(),
            act_path: None,
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        assert_eq!(request.get_act_path(), "data/sprite/MONSTER.act");
    }

    #[test]
    fn test_explicit_act_path() {
        let request = SpritePngRequest {
            sprite_path: "data/sprite/monster.spr".to_string(),
            act_path: Some("data/sprite/custom.act".to_string()),
            action_index: 0,
            frame_index: 0,
            palette_path: None,
            scale: 1.0,
        };

        assert_eq!(request.get_act_path(), "data/sprite/custom.act");
    }

    #[test]
    fn test_base64_encoding() {
        let response = SpritePngResponse {
            png_data: vec![0x89, 0x50, 0x4E, 0x47], // PNG header
            width: 64,
            height: 64,
            offset_x: 0,
            offset_y: 0,
            from_cache: false,
        };

        let base64_str = response.to_base64();
        assert!(!base64_str.is_empty());
        assert!(base64_str.len() > 4);
    }
}
