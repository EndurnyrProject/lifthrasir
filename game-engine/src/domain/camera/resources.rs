use bevy::prelude::*;

/// Resource that accumulates camera rotation deltas from mouse input.
///
/// # Purpose
/// - Stores pixel deltas from right-click drag events
/// - Applied by camera_follow_system to update yaw/pitch
/// - Cleared after processing to prevent accumulation
///
/// # Usage
/// Frontend sends deltas via Tauri bridge -> accumulates here -> camera system applies
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
