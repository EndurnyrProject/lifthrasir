use crate::infrastructure::ro_formats::RswModel;
use crate::utils::constants::CELL_SIZE;
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

/// Convert RO spawn coordinates (cell coords) to Bevy world position
/// RO coordinates are in cells, this converts to world space centered like terrain
pub fn spawn_coords_to_world_position(x: u16, y: u16, map_width: u32, map_height: u32) -> Vec3 {
    // Convert cell coords to world space, centering the map
    let world_x = (x as f32 * CELL_SIZE) - (map_width as f32 * CELL_SIZE / 2.0);
    let world_z = -((y as f32 * CELL_SIZE) - (map_height as f32 * CELL_SIZE / 2.0));

    Vec3::new(world_x, 0.0, world_z)
}

/// Convert Bevy world position to RO spawn coordinates (cell coords)
pub fn world_position_to_spawn_coords(pos: Vec3, map_width: u32, map_height: u32) -> (u16, u16) {
    let x = ((pos.x + (map_width as f32 * CELL_SIZE / 2.0)) / CELL_SIZE) as u16;
    let y = ((-(pos.z) + (map_height as f32 * CELL_SIZE / 2.0)) / CELL_SIZE) as u16;
    (x, y)
}
