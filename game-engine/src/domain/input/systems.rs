use crate::{
    domain::entities::markers::LocalPlayer,
    domain::entities::movement::events::MovementRequested,
    domain::entities::pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
    domain::entities::sprite_rendering::SpriteObjectTree,
    domain::world::components::MapLoader,
    infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset},
    utils::coordinates::world_position_to_spawn_coords,
};
use bevy::math::primitives::InfinitePlane3d;
use bevy::prelude::*;

use super::{ForwardedCursorPosition, ForwardedMouseClick};

/// Raycast from cursor to terrain and return world position with height
/// Returns None if raycast fails or position is outside terrain
fn raycast_terrain_position(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor_pos: Vec2,
    map_loader: &MapLoader,
    altitude_assets: &Assets<RoAltitudeAsset>,
) -> Option<(Vec3, f32)> {
    // Get altitude (GAT) asset
    let altitude_handle = map_loader.altitude.as_ref()?;
    let altitude_asset = altitude_assets.get(altitude_handle)?;

    // Create ray from camera through cursor position
    let ray = camera
        .viewport_to_world(camera_transform, cursor_pos)
        .ok()?;

    // Intersect ray with ground plane (Y = 0)
    let ground_plane = InfinitePlane3d::new(Vec3::Y);
    let distance = ray.intersect_plane(Vec3::ZERO, ground_plane)?;

    // Calculate world position where ray hits ground
    let world_pos = ray.origin + ray.direction * distance;

    // Get GAT terrain height
    let terrain_height = altitude_asset
        .altitude
        .get_terrain_height_at_position(world_pos)?;

    Some((world_pos, terrain_height))
}

/// Render terrain cursor gizmo at the world position under the mouse cursor
/// Shows where the player is pointing on the terrain with visual indicators
pub fn render_terrain_cursor(
    mut gizmos: Gizmos,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    cursor_pos: Res<ForwardedCursorPosition>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Ok(map_loader) = map_loader_query.single() else {
        return;
    };

    let Some(cursor_position) = cursor_pos.position else {
        return;
    };

    // Use shared raycast logic
    let Some((world_pos, gizmo_height)) = raycast_terrain_position(
        camera,
        camera_transform,
        cursor_position,
        map_loader,
        &altitude_assets,
    ) else {
        return;
    };

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

/// Handle terrain clicks for player movement
/// Reads ForwardedMouseClick, raycasts to terrain, converts to RO coords, emits MovementRequested
#[allow(clippy::too_many_arguments)]
pub fn handle_terrain_click(
    mut commands: Commands,
    mut mouse_click: ResMut<ForwardedMouseClick>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
    player_query: Query<(Entity, &SpriteObjectTree), With<LocalPlayer>>,
    sprite_transforms: Query<&Transform>,
    mut movement_events: MessageWriter<MovementRequested>,
) {
    let Some(click_pos) = mouse_click.position.take() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        warn!("No camera found for terrain click");
        return;
    };

    let Ok(map_loader) = map_loader_query.single() else {
        warn!("No map loaded, ignoring terrain click");
        return;
    };

    let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
        warn!("Ground asset not loaded, ignoring terrain click");
        return;
    };

    let Some((world_pos, _terrain_height)) = raycast_terrain_position(
        camera,
        camera_transform,
        click_pos,
        map_loader,
        &altitude_assets,
    ) else {
        debug!("Click raycast missed terrain");
        return;
    };

    let (dest_x, dest_y) = world_position_to_spawn_coords(
        world_pos,
        ground_asset.ground.width,
        ground_asset.ground.height,
    );

    let Ok((player_entity, object_tree)) = player_query.single() else {
        warn!("No player character found for movement request");
        return;
    };

    let Ok(transform) = sprite_transforms.get(object_tree.root) else {
        warn!("Player sprite transform not found");
        return;
    };

    let current_pos = transform.translation;
    let (current_x, current_y) = world_position_to_spawn_coords(
        current_pos,
        ground_asset.ground.width,
        ground_asset.ground.height,
    );

    let Some(grid) = pathfinding_grid else {
        warn!("Pathfinding grid not yet loaded, ignoring terrain click");
        return;
    };

    let path = find_path(&grid.0, (current_x, current_y), (dest_x, dest_y));

    match path {
        Some(waypoints) if waypoints.len() > 1 => {
            debug!("Path found with {} waypoints", waypoints.len());

            commands
                .entity(player_entity)
                .insert(WalkablePath::new(waypoints.clone(), (dest_x, dest_y)));

            movement_events.write(MovementRequested {
                entity: player_entity,
                dest_x,
                dest_y,
                direction: 0,
            });

            debug!(
                "Terrain clicked: current=({}, {}), final destination=({}, {}), path length={}",
                current_x,
                current_y,
                dest_x,
                dest_y,
                waypoints.len()
            );
        }
        Some(_waypoints) => {
            debug!("Direct path (adjacent or same cell)");
            movement_events.write(MovementRequested {
                entity: player_entity,
                dest_x,
                dest_y,
                direction: 0,
            });

            debug!(
                "Terrain clicked: direct movement from ({}, {}) to ({}, {})",
                current_x, current_y, dest_x, dest_y
            );
        }
        None => {
            warn!("No path found to ({}, {})", dest_x, dest_y);
        }
    }
}
