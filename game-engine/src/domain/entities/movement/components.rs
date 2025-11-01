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
/// For multi-waypoint paths, it stores all waypoints and calculates
/// smooth interpolation across the entire path without stopping at
/// intermediate waypoints.
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

    /// Optional waypoints for smooth multi-segment interpolation
    /// Each waypoint is cached as world position for performance
    pub waypoints: Option<Vec<Vec3>>,

    /// Total path length in CELL space for duration calculation (timing)
    /// Used with ms_per_cell to calculate total movement time
    pub total_path_length: f32,

    /// Total path length in WORLD space for interpolation (rendering)
    /// Used to calculate position along the visual path
    pub total_path_length_world: f32,

    /// Cumulative distances at each waypoint in WORLD space (for efficient segment lookup)
    pub segment_distances: Vec<f32>,
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
        // Cell space distance (for timing)
        let dx = (dest_x as f32) - (src_x as f32);
        let dy = (dest_y as f32) - (src_y as f32);
        let total_distance = (dx * dx + dy * dy).sqrt();

        // World space distance (for interpolation)
        let world_distance = src_world_pos.distance(dest_world_pos);

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
            waypoints: None,
            total_path_length: total_distance,
            total_path_length_world: world_distance,
            segment_distances: vec![],
        }
    }

    /// Create a movement target with multi-waypoint path support
    ///
    /// Calculates cumulative distances for smooth interpolation across all waypoints.
    /// This allows the character to move continuously without stopping at intermediate points.
    ///
    /// # Important
    /// The `total_path_length` is calculated in **cell space** (not world space) to ensure
    /// correct duration calculation with `ms_per_cell`. World positions are used only for
    /// visual interpolation.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_waypoints(
        src_x: u16,
        src_y: u16,
        dest_x: u16,
        dest_y: u16,
        src_world_pos: Vec3,
        dest_world_pos: Vec3,
        start_tick: u32,
        waypoint_world_positions: Vec<Vec3>,
        waypoint_cell_coords: Vec<(u16, u16)>,
    ) -> Self {
        let dx = (dest_x as f32) - (src_x as f32);
        let dy = (dest_y as f32) - (src_y as f32);
        let total_distance = (dx * dx + dy * dy).sqrt();

        // Calculate cumulative distances in WORLD space for interpolation
        let mut segment_distances = Vec::new();
        let mut cumulative_world_distance = 0.0;
        segment_distances.push(0.0);

        for i in 1..waypoint_world_positions.len() {
            let prev = waypoint_world_positions[i - 1];
            let curr = waypoint_world_positions[i];
            let segment_length = prev.distance(curr);
            cumulative_world_distance += segment_length;
            segment_distances.push(cumulative_world_distance);
        }

        // Calculate total path length in CELL space for duration calculation
        let mut cell_space_distance = 0.0;
        for i in 1..waypoint_cell_coords.len() {
            let (prev_x, prev_y) = waypoint_cell_coords[i - 1];
            let (curr_x, curr_y) = waypoint_cell_coords[i];
            let dx = (curr_x as f32) - (prev_x as f32);
            let dy = (curr_y as f32) - (prev_y as f32);
            cell_space_distance += (dx * dx + dy * dy).sqrt();
        }

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
            waypoints: Some(waypoint_world_positions),
            total_path_length: cell_space_distance,
            total_path_length_world: cumulative_world_distance,
            segment_distances,
        }
    }

    /// Create a new movement target with elapsed time for mid-movement spawning
    ///
    /// This constructor allows spawning entities that are already mid-movement.
    /// The start_time is adjusted backwards by elapsed_ms so that progress()
    /// calculations work correctly for entities entering view while moving.
    ///
    /// # Arguments
    ///
    /// * `elapsed_ms` - Milliseconds since movement started on server
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_elapsed(
        src_x: u16,
        src_y: u16,
        dest_x: u16,
        dest_y: u16,
        src_world_pos: Vec3,
        dest_world_pos: Vec3,
        start_tick: u32,
        elapsed_ms: u32,
    ) -> Self {
        // Cell space distance (for timing)
        let dx = (dest_x as f32) - (src_x as f32);
        let dy = (dest_y as f32) - (src_y as f32);
        let total_distance = (dx * dx + dy * dy).sqrt();

        // World space distance (for interpolation)
        let world_distance = src_world_pos.distance(dest_world_pos);

        let start_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(elapsed_ms as u64))
            .unwrap_or_else(std::time::Instant::now);

        Self {
            src_x,
            src_y,
            dest_x,
            dest_y,
            src_world_pos,
            dest_world_pos,
            start_tick,
            total_distance,
            start_time,
            waypoints: None,
            total_path_length: total_distance,
            total_path_length_world: world_distance,
            segment_distances: vec![],
        }
    }

    /// Create movement target with waypoints AND elapsed time for mid-movement spawning
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_waypoints_and_elapsed(
        src_x: u16,
        src_y: u16,
        dest_x: u16,
        dest_y: u16,
        src_world_pos: Vec3,
        dest_world_pos: Vec3,
        start_tick: u32,
        elapsed_ms: u32,
        waypoint_world_positions: Vec<Vec3>,
        waypoint_cell_coords: Vec<(u16, u16)>,
    ) -> Self {
        let mut target = Self::new_with_waypoints(
            src_x,
            src_y,
            dest_x,
            dest_y,
            src_world_pos,
            dest_world_pos,
            start_tick,
            waypoint_world_positions,
            waypoint_cell_coords,
        );

        target.start_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(elapsed_ms as u64))
            .unwrap_or_else(std::time::Instant::now);

        target
    }

    /// Calculate movement progress (0.0 to 1.0)
    ///
    /// Uses elapsed time and speed to determine how far along the movement we are.
    /// Returns 1.0 when movement is complete.
    /// For multi-waypoint paths, uses total_path_length instead of direct distance.
    pub fn progress(&self, speed_ms_per_cell: f32) -> f32 {
        let elapsed_ms = self.start_time.elapsed().as_millis() as f32;
        let path_length = self.total_path_length;
        let total_duration_ms = path_length * speed_ms_per_cell;

        if total_duration_ms <= 0.0 {
            return 1.0;
        }

        (elapsed_ms / total_duration_ms).min(1.0)
    }

    /// Check if movement is complete
    pub fn is_complete(&self, speed_ms_per_cell: f32) -> bool {
        self.progress(speed_ms_per_cell) >= 1.0
    }

    /// Calculate current position along multi-waypoint path
    ///
    /// Returns the interpolated world position based on progress through all waypoints.
    /// For single-segment paths, falls back to simple linear interpolation.
    pub fn interpolated_position(&self, speed_ms_per_cell: f32) -> Vec3 {
        let progress = self.progress(speed_ms_per_cell);

        let Some(waypoints) = &self.waypoints else {
            return self.src_world_pos.lerp(self.dest_world_pos, progress);
        };

        if waypoints.is_empty() {
            return self.src_world_pos.lerp(self.dest_world_pos, progress);
        }

        // Use world space distance for interpolation (matches segment_distances)
        let target_distance = progress * self.total_path_length_world;

        for i in 1..self.segment_distances.len() {
            let segment_start_dist = self.segment_distances[i - 1];
            let segment_end_dist = self.segment_distances[i];

            if target_distance <= segment_end_dist {
                let segment_length = segment_end_dist - segment_start_dist;
                if segment_length <= 0.0 {
                    return waypoints[i - 1];
                }

                let segment_progress = (target_distance - segment_start_dist) / segment_length;
                return waypoints[i - 1].lerp(waypoints[i], segment_progress);
            }
        }

        waypoints.last().copied().unwrap_or(self.dest_world_pos)
    }

    /// Calculate current movement direction based on path progress
    ///
    /// Returns the direction the character should be facing based on which segment
    /// of the path they're currently traversing. For multi-waypoint paths, this
    /// ensures the character faces the correct direction as they turn at corners.
    ///
    /// # Arguments
    ///
    /// * `speed_ms_per_cell` - Movement speed in milliseconds per cell
    ///
    /// # Returns
    ///
    /// Direction enum value representing the current facing direction
    pub fn current_direction(
        &self,
        speed_ms_per_cell: f32,
    ) -> crate::domain::entities::character::components::visual::Direction {
        use crate::domain::entities::character::components::visual::Direction;

        let progress = self.progress(speed_ms_per_cell);

        let Some(waypoints) = &self.waypoints else {
            let dx = self.dest_world_pos.x - self.src_world_pos.x;
            let dz = self.dest_world_pos.z - self.src_world_pos.z;
            return Direction::from_movement_vector(-dx, dz);
        };

        if waypoints.is_empty() {
            let dx = self.dest_world_pos.x - self.src_world_pos.x;
            let dz = self.dest_world_pos.z - self.src_world_pos.z;
            return Direction::from_movement_vector(-dx, dz);
        }

        let target_distance = progress * self.total_path_length_world;

        for i in 1..self.segment_distances.len() {
            let segment_end_dist = self.segment_distances[i];

            if target_distance <= segment_end_dist {
                let segment_start = waypoints[i - 1];
                let segment_end = waypoints[i];

                let dx = segment_end.x - segment_start.x;
                let dz = segment_end.z - segment_start.z;

                if dx.abs() < 0.01 && dz.abs() < 0.01 {
                    continue;
                }

                return Direction::from_movement_vector(-dx, dz);
            }
        }

        if let Some(last_waypoint) = waypoints.last() {
            let dx = self.dest_world_pos.x - last_waypoint.x;
            let dz = self.dest_world_pos.z - last_waypoint.z;
            return Direction::from_movement_vector(-dx, dz);
        }

        let dx = self.dest_world_pos.x - self.src_world_pos.x;
        let dz = self.dest_world_pos.z - self.src_world_pos.z;
        Direction::from_movement_vector(-dx, dz)
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

    #[test]
    fn test_current_direction_simple_path() {
        use crate::domain::entities::character::components::visual::Direction;

        let target = MovementTarget::new(
            0,
            0,
            10,
            0,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            1000,
        );

        let speed = MovementSpeed::new(100.0);
        let direction = target.current_direction(speed.ms_per_cell);

        assert_eq!(direction, Direction::West);
    }

    #[test]
    fn test_current_direction_l_shaped_path() {
        use crate::domain::entities::character::components::visual::Direction;

        let waypoint_world_positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 50.0),
        ];
        let waypoint_cell_coords = vec![(0, 0), (10, 0), (10, 10)];

        let mut target = MovementTarget::new_with_waypoints(
            0,
            0,
            10,
            10,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 50.0),
            1000,
            waypoint_world_positions,
            waypoint_cell_coords,
        );

        let speed = MovementSpeed::new(100.0);

        target.start_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(500))
            .unwrap_or_else(std::time::Instant::now);

        let direction_at_start = target.current_direction(speed.ms_per_cell);
        assert_eq!(direction_at_start, Direction::West);

        target.start_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(1200))
            .unwrap_or_else(std::time::Instant::now);

        let direction_at_corner = target.current_direction(speed.ms_per_cell);
        assert_eq!(direction_at_corner, Direction::North);
    }
}
