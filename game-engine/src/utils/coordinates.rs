use crate::infrastructure::ro_formats::RswModel;
use bevy::prelude::*;

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
    let x = (pos.x / RO_UNITS_PER_CELL) as u16;
    let y = (pos.z / RO_UNITS_PER_CELL) as u16;
    (x, y)
}
