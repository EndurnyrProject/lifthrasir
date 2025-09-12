use crate::ro_formats::{PosKeyframe, RotKeyframe};
use bevy::prelude::*;

/// Animation type for RSM models
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// No animation playing
    None,
    /// Loop animation indefinitely
    Loop,
    /// Play animation once
    Once,
}

impl Default for AnimationType {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelInstanceId(pub Entity);

#[derive(Component, Debug, Clone)]
pub struct AnimatedTransform {
    /// Current translation in world space (following RO->Bevy coordinate convention)
    pub translation: Vec3,
    /// Current rotation as quaternion
    pub rotation: Quat,
    /// Current scale
    pub scale: Vec3,
}

impl Default for AnimatedTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl AnimatedTransform {
    /// Create from Bevy Transform
    pub fn from_transform(transform: &Transform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}

/// Stores RSM animation data for a single node
#[derive(Component, Debug, Clone)]
pub struct RsmNodeAnimation {
    /// Position keyframes from RSM data
    pub position_keyframes: Vec<PosKeyframe>,
    /// Rotation keyframes from RSM data
    pub rotation_keyframes: Vec<RotKeyframe>,
    /// Base transform when animation is at frame 0
    pub base_transform: AnimatedTransform,
    /// Total animation duration in frames
    pub duration_frames: i32,
}

impl RsmNodeAnimation {
    /// Create new node animation data
    pub fn new(
        position_keyframes: Vec<PosKeyframe>,
        rotation_keyframes: Vec<RotKeyframe>,
        base_transform: AnimatedTransform,
        duration_frames: i32,
    ) -> Self {
        Self {
            position_keyframes,
            rotation_keyframes,
            base_transform,
            duration_frames,
        }
    }

    /// Get interpolated position at given animation time (in milliseconds)
    pub fn get_position_at_frame(&self, current_time_ms: f32) -> Vec3 {
        if self.position_keyframes.is_empty() {
            return self.base_transform.translation;
        }

        // Handle time wrapping for looped animations
        let wrapped_time = if self.duration_frames > 0 {
            current_time_ms % self.duration_frames as f32
        } else {
            current_time_ms
        };

        // Convert current animation time to keyframe index
        // RSM keyframes use frame numbers, so we need to map time to frame
        let max_frame = self
            .position_keyframes
            .iter()
            .map(|kf| kf.frame)
            .max()
            .unwrap_or(0) as f32;

        let current_frame = if self.duration_frames > 0 && max_frame > 0.0 {
            // Map animation time to keyframe range
            (wrapped_time / self.duration_frames as f32) * max_frame
        } else {
            wrapped_time
        };

        // Find keyframes to interpolate between
        let mut prev_kf = None;
        let mut next_kf = None;

        for kf in &self.position_keyframes {
            if kf.frame as f32 <= current_frame {
                prev_kf = Some(kf);
            }
            if kf.frame as f32 >= current_frame && next_kf.is_none() {
                next_kf = Some(kf);
                break;
            }
        }

        match (prev_kf, next_kf) {
            (Some(prev), Some(next)) if prev.frame != next.frame => {
                // Interpolate between keyframes
                let t = (current_frame - prev.frame as f32) / (next.frame - prev.frame) as f32;
                let prev_pos = Vec3::new(prev.px, prev.py, prev.pz);
                let next_pos = Vec3::new(next.px, next.py, next.pz);
                prev_pos.lerp(next_pos, t)
            }
            (Some(kf), _) => {
                // Use exact keyframe
                Vec3::new(kf.px, kf.py, kf.pz)
            }
            _ => self.base_transform.translation,
        }
    }

