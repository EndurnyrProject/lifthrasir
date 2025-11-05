use crate::{
    domain::entities::markers::LocalPlayer,
    domain::entities::movement::events::MovementRequested,
    domain::entities::pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
    domain::entities::sprite_rendering::SpriteObjectTree,
    domain::world::components::MapLoader,
    infrastructure::assets::loaders::RoGroundAsset,
    utils::coordinates::world_position_to_spawn_coords,
};
use bevy::prelude::*;

use super::{
    cursor::{CursorChangeRequest, CursorType},
    terrain_raycast::TerrainRaycastCache,
    ForwardedMouseClick,
};

/// Render terrain cursor gizmo at the world position under the mouse cursor
/// Shows where the player is pointing on the terrain with corner markers
pub fn render_terrain_cursor(mut gizmos: Gizmos, cache: Res<TerrainRaycastCache>) {
    if !cache.is_walkable {
        return;
    }

    let Some(world_pos) = cache.world_position else {
        return;
    };

    let Some((cell_x, cell_y)) = cache.cell_coords else {
        return;
    };

    const RO_UNITS_PER_CELL: f32 = 5.0;
    const HALF_RO_CELL: f32 = RO_UNITS_PER_CELL / 2.0;
    let cell_center_x = cell_x as f32 * RO_UNITS_PER_CELL + HALF_RO_CELL;
    let cell_center_z = cell_y as f32 * RO_UNITS_PER_CELL + HALF_RO_CELL;

    const MARKER_SIZE: f32 = 0.4;
    let color = Srgba::hex("00FF00").unwrap().with_alpha(0.4);

    let corners = [
        Vec3::new(
            cell_center_x - HALF_RO_CELL,
            world_pos.y,
            cell_center_z - HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x + HALF_RO_CELL,
            world_pos.y,
            cell_center_z - HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x - HALF_RO_CELL,
            world_pos.y,
            cell_center_z + HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x + HALF_RO_CELL,
            world_pos.y,
            cell_center_z + HALF_RO_CELL,
        ),
    ];

    for corner in corners {
        gizmos.sphere(Isometry3d::from_translation(corner), MARKER_SIZE, color);
    }
}

/// Handle terrain clicks for player movement
/// Reads ForwardedMouseClick and cached raycast data, emits MovementRequested
pub fn handle_terrain_click(
    mut commands: Commands,
    mut mouse_click: ResMut<ForwardedMouseClick>,
    cache: Res<TerrainRaycastCache>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
    player_query: Query<(Entity, &SpriteObjectTree), With<LocalPlayer>>,
    sprite_transforms: Query<&Transform>,
) {
    if mouse_click.position.take().is_none() {
        return;
    }

    let Some((dest_x, dest_y)) = cache.cell_coords else {
        debug!("Click with no valid raycast cache");
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

            commands.trigger(MovementRequested {
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
            commands.trigger(MovementRequested {
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

/// Update cursor based on terrain walkability
///
/// Reads cached raycast data and checks if cell is walkable.
/// Emits CursorChangeRequest to update cursor to "default" for walkable terrain
/// or "impossible" for blocked/unwalkable terrain
pub fn update_cursor_for_terrain(
    cache: Res<TerrainRaycastCache>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    let cursor_type = if cache.is_walkable {
        CursorType::Default
    } else {
        CursorType::Impossible
    };

    cursor_messages.write(CursorChangeRequest::new(cursor_type));
}
