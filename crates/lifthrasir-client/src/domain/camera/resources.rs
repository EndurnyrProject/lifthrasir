use bevy::prelude::*;

/// Per-map camera profile baked into `LIF_map`, published by the map loader
/// when a map's `LifMapData` is ready. Read by `apply_camera_map_profile` to set
/// the indoor/outdoor camera preset and the camera `Exposure`.
///
/// Absent until the first map loads; replaced on every map change.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CurrentMapCameraProfile {
    pub indoor: bool,
    pub exposure_ev100: f32,
}

/// Tracks which map profile is currently applied to the camera.
///
/// `indoor` lets the R-key reset re-apply the correct preset without
/// re-reading the map data.
#[derive(Resource, Debug, Default)]
pub struct ActiveCameraProfile {
    pub indoor: bool,
}

/// Resource that accumulates camera rotation deltas from mouse input.
///
/// # Purpose
/// - Stores pixel deltas from right-click drag events
/// - Applied by camera_follow_system to update yaw/pitch
/// - Cleared after processing to prevent accumulation
///
/// # Usage
/// Input events send deltas -> accumulates here -> camera system applies
#[derive(Resource, Debug, Default)]
pub struct CameraRotationDelta {
    /// Horizontal mouse delta (positive = rotate right)
    pub delta_x: f32,
    /// Vertical mouse delta (positive = rotate down)
    pub delta_y: f32,
}

impl CameraRotationDelta {
    /// Clears accumulated deltas after processing
    pub fn clear(&mut self) {
        self.delta_x = 0.0;
        self.delta_y = 0.0;
    }

    /// Checks if there are any deltas to process
    pub fn has_delta(&self) -> bool {
        self.delta_x.abs() > 0.001 || self.delta_y.abs() > 0.001
    }
}

/// Request a camera shake. Written by any effect descriptor that carries a
/// `shake`; the strongest running request wins.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct CameraShakeRequest {
    /// Peak positional amplitude in world units.
    pub amplitude: f32,
    pub duration_secs: f32,
}

/// The shake currently playing on the follow camera. `applied` is the offset
/// already added to the camera transform, so the next frame can remove it
/// instead of feeding a shaken position back into the follow smoothing.
#[derive(Resource, Debug, Default)]
pub struct ActiveCameraShake {
    pub amplitude: f32,
    pub duration: f32,
    pub remaining: f32,
    pub applied: Vec3,
}

impl ActiveCameraShake {
    /// Replace the running shake when the request is stronger or nothing is
    /// playing, so a weaker simultaneous hit cannot cut a stronger one short.
    pub fn request(&mut self, request: CameraShakeRequest) {
        if request.amplitude <= 0.0 || request.duration_secs <= 0.0 {
            return;
        }
        if self.remaining > 0.0 && request.amplitude <= self.amplitude {
            return;
        }
        self.amplitude = request.amplitude;
        self.duration = request.duration_secs;
        self.remaining = request.duration_secs;
    }

    /// Whether the next frame must still touch the camera transform.
    pub fn is_active(&self) -> bool {
        self.remaining > 0.0 || self.applied != Vec3::ZERO
    }

    /// Advance the envelope and return the offset to apply this frame.
    pub fn advance(&mut self, delta_secs: f32, elapsed_secs: f32) -> Vec3 {
        self.remaining = (self.remaining - delta_secs).max(0.0);
        let falloff = if self.duration > 0.0 {
            self.remaining / self.duration
        } else {
            0.0
        };
        shake_offset(elapsed_secs, self.amplitude * falloff * falloff)
    }
}

/// Oscillating offset for `amplitude` at `elapsed_secs`. Deterministic sines
/// instead of a RNG: the shake needs a vibration, not entropy.
fn shake_offset(elapsed_secs: f32, amplitude: f32) -> Vec3 {
    if amplitude <= 0.0 {
        return Vec3::ZERO;
    }
    let phase = elapsed_secs * SHAKE_RADIANS_PER_SEC;
    Vec3::new(
        phase.sin(),
        (phase * 1.7).sin() * SHAKE_VERTICAL_RATIO,
        (phase * 0.8).cos(),
    ) * amplitude
}

/// Angular frequency of the shake vibration (~18 Hz).
const SHAKE_RADIANS_PER_SEC: f32 = 113.0;
/// Vertical shake relative to the horizontal one; the classic quake is mostly lateral.
const SHAKE_VERTICAL_RATIO: f32 = 0.35;