    /// Get interpolated rotation at given animation time (in milliseconds)
    pub fn get_rotation_at_frame(&self, current_time_ms: f32) -> Quat {
        if self.rotation_keyframes.is_empty() {
            return self.base_transform.rotation;
        }

        // Handle time wrapping for looped animations
        let wrapped_time = if self.duration_frames > 0 {
            current_time_ms % self.duration_frames as f32
        } else {
            current_time_ms
        };

        // Convert current animation time to keyframe index
        // RSM keyframes use frame numbers, so we need to map time to frame
        let max_frame = self
            .rotation_keyframes
            .iter()
            .map(|kf| kf.frame)
            .max()
            .unwrap_or(0) as f32;

        let current_frame = if self.duration_frames > 0 && max_frame > 0.0 {
            // Map animation time to keyframe range
            (wrapped_time / self.duration_frames as f32) * max_frame
        } else {
            wrapped_time
        };

        // Find keyframes to interpolate between
        let mut prev_kf = None;
        let mut next_kf = None;

        for kf in &self.rotation_keyframes {
            if kf.frame as f32 <= current_frame {
                prev_kf = Some(kf);
            }
            if kf.frame as f32 >= current_frame && next_kf.is_none() {
                next_kf = Some(kf);
                break;
            }
        }

        match (prev_kf, next_kf) {
            (Some(prev), Some(next)) if prev.frame != next.frame => {
                // Interpolate between keyframes using slerp
                let t = (current_frame - prev.frame as f32) / (next.frame - prev.frame) as f32;
                let prev_quat = Quat::from_xyzw(prev.q[0], prev.q[1], prev.q[2], prev.q[3]);
                let next_quat = Quat::from_xyzw(next.q[0], next.q[1], next.q[2], next.q[3]);
                prev_quat.slerp(next_quat, t)
            }
            (Some(kf), _) => {
                // Use exact keyframe
                Quat::from_xyzw(kf.q[0], kf.q[1], kf.q[2], kf.q[3])
            }
            _ => self.base_transform.rotation,
        }
    }
}

/// Controls RSM animation playback for a model instance
#[derive(Component, Debug, Clone)]
pub struct RsmAnimationController {
    /// Current animation type
    pub anim_type: AnimationType,
    /// Animation speed multiplier (1.0 = normal speed)
    pub speed: f32,
    /// Frame rate for animation timing (frames per second)
    pub frame_rate: f32,
    /// Whether animation is currently playing
    pub is_playing: bool,
    /// Current animation frame
    pub current_frame: f32,
}

impl Default for RsmAnimationController {
    fn default() -> Self {
        Self {
            anim_type: AnimationType::None,
            speed: 1.0,
            frame_rate: 25.0, // Default RO animation frame rate
            is_playing: false,
            current_frame: 0.0,
        }
    }
}

impl RsmAnimationController {
    /// Create a new animation controller
    pub fn new() -> Self {
        Self::default()
    }

    /// Start playing animation
    pub fn play(&mut self, anim_type: AnimationType) {
        self.anim_type = anim_type;
        self.is_playing = true;
        self.current_frame = 0.0;
    }

    /// Update animation frame based on delta time
    /// Uses proper RO animation timing: anim_len is total duration in milliseconds
    pub fn update_frame(&mut self, delta_time: f32, anim_len_ms: i32) {
        if !self.is_playing || self.anim_type == AnimationType::None || anim_len_ms <= 0 {
            return;
        }

        // Convert milliseconds to seconds for proper timing
        let anim_duration_secs = anim_len_ms as f32 / 1000.0;

        // Calculate frame advancement based on animation duration and speed
        // Higher speed = faster playback (e.g., speed 2.0 = twice as fast)
        let frame_delta = (delta_time * self.speed) / anim_duration_secs * anim_len_ms as f32;
        self.current_frame += frame_delta;

        match self.anim_type {
            AnimationType::Loop => {
                if anim_len_ms > 0 {
                    // Loop back to start when exceeding animation length
                    self.current_frame %= anim_len_ms as f32;
                }
            }
            AnimationType::Once => {
                if self.current_frame >= anim_len_ms as f32 {
                    // Stay at last frame but keep playing to avoid sudden stops
                    self.current_frame = anim_len_ms as f32 - 1.0;
                }
            }
            AnimationType::None => {}
        }
    }

    /// Set animation speed
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.0); // Ensure non-negative speed
    }

    /// Set frame rate
    pub fn set_frame_rate(&mut self, frame_rate: f32) {
        self.frame_rate = frame_rate.max(1.0); // Ensure at least 1 FPS
    }
}

