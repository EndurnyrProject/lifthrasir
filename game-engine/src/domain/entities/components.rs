use crate::infrastructure::assets::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use crate::utils::DEFAULT_ANIMATION_DELAY;
use bevy::prelude::*;

use super::types::ObjectType;

/// Network entity identifier component
///
/// This component is added to ALL entities spawned from network packets.
/// Provides the link between server-side Account ID and client-side Entity ID.
#[derive(Component, Debug, Clone, Copy)]
pub struct NetworkEntity {
    /// Account ID from server (unique identifier in network protocol)
    pub aid: u32,

    /// Game ID from server (may differ from AID in some cases)
    pub gid: u32,

    /// Type of entity (Pc, Npc, Mob, etc.)
    pub object_type: ObjectType,
}

impl NetworkEntity {
    pub fn new(aid: u32, gid: u32, object_type: ObjectType) -> Self {
        Self {
            aid,
            gid,
            object_type,
        }
    }
}

/// Pending despawn component for deferred entity removal
///
/// When an entity receives a VANISH packet while still moving, this component
/// is added to mark it for despawn after movement completes. This prevents
/// entities from disappearing mid-animation when they move out of view range.
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingDespawn {
    /// Type of vanish event (0=out of sight, 1=died, 2=logged out, 3=teleported)
    pub vanish_type: u8,

    /// When the despawn was requested
    pub marked_at: std::time::Instant,
}

impl PendingDespawn {
    pub fn new(vanish_type: u8) -> Self {
        Self {
            vanish_type,
            marked_at: std::time::Instant::now(),
        }
    }

    /// Check if the pending despawn has timed out (5 second safety timeout)
    pub fn has_timed_out(&self) -> bool {
        self.marked_at.elapsed().as_secs() >= 5
    }
}

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
