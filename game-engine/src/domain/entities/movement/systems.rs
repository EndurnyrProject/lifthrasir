use super::components::{MovementSpeed, MovementState, MovementTarget};
use super::events::{MovementConfirmed, MovementRequested, MovementStopped, StopReason};
use crate::domain::entities::character::components::core::Grounded;
use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
use crate::domain::entities::character::states::{StartWalking, StopWalking};
use crate::domain::entities::pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath};
use crate::domain::entities::sprite_rendering::components::SpriteObjectTree;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use crate::infrastructure::networking::client::ZoneServerClient;
use crate::infrastructure::networking::protocol::zone::MovementConfirmedByServer;
use crate::infrastructure::networking::protocol::zone::MovementStoppedByServer;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::prelude::*;

/// Send movement requests to the server
///
/// Consumes MovementRequested events and sends CZ_REQUEST_MOVE2 packets.
/// This is the first step in the client-server movement flow.
pub fn send_movement_requests_system(
    mut events: MessageReader<MovementRequested>,
    client: Option<ResMut<ZoneServerClient>>,
) {
    let Some(mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    for event in events.read() {
        debug!(
            "Sending movement request for {:?} to ({}, {}) dir {}",
            event.entity, event.dest_x, event.dest_y, event.direction
        );

        if let Err(e) = client.request_move(event.dest_x, event.dest_y, event.direction) {
            error!("Failed to send movement request: {:?}", e);
        }
    }
}

/// Handle server-confirmed movement
///
/// Processes MovementConfirmedByServer events and initiates client-side interpolation.
/// This system:
/// 1. Looks up entity from AID via EntityRegistry
/// 2. Creates MovementTarget component with multi-waypoint interpolation data
/// 3. Updates character direction based on movement vector
/// 4. Changes state to Moving (only if not already walking)
/// 5. Inserts StartWalking trigger for animation state machine (only if not already walking)
///
/// **Multi-Waypoint Support:** If the entity has a WalkablePath component with waypoints,
/// this system creates a MovementTarget that interpolates smoothly across all waypoints
/// without stopping at intermediate cells. This eliminates the stuttering movement issue.
///
/// **Position Continuity:** When a new movement is confirmed during an existing movement,
/// this system uses the character's current interpolated position as the source instead of
/// the server's stale position. This prevents the character from snapping back to the old
/// destination before moving to the new target.
#[allow(clippy::too_many_arguments)]
pub fn handle_movement_confirmed_system(
    mut commands: Commands,
    mut server_events: MessageReader<MovementConfirmedByServer>,
    mut client_events: MessageWriter<MovementConfirmed>,
    entity_registry: Res<crate::domain::entities::registry::EntityRegistry>,
    query: Query<(
        Option<&MovementTarget>,
        &SpriteObjectTree,
        Option<&WalkablePath>,
    )>,
    sprite_transforms: Query<&Transform>,
    movement_states: Query<&MovementState>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
) {
    for event in server_events.read() {
        let Some(entity) = entity_registry.get_entity(event.aid) else {
            warn!(
                "Movement confirmed for unknown entity AID: {} - may not be spawned yet",
                event.aid
            );
            continue;
        };

        let Ok((existing_target, object_tree, walkable_path)) = query.get(entity) else {
            warn!(
                "Entity {:?} missing required components for movement",
                entity
            );
            continue;
        };

        debug!(
            "Movement confirmed for entity {:?}: ({}, {}) -> ({}, {}) at tick {}",
            entity, event.src_x, event.src_y, event.dest_x, event.dest_y, event.server_tick
        );

        let (actual_src_x, actual_src_y, src_world_pos) = if existing_target.is_some() {
            if let Ok(transform) = sprite_transforms.get(object_tree.root) {
                let current_pos = transform.translation;
                let (current_x, current_y) =
                    crate::utils::coordinates::world_position_to_spawn_coords(current_pos, 0, 0);
                let current_world_pos = Vec3::new(current_pos.x, 0.0, current_pos.z);

                debug!(
                    "🔄 Movement interrupted: using current position ({}, {}) instead of server source ({}, {})",
                    current_x, current_y, event.src_x, event.src_y
                );

                (current_x, current_y, current_world_pos)
            } else {
                let pos = spawn_coords_to_world_position(event.src_x, event.src_y, 0, 0);
                (event.src_x, event.src_y, pos)
            }
        } else {
            let pos = spawn_coords_to_world_position(event.src_x, event.src_y, 0, 0);
            (event.src_x, event.src_y, pos)
        };

        let dest_world_pos = spawn_coords_to_world_position(event.dest_x, event.dest_y, 0, 0);

        // Check if we can reuse existing path
        let path_to_use = walkable_path
            .filter(|path| {
                let destination_matches = path.final_destination == (event.dest_x, event.dest_y);
                if destination_matches {
                    debug!("Reusing existing path for entity {:?}", entity);
                }
                destination_matches
            })
            .cloned();

        // Generate new path only if needed
        let path_to_use = if path_to_use.is_none() {
            if let Some(grid) = pathfinding_grid.as_ref() {
                if let Some(waypoints) = find_path(
                    &grid.0,
                    (actual_src_x, actual_src_y),
                    (event.dest_x, event.dest_y),
                ) {
                    if waypoints.len() > 1 {
                        debug!(
                            "Generated new path for entity {:?} with {} waypoints",
                            entity,
                            waypoints.len()
                        );
                        let walkable_path =
                            WalkablePath::new(waypoints, (event.dest_x, event.dest_y));
                        commands.entity(entity).insert(walkable_path.clone());
                        Some(walkable_path)
                    } else {
                        None
                    }
                } else {
                    warn!(
                        "Could not find path for entity {:?} from ({}, {}) to ({}, {}) - will use direct movement",
                        entity, actual_src_x, actual_src_y, event.dest_x, event.dest_y
                    );
                    None
                }
            } else {
                warn!(
                    "Pathfinding grid not available for entity {:?} - will use direct movement",
                    entity
                );
                None
            }
        } else {
            path_to_use
        };

        let target = if let Some(path) = path_to_use {
            let waypoint_world_positions: Vec<Vec3> = path
                .waypoints
                .iter()
                .map(|(x, y)| spawn_coords_to_world_position(*x, *y, 0, 0))
                .collect();

            // Clone cell coordinates for duration calculation
            let waypoint_cell_coords = path.waypoints.clone();

            debug!(
                "Creating multi-waypoint movement target with {} waypoints",
                waypoint_world_positions.len()
            );

            MovementTarget::new_with_waypoints(
                actual_src_x,
                actual_src_y,
                event.dest_x,
                event.dest_y,
                src_world_pos,
                dest_world_pos,
                event.server_tick,
                waypoint_world_positions,
                waypoint_cell_coords,
            )
        } else {
            MovementTarget::new(
                actual_src_x,
                actual_src_y,
                event.dest_x,
                event.dest_y,
                src_world_pos,
                dest_world_pos,
                event.server_tick,
            )
        };

        let dx = (event.dest_x as f32) - (actual_src_x as f32);
        let dy = (event.dest_y as f32) - (actual_src_y as f32);
        let direction = Direction::from_movement_vector(-dx, dy);

        let already_walking = matches!(movement_states.get(entity), Ok(MovementState::Moving));

        if already_walking {
            debug!(
                "🔄 Entity {:?} already walking - updating target without retriggering animation",
                entity
            );
            commands
                .entity(entity)
                .insert((target, CharacterDirection { facing: direction }));
        } else {
            debug!("🚶 INSERTING StartWalking trigger for entity {:?}", entity);
            commands.entity(entity).remove::<StopWalking>().insert((
                target,
                MovementState::Moving,
                CharacterDirection { facing: direction },
                StartWalking,
            ));
        }

        client_events.write(MovementConfirmed {
            entity,
            src_x: event.src_x,
            src_y: event.src_y,
            dest_x: event.dest_x,
            dest_y: event.dest_y,
            server_tick: event.server_tick,
        });
    }
}

/// Interpolate character movement every frame
///
/// This is the core movement system that runs every frame to smoothly
/// move characters through their entire path. For multi-waypoint paths,
/// it interpolates continuously across all waypoints without stopping
/// at intermediate cells.
///
/// The system also updates the character's facing direction dynamically
/// as they traverse path segments, ensuring proper sprite orientation
/// during turns.
pub fn interpolate_movement_system(
    mut query: Query<(
        Entity,
        &MovementTarget,
        &MovementSpeed,
        &MovementState,
        &SpriteObjectTree,
        &mut CharacterDirection,
    )>,
    mut sprite_transforms: Query<&mut Transform>,
    mut stop_events: MessageWriter<MovementStopped>,
) {
    for (entity, target, speed, state, object_tree, mut character_direction) in query.iter_mut() {
        if *state != MovementState::Moving {
            continue;
        }

        let Ok(mut transform) = sprite_transforms.get_mut(object_tree.root) else {
            warn!("Sprite root transform not found for entity {:?}", entity);
            continue;
        };

        let progress = target.progress(speed.ms_per_cell);

        if progress >= 1.0 {
            transform.translation.x = target.dest_world_pos.x;
            transform.translation.z = target.dest_world_pos.z;

            stop_events.write(MovementStopped {
                entity,
                x: target.dest_x,
                y: target.dest_y,
                reason: StopReason::ReachedDestination,
            });

            debug!(
                "Movement complete for {:?} at ({}, {})",
                entity, target.dest_x, target.dest_y
            );
        } else {
            let interpolated_pos = target.interpolated_position(speed.ms_per_cell);
            transform.translation.x = interpolated_pos.x;
            transform.translation.z = interpolated_pos.z;

            let current_direction = target.current_direction(speed.ms_per_cell);
            if character_direction.facing != current_direction {
                character_direction.facing = current_direction;
            }
        }
    }
}

/// Handle server-initiated movement stops
///
/// Converts MovementStoppedByServer network events into client MovementStopped events.
/// Looks up the entity from AID via EntityRegistry and snaps its position to the
/// server-provided coordinates.
pub fn handle_server_stop_system(
    mut server_stop_events: MessageReader<MovementStoppedByServer>,
    mut client_stop_events: MessageWriter<MovementStopped>,
    entity_registry: Res<crate::domain::entities::registry::EntityRegistry>,
    query: Query<&SpriteObjectTree>,
    mut sprite_transforms: Query<&mut Transform>,
) {
    for server_event in server_stop_events.read() {
        // Look up entity from AID
        let Some(entity) = entity_registry.get_entity(server_event.aid) else {
            warn!("Movement stop for unknown entity AID: {}", server_event.aid);
            continue;
        };

        let Ok(object_tree) = query.get(entity) else {
            warn!("Entity {:?} missing SpriteObjectTree", entity);
            continue;
        };

        debug!(
            "Movement stopped by server for entity {:?} at ({}, {}) tick {}",
            entity, server_event.x, server_event.y, server_event.server_tick
        );

        // Snap to server position
        if let Ok(mut transform) = sprite_transforms.get_mut(object_tree.root) {
            let final_pos = spawn_coords_to_world_position(server_event.x, server_event.y, 0, 0);
            transform.translation.x = final_pos.x;
            transform.translation.z = final_pos.z;
        }

        client_stop_events.write(MovementStopped {
            entity,
            x: server_event.x,
            y: server_event.y,
            reason: StopReason::ServerInterrupted,
        });
    }
}

/// Handle movement stopped events
///
/// Cleanup system that runs when movement completes or is interrupted.
/// - Removes MovementTarget component
/// - Removes WalkablePath component (path is complete)
/// - Updates state to Idle
/// - Inserts StopWalking trigger for animation
///
/// With the new smooth multi-waypoint interpolation, movement only stops
/// when the character reaches the final destination, so this always triggers
/// the complete cleanup sequence.
pub fn handle_movement_stopped_system(
    mut commands: Commands,
    mut events: MessageReader<MovementStopped>,
    movement_states: Query<&MovementState>,
) {
    for event in events.read() {
        debug!(
            "Cleaning up movement for {:?}: reason {:?}",
            event.entity, event.reason
        );

        if let Ok(movement_state) = movement_states.get(event.entity) {
            if matches!(movement_state, MovementState::Idle) {
                debug!(
                    "⏭️ Skipping StopWalking trigger for {:?}: already Idle",
                    event.entity
                );
                continue;
            }
        }

        debug!(
            "🛑 INSERTING StopWalking trigger for entity {:?}",
            event.entity
        );
        commands
            .entity(event.entity)
            .remove::<MovementTarget>()
            .remove::<WalkablePath>()
            .remove::<StartWalking>()
            .insert((MovementState::Idle, StopWalking));
    }
}

/// Update altitude for all grounded entities to follow terrain height
///
/// This system runs every frame to keep grounded entities (like characters) aligned
/// with the terrain height at their current position. It queries the GAT altitude data
/// for the terrain height and updates the entity's Y position accordingly.
///
/// # System Ordering
/// - Must run AFTER interpolate_movement_system (to update after position changes)
/// - Must run AFTER handle_server_stop_system (to update after server corrections)
pub fn update_entity_altitude_system(
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Option<Res<Assets<RoAltitudeAsset>>>,
    grounded_entities: Query<&SpriteObjectTree, With<Grounded>>,
    mut sprite_transforms: Query<&mut Transform>,
) {
    let Some(altitude_assets) = altitude_assets else {
        return;
    };

    let Ok(map_loader) = map_loader_query.single() else {
        return;
    };

    let Some(altitude_handle) = &map_loader.altitude else {
        return;
    };

    let Some(altitude_asset) = altitude_assets.get(altitude_handle) else {
        return;
    };

    for object_tree in grounded_entities.iter() {
        let Ok(mut transform) = sprite_transforms.get_mut(object_tree.root) else {
            continue;
        };

        if let Some(height) = altitude_asset
            .altitude
            .get_terrain_height_at_position(transform.translation)
        {
            transform.translation.y = height;
        }
    }
}

// DEPRECATED: This system is no longer used with smooth multi-waypoint interpolation.
// The new MovementTarget::new_with_waypoints() creates a single movement that
// interpolates smoothly across all waypoints without stopping at intermediate cells.
// Keeping this code for reference but it's not registered in the plugin.
//
// pub fn advance_waypoint_system(
//     mut commands: Commands,
//     mut stop_events: MessageReader<MovementStopped>,
//     mut movement_requests: MessageWriter<MovementRequested>,
//     mut path_query: Query<(Entity, &mut WalkablePath)>,
// ) {
//     for event in stop_events.read() {
//         if event.reason != StopReason::ReachedDestination {
//             continue;
//         }
//
//         let Ok((entity, mut path)) = path_query.get_mut(event.entity) else {
//             continue;
//         };
//
//         if path.advance() {
//             if let Some((next_x, next_y)) = path.next_waypoint() {
//                 debug!(
//                     "Advancing to waypoint {}/{}: ({}, {})",
//                     path.current_waypoint + 1,
//                     path.waypoints.len(),
//                     next_x,
//                     next_y
//                 );
//
//                 movement_requests.write(MovementRequested {
//                     entity,
//                     dest_x: next_x,
//                     dest_y: next_y,
//                     direction: 0,
//                 });
//             }
//         } else {
//             debug!("Path complete, removing WalkablePath component");
//             commands.entity(entity).remove::<WalkablePath>();
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_from_movement() {
        // West (positive X in atan2 = 0° = West in RO coords)
        assert_eq!(Direction::from_movement_vector(1.0, 0.0), Direction::West);
        // East (negative X in atan2 = 180° = East in RO coords)
        assert_eq!(Direction::from_movement_vector(-1.0, 0.0), Direction::East);
        // North (positive Z in atan2 = 90° = North in RO coords)
        assert_eq!(Direction::from_movement_vector(0.0, 1.0), Direction::North);
        // South (negative Z in atan2 = 270° = South in RO coords)
        assert_eq!(Direction::from_movement_vector(0.0, -1.0), Direction::South);
    }
}
