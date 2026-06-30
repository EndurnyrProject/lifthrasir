use bevy::prelude::*;

use crate::infrastructure::assets::IndoorMapTableAsset;

/// Holds the handle to the indoor map table asset (`data\indoorrswtable.txt`).
/// Loaded once at startup; read by `apply_camera_map_profile` to decide whether
/// a map uses the restricted indoor camera.
#[derive(Resource, Debug, Default)]
pub struct IndoorMapTable {
    pub handle: Option<Handle<IndoorMapTableAsset>>,
}

/// Tracks which map profile is currently applied to the camera.
///
/// `map_name` is the normalized name the profile was last applied for; it gates
/// re-application so the profile is only set once per map change. `indoor` lets
/// the R-key reset re-apply the correct preset.
#[derive(Resource, Debug, Default)]
pub struct ActiveCameraProfile {
    pub map_name: String,
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
