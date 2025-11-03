use crate::infrastructure::ro_formats::RswModel;
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
    /// Convert direction to sprite direction index (0-7)
    pub fn to_sprite_direction(self) -> u8 {
        self as u8
    }

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

    /// Convert an angle (in radians) to the nearest 8-direction
    ///
    /// Maps angles in the range [0, 2π] to discrete 8-directional values.
    /// Angles are measured counter-clockwise from the positive X-axis (East).
    ///
    /// # Arguments
    ///
    /// * `angle` - Angle in radians (any value, will be normalized to 0-2π)
    ///
    /// # Returns
    ///
    /// The closest Direction enum value for the given angle
    ///
    /// # Example
    ///
    /// ```ignore
    /// let dir = Direction::from_angle(0.0); // East
    /// let dir = Direction::from_angle(std::f32::consts::PI); // West
    /// ```
    pub fn from_angle(angle: f32) -> Self {
        // Normalize angle to [0, 2π] range
        let normalized = ((angle % (2.0 * std::f32::consts::PI) + 2.0 * std::f32::consts::PI)
            % (2.0 * std::f32::consts::PI))
            * 180.0
            / std::f32::consts::PI;

        // Map degrees to 8 directions (45° per direction, centered on each cardinal/ordinal)
        // RO coordinate system: 0° = West (negative X in standard math coords)
        match normalized as u32 {
            337..=360 | 0..=22 => Direction::West, // 0° ± 22.5°
            23..=67 => Direction::NorthWest,       // 45° ± 22.5°
            68..=112 => Direction::North,          // 90° ± 22.5°
            113..=157 => Direction::NorthEast,     // 135° ± 22.5°
            158..=202 => Direction::East,          // 180° ± 22.5°
            203..=247 => Direction::SouthEast,     // 225° ± 22.5°
            248..=292 => Direction::South,         // 270° ± 22.5°
            293..=336 => Direction::SouthWest,     // 315° ± 22.5°
            _ => Direction::South,                 // Fallback (shouldn't happen)
        }
    }

    /// Calculate direction from a 2D movement vector
    ///
    /// Takes a movement delta (destination - source) and returns the appropriate
    /// 8-direction facing. Uses atan2 to compute the angle then maps to discrete directions.
    ///
    /// # Arguments
    ///
    /// * `dx` - Delta X (destination X - source X)
    /// * `dz` - Delta Z (destination Z - source Z)
    ///   Note: In Bevy 3D space, Z maps to the RO Y coordinate
    ///
    /// # Returns
    ///
    /// The closest 8-direction enum value for the given movement vector.
    /// Returns `Direction::South` if the movement vector is too small (near-zero).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let dir = Direction::from_movement_vector(1.0, 0.0); // Moving east
    /// let dir = Direction::from_movement_vector(-1.0, 1.0); // Moving northwest
    /// ```
    pub fn from_movement_vector(dx: f32, dz: f32) -> Self {
        // Handle near-zero movement (no meaningful direction)
        if dx.abs() < 0.01 && dz.abs() < 0.01 {
            return Direction::South;
        }

        // Calculate angle using atan2 (returns -PI to PI)
        // atan2(z, x) gives angle from positive X axis
        let angle = dz.atan2(dx);

        // Normalize to [0, 2π] range and use from_angle
        let normalized_angle = if angle < 0.0 {
            angle + 2.0 * std::f32::consts::PI
        } else {
            angle
        };

        Self::from_angle(normalized_angle)
    }
}

pub fn rsw_to_bevy_transform(model: &RswModel, map_width: f32, map_height: f32) -> Transform {
    let position = Vec3::new(
        model.position[0] + (map_width * 5.0), // Add half of terrain width
        model.position[1],
        model.position[2] + (map_height * 5.0), // Add half of terrain height
    );

    // mat4.rotateZ, then mat4.rotateX, then mat4.rotateY
    let quat_z = Quat::from_rotation_z(model.rotation[2].to_radians());
    let quat_x = Quat::from_rotation_x(model.rotation[0].to_radians());
    let quat_y = Quat::from_rotation_y(model.rotation[1].to_radians());

    let rotation = quat_z * quat_x * quat_y;

    Transform {
        translation: position,
        rotation,
        scale: Vec3::from_array(model.scale),
    }
}

pub fn get_map_dimensions_from_ground(
    ground: &crate::infrastructure::ro_formats::RoGround,
) -> (f32, f32) {
    let width = ground.width as f32;
    let height = ground.height as f32;

    (width, height)
}

