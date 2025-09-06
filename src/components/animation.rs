use bevy::prelude::*;
use crate::assets::{RoSpriteAsset, RoActAsset};
use crate::utils::DEFAULT_ANIMATION_DELAY;

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
        }
    }
    
    pub fn reset(&mut self) {
        self.action_index = 0;
        self.animation_index = 0;
        self.frame_index = 0;
        self.timer = 0.0;
    }
}