use crate::infrastructure::assets::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use crate::utils::DEFAULT_ANIMATION_DELAY;
use bevy::prelude::*;

/// Marker component for sprites that should keep their absolute position and not apply ACT offsets
#[derive(Component)]
pub struct KeepAbsolutePosition;

/// Controls animation playback for RO sprites
#[derive(Component)]
pub struct RoAnimationController {
    pub action_index: usize,
    pub animation_index: usize,
    pub frame_index: usize,
    pub timer: f32,
    pub current_delay: f32,
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub palette_handle: Option<Handle<RoPaletteAsset>>,
    pub loop_animation: bool,
    pub paused: bool,
}

impl RoAnimationController {
    pub fn new(sprite_handle: Handle<RoSpriteAsset>, action_handle: Handle<RoActAsset>) -> Self {
        Self {
            action_index: 0,
            animation_index: 0,
            frame_index: 0,
            timer: 0.0,
            current_delay: DEFAULT_ANIMATION_DELAY,
            sprite_handle,
            action_handle,
            palette_handle: None,
            loop_animation: true,
            paused: false,
        }
    }

    pub fn with_palette(mut self, palette_handle: Handle<RoPaletteAsset>) -> Self {
        self.palette_handle = Some(palette_handle);
        self
    }

    pub fn with_action(mut self, action_index: usize) -> Self {
        self.action_index = action_index;
        self
    }

    pub fn looping(mut self, should_loop: bool) -> Self {
        self.loop_animation = should_loop;
        self
    }

    pub fn paused(mut self, is_paused: bool) -> Self {
        self.paused = is_paused;
        self
    }

    pub fn reset(&mut self) {
        self.action_index = 0;
        self.animation_index = 0;
        self.frame_index = 0;
        self.timer = 0.0;
    }
}
