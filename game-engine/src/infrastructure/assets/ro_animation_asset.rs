use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::time::Duration;

/// Pre-rendered animation asset with cached frames
/// All RGBA conversion happens during asset loading
#[derive(Asset, TypePath, Clone, Debug)]
pub struct RoAnimationAsset {
    /// Pre-rendered Image handles for each frame
    pub frames: Vec<Handle<Image>>,

    /// Duration each frame should display
    pub frame_duration: Duration,

    /// Whether animation should loop
    pub loop_animation: bool,

    /// Action index this animation represents
    pub action_index: usize,

    /// Total number of actions available
    pub total_actions: usize,

    /// Frame offsets for proper positioning (from ACT file)
    pub frame_offsets: Vec<(f32, f32)>,
}

impl RoAnimationAsset {
    pub fn new(
        frames: Vec<Handle<Image>>,
        frame_duration: Duration,
        loop_animation: bool,
        frame_offsets: Vec<(f32, f32)>,
    ) -> Self {
        Self {
            frames,
            frame_duration,
            loop_animation,
            action_index: 0,
            total_actions: 1,
            frame_offsets,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}
