use bevy::prelude::*;

/// 8-directional facing for characters and entities
///
/// This enum represents the 8 cardinal and ordinal directions used in Ragnarok Online.
/// The values correspond to sprite animation direction indices (0-7).
///
/// # Direction Layout
/// ```text
///       North (4)
///         |
/// NW (3)  |  NE (5)
///    \    |    /
///     \   |   /
/// West (2)--+--East (6)
///     /   |   \
///    /    |    \
/// SW (1)  |  SE (7)
///         |
///     South (0)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    South = 0,
    SouthWest = 1,
    West = 2,
    NorthWest = 3,
    North = 4,
    NorthEast = 5,
    East = 6,
    SouthEast = 7,
}

impl Direction {
    /// Create a Direction from a u8 value (0-7)
    /// Values outside the range default to South
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Direction::South,
            1 => Direction::SouthWest,
            2 => Direction::West,
            3 => Direction::NorthWest,
            4 => Direction::North,
            5 => Direction::NorthEast,
            6 => Direction::East,
            7 => Direction::SouthEast,
            _ => Direction::South,
        }
    }

    /// Convert RO's network direction order into the internal ACT direction order.
    pub fn from_ro_u8(value: u8) -> Self {
        match value {
            0 => Direction::North,
            1 => Direction::NorthWest,
            2 => Direction::West,
            3 => Direction::SouthWest,
            4 => Direction::South,
            5 => Direction::SouthEast,
            6 => Direction::East,
            7 => Direction::NorthEast,
            _ => Direction::South,
        }
    }

    /// Convert a compass angle (in radians) to the nearest 8-direction.
    ///
    /// The angle is measured counter-clockwise from East as seen from above:
    /// 0 = East, π/2 = North, π = West, 3π/2 = South. Any value is accepted
    /// and normalized to [0, 2π).
    pub fn from_angle(angle: f32) -> Self {
        use std::f32::consts::TAU;
        let normalized = (angle % TAU + TAU) % TAU * 180.0 / std::f32::consts::PI;

        // 45° per direction, centered on each cardinal/ordinal.
        match normalized as u32 {
            337..=360 | 0..=22 => Direction::East,
            23..=67 => Direction::NorthEast,
            68..=112 => Direction::North,
            113..=157 => Direction::NorthWest,
            158..=202 => Direction::West,
            203..=247 => Direction::SouthWest,
            248..=292 => Direction::South,
            293..=336 => Direction::SouthEast,
            _ => Direction::South,
        }
    }

    /// Facing for a world-space movement delta (destination - source).
    ///
    /// World axes: +X is East, -Z is North (RO cell +y). Returns
    /// `Direction::South` when the delta is too small to have a direction.
    pub fn from_movement_vector(dx: f32, dz: f32) -> Self {
        if dx.abs() < 0.01 && dz.abs() < 0.01 {
            return Direction::South;
        }
        Self::from_angle((-dz).atan2(dx))
    }
}

/// World units per RO cell. RO's native scale is 5.0 units per cell; `CELL_SIZE`
/// (10.0) is the GND cell, which spans two GAT cells.
pub const RO_UNITS_PER_CELL: f32 = 5.0;

/// Convert RO cell coordinates to a world position on the y = 0 plane.
/// World up is +Y and RO cell +y runs toward -Z, so the world is right-handed
/// with North at -Z, matching glTF and Bevy's forward convention.
pub fn spawn_coords_to_world_position(x: u16, y: u16) -> Vec3 {
    Vec3::new(
        x as f32 * RO_UNITS_PER_CELL,
        0.0,
        -(y as f32) * RO_UNITS_PER_CELL,
    )
}

/// Convert a world position to RO cell coordinates.
/// Inverse of `spawn_coords_to_world_position`.
pub fn world_position_to_spawn_coords(pos: Vec3) -> (u16, u16) {
    let x = (pos.x / RO_UNITS_PER_CELL).round() as u16;
    let y = (-pos.z / RO_UNITS_PER_CELL).round() as u16;
    (x, y)
}
