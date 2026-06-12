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

/// Lightweight animation state component for entities.
/// Replaces the complex RoAnimationController + child entity hierarchy.
#[derive(Component, Clone, Debug)]
pub struct RoSprite {
    /// Handle to the pre-processed animation asset
    pub animation: Handle<RoAnimationAsset>,

    /// Base action type (Idle=0, Walk=1, Attack=2, etc.)
    /// Actual action index = base_action * 8 + direction
    pub base_action: u8,

    /// Current direction (0-7, where 0=South, going clockwise)
    pub direction: u8,

    /// Game time (ms) when current action started
    pub start_time: u32,

    /// Speed multiplier for animation playback
    pub speed_factor: f32,

    /// Whether the animation loops
    pub looping: bool,

    /// Whether animation is paused
    pub paused: bool,
}

impl Default for RoSprite {
    fn default() -> Self {
        Self {
            animation: Handle::default(),
            base_action: 0,
            direction: 0,
            start_time: 0,
            speed_factor: 1.0,
            looping: true,
            paused: false,
        }
    }
}

impl RoSprite {
    /// Calculate actual action index from base_action and direction.
    /// In RO, action index = base_action * 8 + direction.
    #[inline]
    pub fn actual_action_index(&self) -> usize {
        (self.base_action as usize) * 8 + (self.direction as usize)
    }

    /// Get the current frame data based on game time.
    pub fn get_frame<'a>(
        &self,
        animation: &'a RoAnimationAsset,
        game_time_ms: u32,
    ) -> Option<&'a FrameData> {
        self.get_frame_internal(animation, game_time_ms, false)
    }

    /// Get static frame 0 for current direction (used for head during idle to prevent dori-dori).
    pub fn get_static_frame<'a>(&self, animation: &'a RoAnimationAsset) -> Option<&'a FrameData> {
        self.get_frame_internal(animation, 0, true)
    }

    fn get_frame_internal<'a>(
        &self,
        animation: &'a RoAnimationAsset,
        game_time_ms: u32,
        force_static: bool,
    ) -> Option<&'a FrameData> {
        let action_index = self.actual_action_index();
        let action_data = animation.actions.get(action_index)?;

        if action_data.frames.is_empty() {
            return None;
        }

        let frame_count = action_data.frames.len();

        let frame_index = if force_static {
            0
        } else {
            let elapsed = if self.paused {
                0
            } else {
                game_time_ms.wrapping_sub(self.start_time)
            };

            let delay = (action_data.delay_ms * self.speed_factor).max(1.0);
            let frame_time = (elapsed as f32 / delay) as usize;

            if self.looping {
                frame_time % frame_count
            } else {
                frame_time.min(frame_count.saturating_sub(1))
            }
        };

        action_data.frames.get(frame_index)
    }

    /// Start a new action, resetting the animation timer
    pub fn set_action(&mut self, base_action: u8, game_time_ms: u32) {
        if self.base_action != base_action {
            self.base_action = base_action;
            self.start_time = game_time_ms;
        }
    }

    /// Change direction without resetting animation
    pub fn set_direction(&mut self, direction: u8) {
        self.direction = direction % 8;
    }
}

impl Default for RoAnimationAsset {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            actions: Vec::new(),
            layer: LAYER_BODY,
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
