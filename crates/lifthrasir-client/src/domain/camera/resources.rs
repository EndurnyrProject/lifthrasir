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
