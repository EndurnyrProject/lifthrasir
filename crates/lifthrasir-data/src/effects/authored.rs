//! Serializable authored effects and validation, independent of asset loading.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Hand-authored effect from a `*.strfx.ron` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredEffect {
    pub fps: u32,
    pub layers: Vec<AuthoredLayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredLayer {
    /// Full asset paths; per-key `texture_index` selects into this list.
    pub textures: Vec<String>,
    pub blend: AuthoredBlend,
    pub keys: Vec<AuthoredKey>,
}

/// Named blend mode, mapped to the renderer's blend mode at loading time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoredBlend {
    Add,
    Blend,
    Multiply,
}

/// A keyframe at a STR frame index. Exactly one of `quad` or `xy` must be set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredKey {
    pub at: u32,
    #[serde(default)]
    pub offset: (f32, f32),
    /// `(w, h)` centered on the origin. Mutually exclusive with `xy`.
    #[serde(default)]
    pub quad: Option<(f32, f32)>,
    /// Raw STR corner array. Mutually exclusive with `quad`.
    #[serde(default)]
    pub xy: Option<[f32; 8]>,
    /// STR `uv[8]`; defaults to the full quad when converted.
    #[serde(default)]
    pub uv: Option<[f32; 8]>,
    /// RGBA in 0-255 (divided by 255 at interpolation time).
    pub color: (u8, u8, u8, u8),
    /// Raw STR angle units.
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub texture_index: usize,
}

/// Validation failures for an authored effect, with layer/key context.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthoredEffectError {
    #[error("authored effect has no layers")]
    NoLayers,
    #[error("layer {layer} has no textures")]
    NoTextures { layer: usize },
    #[error("layer {layer} has no keys")]
    NoKeys { layer: usize },
    #[error("layer {layer} keys are not strictly ascending by `at` (at key index {key})")]
    UnsortedKeys { layer: usize, key: usize },
    #[error(
        "layer {layer} key {key} texture_index {index} out of range (layer has {len} textures)"
    )]
    TextureIndexOutOfRange {
        layer: usize,
        key: usize,
        index: usize,
        len: usize,
    },
    #[error("layer {layer} key {key} must set exactly one of `quad` or `xy`")]
    QuadXyAmbiguous { layer: usize, key: usize },
}

impl AuthoredEffect {
    /// Validate the same layer/key constraints required by STR playback.
    pub fn validate(&self) -> Result<(), AuthoredEffectError> {
        if self.layers.is_empty() {
            return Err(AuthoredEffectError::NoLayers);
        }
        for (layer_index, layer) in self.layers.iter().enumerate() {
            if layer.textures.is_empty() {
                return Err(AuthoredEffectError::NoTextures { layer: layer_index });
            }
            if layer.keys.is_empty() {
                return Err(AuthoredEffectError::NoKeys { layer: layer_index });
            }
            let mut previous_at = None;
            for (key_index, key) in layer.keys.iter().enumerate() {
                if previous_at.is_some_and(|previous| key.at <= previous) {
                    return Err(AuthoredEffectError::UnsortedKeys {
                        layer: layer_index,
                        key: key_index,
                    });
                }
                previous_at = Some(key.at);
                if key.texture_index >= layer.textures.len() {
                    return Err(AuthoredEffectError::TextureIndexOutOfRange {
                        layer: layer_index,
                        key: key_index,
                        index: key.texture_index,
                        len: layer.textures.len(),
                    });
                }
                if key.quad.is_some() == key.xy.is_some() {
                    return Err(AuthoredEffectError::QuadXyAmbiguous {
                        layer: layer_index,
                        key: key_index,
                    });
                }
            }
        }
        Ok(())
    }
}