/// Convert RO spawn coordinates to Bevy world position
/// RO coordinates use 5.0 units per cell, while Bevy uses 10.0 (CELL_SIZE)
/// This is because CELL_SIZE is 2x RO's native scale for rendering
pub fn spawn_coords_to_world_position(x: u16, y: u16, _map_width: u32, _map_height: u32) -> Vec3 {
    // RO uses 5.0 units per cell in its native coordinate system
    // Bevy's CELL_SIZE (10.0) is 2x this for scaled rendering
    const RO_UNITS_PER_CELL: f32 = 5.0;
    let world_x = x as f32 * RO_UNITS_PER_CELL;
    let world_z = y as f32 * RO_UNITS_PER_CELL;

    Vec3::new(world_x, 0.0, world_z)
}

/// Convert Bevy world position to RO spawn coordinates
/// Inverse of spawn_coords_to_world_position, using RO's native 5.0 units per cell
pub fn world_position_to_spawn_coords(pos: Vec3, _map_width: u32, _map_height: u32) -> (u16, u16) {
    const RO_UNITS_PER_CELL: f32 = 5.0;
    let x = (pos.x / RO_UNITS_PER_CELL).round() as u16;
    let y = (pos.z / RO_UNITS_PER_CELL).round() as u16;
    (x, y)
}

/// Encode position and direction into 3 bytes
///
/// Used for entity spawn packets (STANDENTRY, NEWENTRY) to encode x, y coordinates
/// and facing direction in a compact format.
///
/// Encoding format:
/// - Byte 0: X[9:2]
/// - Byte 1: X[1:0] Y[9:4]
/// - Byte 2: Y[3:0] Dir[3:0]
///
/// # Arguments
///
/// * `x` - X coordinate (10-bit value, 0-1023)
/// * `y` - Y coordinate (10-bit value, 0-1023)
/// * `dir` - Direction (4-bit value, 0-15)
///
/// # Returns
///
/// 3-byte array containing the encoded position and direction
pub fn encode_pos_dir(x: u16, y: u16, dir: u8) -> [u8; 3] {
    let byte0 = (x >> 2) as u8;
    let byte1 = (((x << 6) | ((y >> 4) & 0x3F)) & 0xFF) as u8;
    let byte2 = (((y << 4) | ((dir as u16) & 0x0F)) & 0xFF) as u8;

    [byte0, byte1, byte2]
}

/// Decode position and direction from 3 bytes
///
/// Inverse of encode_pos_dir, extracts x, y coordinates and direction.
///
/// # Arguments
///
/// * `data` - 3-byte array containing encoded position and direction
///
/// # Returns
///
/// Tuple of (x, y, dir)
pub fn decode_pos_dir(data: [u8; 3]) -> (u16, u16, u8) {
    let x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
    let y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);
    let dir = data[2] & 0x0F;

    (x, y, dir)
}

/// Encode movement data into 6 bytes
///
/// Used for movement packets (MOVEENTRY) to encode source and destination coordinates.
///
/// Encoding format:
/// - byte0: x0[9:2]
/// - byte1: x0[1:0] y0[9:4]
/// - byte2: y0[3:0] x1[9:6]
/// - byte3: x1[5:0] y1[9:8]
/// - byte4: y1[7:0]
/// - byte5: sx[3:0] sy[3:0] (sub-cell offsets, typically 0)
///
/// # Arguments
///
/// * `src_x` - Source X coordinate (10-bit value, 0-1023)
/// * `src_y` - Source Y coordinate (10-bit value, 0-1023)
/// * `dst_x` - Destination X coordinate (10-bit value, 0-1023)
/// * `dst_y` - Destination Y coordinate (10-bit value, 0-1023)
///
/// # Returns
///
/// 6-byte array containing the encoded movement data
pub fn encode_move_data(src_x: u16, src_y: u16, dst_x: u16, dst_y: u16) -> [u8; 6] {
    let byte0 = (src_x >> 2) as u8;
    let byte1 = (((src_x << 6) | ((src_y >> 4) & 0x3F)) & 0xFF) as u8;
    let byte2 = (((src_y << 4) | ((dst_x >> 6) & 0x0F)) & 0xFF) as u8;
    let byte3 = (((dst_x << 2) | ((dst_y >> 8) & 0x03)) & 0xFF) as u8;
    let byte4 = (dst_y & 0xFF) as u8;
    let byte5 = 0; // Sub-cell offsets (typically 0)

    [byte0, byte1, byte2, byte3, byte4, byte5]
}

/// Decode movement data from 6 bytes
///
/// Inverse of encode_move_data, extracts source and destination coordinates.
///
/// # Arguments
///
/// * `data` - 6-byte array containing encoded movement data
///
/// # Returns
///
/// Tuple of (src_x, src_y, dst_x, dst_y)
pub fn decode_move_data(data: [u8; 6]) -> (u16, u16, u16, u16) {
    let src_x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
    let src_y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);
    let dst_x = (((data[2] as u16) & 0x0F) << 6) | ((data[3] as u16) >> 2);
    let dst_y = (((data[3] as u16) & 0x03) << 8) | (data[4] as u16);

    (src_x, src_y, dst_x, dst_y)
}
