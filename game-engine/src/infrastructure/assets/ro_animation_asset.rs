use bevy::prelude::*;
use bevy::reflect::TypePath;
use moonshine_tag::Tag;

use crate::domain::sprite::tags::LAYER_BODY;

/// Pre-processed animation asset with all textures converted at load time.
/// Each RoAnimationAsset represents a single sprite layer (body, head, weapon, etc.).
/// Players composite multiple assets at render time.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct RoAnimationAsset {
    /// Pre-converted GPU textures for all frames
    pub textures: Vec<Handle<Image>>,

    /// Animation data per action+direction combo.
    /// Index = base_action * 8 + direction for 8-directional sprites.
    pub actions: Vec<ActionData>,

    /// Layer type this asset represents (for render ordering)
    pub layer: Tag,

    /// Sound filenames from the ACT file. `FrameData.sound_id` indexes this.
    pub sounds: Vec<String>,
}

/// Data for a single action+direction (e.g., idle facing south, walk facing east)
/// Each ActionData represents ONE direction of ONE action type.
#[derive(Clone, Debug)]
pub struct ActionData {
    /// All frames for this action+direction
    pub frames: Vec<FrameData>,

    /// Base delay in milliseconds between frames
    pub delay_ms: f32,
}

/// Data for a single animation frame
#[derive(Clone, Debug)]
pub struct FrameData {
    /// Sprite parts composited in this frame (sorted by layer order)
    pub parts: Vec<FramePart>,

    /// Pre-computed bounding box size
    pub size: Vec2,

    /// Frame offset from entity origin
    pub offset: Vec2,

    /// Attach point for body/head connection (if applicable)
    pub attach_point: Option<Vec2>,

    /// Sound event to trigger (index into RoAction.sounds)
    pub sound_id: Option<i32>,

    /// Whether this frame triggers attack damage
    pub is_attack_frame: bool,
}

/// A single sprite part within a frame
#[derive(Clone, Debug)]
pub struct FramePart {
    /// Index into RoAnimationAsset.textures
    pub texture_index: usize,

    /// Pre-computed affine transform matrix
    pub transform: Mat4,

    /// Raw layer position from ACT file (sprite_clip.position)
    pub position: Vec2,

    /// Scale factors from ACT layer (x, y)
    pub scale: Vec2,

    /// Texture dimensions in pixels (width, height)
    pub texture_size: Vec2,

    /// Color tint (RGBA, pre-multiplied from ACT layer data)
    pub color: Color,

    /// Whether to flip horizontally
    pub mirror: bool,
}

impl Default for RoAnimationAsset {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            actions: Vec::new(),
            layer: LAYER_BODY,
            sounds: Vec::new(),
        }
    }
}

impl Default for ActionData {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            delay_ms: 150.0,
        }
    }
}

impl Default for FrameData {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            size: Vec2::ZERO,
            offset: Vec2::ZERO,
            attach_point: None,
            sound_id: None,
            is_attack_frame: false,
        }
    }
}
