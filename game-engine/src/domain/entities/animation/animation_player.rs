use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use bevy::prelude::*;

/// Lightweight animation player component
/// No conversion logic - just handle swapping
#[derive(Component, Debug)]
pub struct RoAnimationPlayer {
    /// Handle to the pre-rendered animation asset
    pub animation: Handle<RoAnimationAsset>,

    /// Current frame index
    pub frame_index: usize,

    /// Timer for frame advancement
    pub timer: Timer,

    /// Whether animation is paused
    pub paused: bool,

    /// Whether animation should loop
    pub loop_animation: bool,
}

impl RoAnimationPlayer {
    pub fn new(animation_handle: Handle<RoAnimationAsset>, loop_animation: bool) -> Self {
        Self {
            animation: animation_handle,
            frame_index: 0,
            timer: Timer::from_seconds(0.1, TimerMode::Repeating),
            paused: false,
            loop_animation,
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn reset(&mut self) {
        self.frame_index = 0;
        self.timer.reset();
    }
}
