use bevy::prelude::*;

/// Movement state component indicating whether the character is moving
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum MovementState {
    /// Character is idle, not moving
    Idle,
    /// Character is currently moving towards a destination
    Moving,
    /// Movement is blocked (collision, stun, etc.)
    Blocked,
}

impl Default for MovementState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Movement target with timing and distance caching
///
/// This component stores movement data for client-side interpolation.
/// The total_distance is cached to avoid expensive sqrt() calculations
/// every frame during interpolation. World positions are also cached
/// to avoid repeated coordinate conversions during interpolation.
#[derive(Component, Debug, Clone)]
pub struct MovementTarget {
    /// Source position in RO coordinates (10-bit, 0-1023)
    pub src_x: u16,
    pub src_y: u16,

    /// Destination position in RO coordinates (10-bit, 0-1023)
    pub dest_x: u16,
    pub dest_y: u16,

    /// Cached source position in world coordinates (avoids per-frame conversion)
    pub src_world_pos: Vec3,

    /// Cached destination position in world coordinates (avoids per-frame conversion)
    pub dest_world_pos: Vec3,

    /// Server tick when movement started
    pub start_tick: u32,

    /// Cached total distance in cells (avoids per-frame sqrt)
    pub total_distance: f32,

    /// Timestamp when movement started (for progress calculation)
    pub start_time: std::time::Instant,
}

impl MovementTarget {
    /// Create a new movement target and cache the distance and world positions
    ///
    /// Accepts pre-calculated world positions to avoid per-frame coordinate conversions.
    pub fn new(
        src_x: u16,
        src_y: u16,
        dest_x: u16,
        dest_y: u16,
        src_world_pos: Vec3,
        dest_world_pos: Vec3,
        start_tick: u32,
    ) -> Self {
        let dx = (dest_x as f32) - (src_x as f32);
        let dy = (dest_y as f32) - (src_y as f32);
        let total_distance = (dx * dx + dy * dy).sqrt();

        Self {
            src_x,
            src_y,
            dest_x,
            dest_y,
            src_world_pos,
            dest_world_pos,
            start_tick,
            total_distance,
            start_time: std::time::Instant::now(),
        }
    }

    /// Calculate movement progress (0.0 to 1.0)
    ///
    /// Uses elapsed time and speed to determine how far along the movement we are.
    /// Returns 1.0 when movement is complete.
    pub fn progress(&self, speed_ms_per_cell: f32) -> f32 {
        let elapsed_ms = self.start_time.elapsed().as_millis() as f32;
        let total_duration_ms = self.total_distance * speed_ms_per_cell;

        if total_duration_ms <= 0.0 {
            return 1.0;
        }

        (elapsed_ms / total_duration_ms).min(1.0)
    }

    /// Check if movement is complete
    pub fn is_complete(&self, speed_ms_per_cell: f32) -> bool {
        self.progress(speed_ms_per_cell) >= 1.0
    }
}

/// Movement speed component
///
/// Defines how fast a character moves. Speed is expressed as milliseconds per cell,
/// which allows for accurate timing synchronization with the server.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct MovementSpeed {
    /// Milliseconds required to traverse one cell
    /// Default: 150ms per cell (standard walk speed)
    pub ms_per_cell: f32,
}

impl MovementSpeed {
    /// Create a new movement speed
    pub fn new(ms_per_cell: f32) -> Self {
        Self { ms_per_cell }
    }

    /// Default walk speed (150ms per cell)
    pub fn default_walk() -> Self {
        Self { ms_per_cell: 150.0 }
    }

    /// Create movement speed from server's speed value
    /// RO server speed: lower = faster
    pub fn from_server_speed(server_speed: u16) -> Self {
        let ms_per_cell = if server_speed > 0 {
            server_speed as f32
        } else {
            150.0
        };

        Self { ms_per_cell }
    }
}

impl Default for MovementSpeed {
    fn default() -> Self {
        Self::default_walk()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_target_distance_calculation() {
        // Straight line movement: (0, 0) -> (3, 4) = distance 5
        let target = MovementTarget::new(0, 0, 3, 4, Vec3::ZERO, Vec3::new(3.0, 0.0, 4.0), 1000);
        assert!((target.total_distance - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_movement_target_progress() {
        let target =
            MovementTarget::new(0, 0, 10, 10, Vec3::ZERO, Vec3::new(10.0, 0.0, 10.0), 1000);
        let speed = MovementSpeed::new(100.0);

        // Immediately after creation, progress should be near 0
        let progress = target.progress(speed.ms_per_cell);
        assert!((0.0..=0.1).contains(&progress));
    }

    #[test]
    fn test_movement_state_default() {
        let state = MovementState::default();
        assert_eq!(state, MovementState::Idle);
    }
}
