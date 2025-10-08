use crate::{domain::world::components::MapLoader, infrastructure::assets::loaders::RoGroundAsset};
use bevy::math::primitives::InfinitePlane3d;
use bevy::prelude::*;

use super::ForwardedCursorPosition;

/// Render terrain cursor gizmo at the world position under the mouse cursor
/// Shows where the player is pointing on the terrain with visual indicators
pub fn render_terrain_cursor(
    mut gizmos: Gizmos,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    cursor_pos: Res<ForwardedCursorPosition>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Ok(map_loader) = map_loader_query.single() else {
        return; // Map not loaded yet
    };

    let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
        return; // Ground asset not loaded yet
    };

    let Some(cursor_position) = cursor_pos.position else {
        return;
    };

    // Create ray from camera through cursor position
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    // Intersect ray with ground plane (Y = 0) to get approximate world position
    let ground_plane = InfinitePlane3d::new(Vec3::Y);
    let Some(distance) = ray.intersect_plane(Vec3::ZERO, ground_plane) else {
        return;
    };

    // Calculate world position where ray hits ground
    let world_pos = ray.origin + ray.direction * distance;

    // Get the actual terrain height at this position
    let Some(terrain_height) = ground_asset
        .ground
        .get_terrain_height_at_position(world_pos)
    else {
        return; // Position outside terrain bounds
    };

    // Subtract offset to move gizmo UP (since NEG_Y is up in RO coordinate system)
    let gizmo_height = terrain_height - 2.0;

    // Draw crosshair at intersection point
    gizmos.line(
        Vec3::new(world_pos.x - 10.0, gizmo_height, world_pos.z),
        Vec3::new(world_pos.x + 10.0, gizmo_height, world_pos.z),
        Color::srgb(1.0, 0.0, 0.0), // Red
    );
    gizmos.line(
        Vec3::new(world_pos.x, gizmo_height, world_pos.z - 10.0),
        Vec3::new(world_pos.x, gizmo_height, world_pos.z + 10.0),
        Color::srgb(1.0, 0.0, 0.0), // Red
    );

    // Draw circle around intersection
    gizmos.circle(
        Isometry3d::new(
            Vec3::new(world_pos.x, gizmo_height - 0.1, world_pos.z),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        15.0,
        Color::srgb(1.0, 1.0, 0.0), // Yellow
    );

    // Draw sphere above the cursor (NEG_Y is up, so subtract to go up)
    gizmos.sphere(
        Isometry3d::new(
            Vec3::new(world_pos.x, gizmo_height - 5.0, world_pos.z),
            Quat::IDENTITY,
        ),
        5.0,
        Color::srgb(0.0, 1.0, 0.0), // Green
    );
}
