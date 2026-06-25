use crate::infrastructure::effect::LoadedEffectAsset;
use bevy::prelude::*;

/// A playing STR effect instance. One per cast; owns the per-frame timer, the
/// loaded effect handle, and the tint that multiplies the STR's own per-frame
/// colour. Layer children carry `EffectLayer`.
#[derive(Component, Debug, Clone)]
pub struct ActiveEffect {
    pub effect: Handle<LoadedEffectAsset>,
    pub timer: EffectFrameTimer,
    pub repeating: bool,
    pub tint: Color,
    /// Set once the per-layer child entities have been spawned. Layer creation
    /// is deferred until the `LoadedEffectAsset` is available in `Assets`, so an
    /// effect triggered before its asset finishes loading still renders.
    pub layers_initialized: bool,
    /// True when a non-repeating effect has run past `max_key`; the despawn
    /// system tears it (and its layer children) down.
    pub finished: bool,
}

/// Where an effect is anchored in the world. `Entity` anchors follow a unit
/// (tracking moving targets); `Position` anchors stay fixed (ground cells).
#[derive(Component, Debug, Clone, Copy)]
pub enum EffectAnchor {
    Entity(Entity),
    Position(Vec3),
}

/// Marks a layer child of an effect instance with its index into the
/// `LoadedEffectAsset.layers` slice.
#[derive(Component, Debug, Clone, Copy)]
pub struct EffectLayer {
    pub layer_index: usize,
    /// True for additive (glow) layers. Solid (alpha-blended / multiply) layers
    /// are depth-biased in front of additive ones so a figure's face is not
    /// washed out by overlapping glows (additive brightens regardless of order).
    pub additive: bool,
}

/// Despawn timer for repeating (ground) effects: aesir sends no removal packet,
/// so persistent effects expire on this timer instead.
#[derive(Component, Debug)]
pub struct EffectLifetime(pub Timer);

/// Per-frame clock for an STR effect. ticks
/// elapsed time, derives `current_frame` from `fps`, and wraps at `max_key`.
#[derive(Debug, Clone)]
pub struct EffectFrameTimer {
    pub elapsed: f32,
    pub fps: u32,
    pub max_key: u32,
    pub current_frame: usize,
}

impl EffectFrameTimer {
    pub fn new(fps: u32, max_key: u32) -> Self {
        Self {
            elapsed: 0.0,
            fps,
            max_key,
            current_frame: 0,
        }
    }

    /// Advance by `delta` seconds and recompute `current_frame`. Returns `false`
    /// once the effect has passed `max_key` (one loop done): the caller wraps
    /// repeating effects and finishes one-shots. Mirrors `FrameTimer::update`.
    pub fn update(&mut self, delta: f32) -> bool {
        self.elapsed += delta;

        if self.fps == 0 {
            self.current_frame = 0;
            return self.max_key == 0;
        }

        let seconds_per_frame = 1.0 / self.fps as f32;
        self.current_frame = (self.elapsed / seconds_per_frame) as usize;

        if self.current_frame >= self.max_key as usize {
            self.elapsed = 0.0;
            self.current_frame = 0;
            return false;
        }

        true
    }
}
