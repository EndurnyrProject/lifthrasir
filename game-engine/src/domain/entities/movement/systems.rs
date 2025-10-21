use super::components::{MovementSpeed, MovementState, MovementTarget};
use super::events::{MovementConfirmed, MovementRequested, MovementStopped, StopReason};
use crate::domain::entities::character::components::core::Grounded;
use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
use crate::domain::entities::character::sprite_hierarchy::CharacterObjectTree;
use crate::domain::entities::character::states::{StartWalking, StopWalking};
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
/// 2. Creates MovementTarget component with interpolation data
/// 3. Updates character direction based on movement vector
/// 4. Changes state to Moving
/// 5. Inserts StartWalking trigger for animation state machine
///
/// **Position Continuity:** When a new movement is confirmed during an existing movement,
/// this system uses the character's current interpolated position as the source instead of
/// the server's stale position. This prevents the character from snapping back to the old
/// destination before moving to the new target.
pub fn handle_movement_confirmed_system(
    mut commands: Commands,
    mut server_events: MessageReader<MovementConfirmedByServer>,
    mut client_events: MessageWriter<MovementConfirmed>,
    entity_registry: Res<crate::domain::entities::registry::EntityRegistry>,
    query: Query<(Option<&MovementTarget>, &CharacterObjectTree)>,
    sprite_transforms: Query<&Transform>,
) {
    for event in server_events.read() {
        // Look up entity from AID in packet
        let Some(entity) = entity_registry.get_entity(event.aid) else {
            warn!(
                "Movement confirmed for unknown entity AID: {} - may not be spawned yet",
                event.aid
            );
            continue;
        };

        let Ok((existing_target, object_tree)) = query.get(entity) else {
            warn!("Entity {:?} missing required components for movement", entity);
            continue;
        };

        debug!(
            "Movement confirmed for entity {:?}: ({}, {}) -> ({}, {}) at tick {}",
            entity, event.src_x, event.src_y, event.dest_x, event.dest_y, event.server_tick
        );

        // Determine the actual source position for interpolation
        // If the character is already moving, use current position to prevent snapping
        let (actual_src_x, actual_src_y, src_world_pos) = if existing_target.is_some() {
            // Character is mid-movement - use current interpolated position
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
                // Fallback to server position if transform not found
                let pos = spawn_coords_to_world_position(event.src_x, event.src_y, 0, 0);
                (event.src_x, event.src_y, pos)
            }
        } else {
            // Character is idle - use server's source position
            let pos = spawn_coords_to_world_position(event.src_x, event.src_y, 0, 0);
            (event.src_x, event.src_y, pos)
        };

        let dest_world_pos = spawn_coords_to_world_position(event.dest_x, event.dest_y, 0, 0);

        // Create movement target with actual current position as source
        let target = MovementTarget::new(
            actual_src_x,
            actual_src_y,
            event.dest_x,
            event.dest_y,
            src_world_pos,
            dest_world_pos,
            event.server_tick,
        );

        // Calculate direction from actual movement vector (current position to destination)
        let dx = (event.dest_x as f32) - (actual_src_x as f32);
        let dy = (event.dest_y as f32) - (actual_src_y as f32);
        let direction = Direction::from_movement_vector(-dx, dy);

        debug!(
            "🚶 INSERTING StartWalking trigger for entity {:?}",
            entity
        );
        commands
            .entity(entity)
            .remove::<StopWalking>()
            .insert((
                target,
                MovementState::Moving,
                CharacterDirection { facing: direction },
                StartWalking,
            ));

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
/// move characters between their source and destination positions.
pub fn interpolate_movement_system(
    query: Query<(
        Entity,
        &MovementTarget,
        &MovementSpeed,
        &MovementState,
        &CharacterObjectTree,
    )>,
    mut sprite_transforms: Query<&mut Transform>,
    mut stop_events: MessageWriter<MovementStopped>,
) {
    for (entity, target, speed, state, object_tree) in query.iter() {
        if *state != MovementState::Moving {
            continue;
        }

        let Ok(mut transform) = sprite_transforms.get_mut(object_tree.root) else {
            warn!("Sprite root transform not found for entity {:?}", entity);
            continue;
        };

        let progress = target.progress(speed.ms_per_cell);

        if progress >= 1.0 {
            // Movement complete - snap to final position using cached world position
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
            transform.translation.x = target.src_world_pos.x
                + (target.dest_world_pos.x - target.src_world_pos.x) * progress;
            transform.translation.z = target.src_world_pos.z
                + (target.dest_world_pos.z - target.src_world_pos.z) * progress;
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
    query: Query<&CharacterObjectTree>,
    mut sprite_transforms: Query<&mut Transform>,
) {
    for server_event in server_stop_events.read() {
        // Look up entity from AID
        let Some(entity) = entity_registry.get_entity(server_event.aid) else {
            warn!(
                "Movement stop for unknown entity AID: {}",
                server_event.aid
            );
            continue;
        };

        let Ok(object_tree) = query.get(entity) else {
            warn!("Entity {:?} missing CharacterObjectTree", entity);
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
/// - Updates state to Idle
/// - Inserts StopWalking trigger for animation
pub fn handle_movement_stopped_system(
    mut commands: Commands,
    mut events: MessageReader<MovementStopped>,
    movement_states: Query<&MovementState>,
) {
    // Handle all stop events
    for event in events.read() {
        debug!(
            "Cleaning up movement for {:?}: reason {:?}",
            event.entity, event.reason
        );

        // Guard: Only insert StopWalking trigger if not already idle
        // This prevents wasteful Idle→Idle state transitions
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
    grounded_entities: Query<&CharacterObjectTree, With<Grounded>>,
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
