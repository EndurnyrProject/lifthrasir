use super::components::{MovementSpeed, MovementState, MovementTarget};
use super::events::{MovementConfirmed, MovementRequested, MovementStopped, StopReason};
use crate::{
    core::state::GameState,
    domain::{
        entities::{
            character::{
                components::{
                    core::Grounded,
                    status::StatusParameter,
                    visual::{CharacterDirection, Direction},
                },
                events::StatusParameterChanged,
                states::AnimationState,
            },
            pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
        },
        system_sets::MovementSystems,
        world::components::MapLoader,
    },
    infrastructure::assets::loaders::RoAltitudeAsset,
    utils::coordinates::{spawn_coords_to_world_position, world_position_to_spawn_coords},
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_behavior::prelude::*;
use net_contract::commands::MoveRequested;
use net_contract::events::{SelfMoved, UnitMoveStopped};

// =============================================================================
// PHASE 0.2: UPDATED TO USE FLAT ENTITY STRUCTURE
// =============================================================================
// Removed SpriteObjectTree dependency - queries entity Transform directly.
// Entity now has Transform component directly (no child hierarchy).
// =============================================================================

#[auto_observer(plugin = crate::app::movement_plugin::MovementDomainPlugin)]
pub fn send_movement_requests_observer(
    trigger: On<MovementRequested>,
    mut moves: MessageWriter<MoveRequested>,
) {
    let event = trigger.event();
    debug!(
        "Requesting movement for {:?} to ({}, {}) dir {}",
        event.entity, event.dest_x, event.dest_y, event.direction
    );

    moves.write(MoveRequested {
        dest_x: event.dest_x,
        dest_y: event.dest_y,
    });
}

/// Local-player-space view of a `SelfMoved`, casting proto u32 coords back to the
/// u16 cell space the interpolation path uses.
struct MovementConfirmedFields {
    src_x: u16,
    src_y: u16,
    dest_x: u16,
    dest_y: u16,
    server_tick: u32,
}

#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::Confirm,
        run_if = in_state(GameState::InGame)
    )
)]
#[allow(clippy::too_many_arguments)]
pub fn handle_movement_confirmed_system(
    mut commands: Commands,
    mut server_events: MessageReader<SelfMoved>,
    entity_registry: Res<crate::domain::entities::registry::EntityRegistry>,
    query: Query<(Option<&MovementTarget>, &Transform, Option<&WalkablePath>)>,
    movement_states: Query<&MovementState>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
) {
    for moved in server_events.read() {
        // SelfMove targets the local player (the proto carries no entity id).
        let event = MovementConfirmedFields {
            src_x: moved.src_x as u16,
            src_y: moved.src_y as u16,
            dest_x: moved.dst_x as u16,
            dest_y: moved.dst_y as u16,
            server_tick: moved.start_time as u32,
        };

        let Some(entity) = entity_registry.local_player_entity() else {
            warn!("Self move received but local player entity not spawned yet");
            continue;
        };

        let Ok((existing_target, transform, walkable_path)) = query.get(entity) else {
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
            let current_pos = transform.translation;
            let (current_x, current_y) =
                crate::utils::coordinates::world_position_to_spawn_coords(current_pos, 0, 0);
            let current_world_pos = Vec3::new(current_pos.x, 0.0, current_pos.z);

            debug!(
                "Movement interrupted: using current position ({}, {}) instead of server source ({}, {})",
                current_x, current_y, event.src_x, event.src_y
            );

            (current_x, current_y, current_world_pos)
        } else {
            let pos = spawn_coords_to_world_position(event.src_x, event.src_y, 0, 0);
            (event.src_x, event.src_y, pos)
        };

        let dest_world_pos = spawn_coords_to_world_position(event.dest_x, event.dest_y, 0, 0);

        let path_to_use = walkable_path
            .filter(|path| {
                let destination_matches = path.final_destination == (event.dest_x, event.dest_y);
                if destination_matches {
                    debug!("Reusing existing path for entity {:?}", entity);
                }
                destination_matches
            })
            .cloned();

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
                        let Ok(mut entity_commands) = commands.get_entity(entity) else {
                            debug!("Entity {:?} despawned before path could be applied", entity);
                            continue;
                        };
                        entity_commands.insert(walkable_path.clone());
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
        let direction = Direction::from_movement_vector(dx, dy);

        let already_walking = matches!(movement_states.get(entity), Ok(MovementState::Moving));

        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            debug!(
                "Entity {:?} despawned before movement components could be applied",
                entity
            );
            continue;
        };

        if already_walking {
            debug!(
                "Entity {:?} already walking - updating target without retriggering animation",
                entity
            );
            entity_commands.insert((target, CharacterDirection { facing: direction }));
        } else {
            debug!("Starting Walking animation for entity {:?}", entity);
            entity_commands.insert((
                target,
                MovementState::Moving,
                CharacterDirection { facing: direction },
            ));

            if let Ok(mut behavior) = behaviors.get_mut(entity) {
                behavior.start(AnimationState::Walking);
            }
        }

        commands.trigger(MovementConfirmed {
            entity,
            src_x: event.src_x,
            src_y: event.src_y,
            dest_x: event.dest_x,
            dest_y: event.dest_y,
            server_tick: event.server_tick,
        });
    }
}

#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::Interpolate,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn interpolate_movement_system(
    mut query: Query<(
        Entity,
        &MovementTarget,
        &MovementSpeed,
        &MovementState,
        &mut Transform,
        &mut CharacterDirection,
    )>,
    mut commands: Commands,
) {
    for (entity, target, speed, state, mut transform, mut character_direction) in query.iter_mut() {
        if *state != MovementState::Moving {
            continue;
        }

        let progress = target.progress(speed.ms_per_cell);

        if progress >= 1.0 {
            transform.translation.x = target.dest_world_pos.x;
            transform.translation.z = target.dest_world_pos.z;

            commands.trigger(MovementStopped {
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

#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::Stop,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn handle_server_stop_system(
    mut server_stop_events: MessageReader<UnitMoveStopped>,
    mut commands: Commands,
    entity_registry: Res<crate::domain::entities::registry::EntityRegistry>,
    mut transforms: Query<&mut Transform>,
) {
    for server_event in server_stop_events.read() {
        let Some(entity) = entity_registry.get_entity(server_event.gid) else {
            warn!("Movement stop for unknown entity GID: {}", server_event.gid);
            continue;
        };

        let stop_x = server_event.x as u16;
        let stop_y = server_event.y as u16;

        debug!(
            "Movement stopped by server for entity {:?} at ({}, {})",
            entity, stop_x, stop_y
        );

        if let Ok(mut transform) = transforms.get_mut(entity) {
            let final_pos = spawn_coords_to_world_position(stop_x, stop_y, 0, 0);
            transform.translation.x = final_pos.x;
            transform.translation.z = final_pos.z;
        }

        commands.trigger(MovementStopped {
            entity,
            x: stop_x,
            y: stop_y,
            reason: StopReason::ServerInterrupted,
        });
    }
}

/// Mirrors the server's walk-speed param (var 0) into the local player's
/// interpolation speed. The server steps its position at `walk_speed` ms per
/// cell; without this the sprite always covers cells at the default 150 and
/// drifts behind (or ahead of) the authoritative walk whenever a status
/// changes the speed (cart weight, Agi buffs, Quagmire, Free Cast).
#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn sync_walk_speed_from_params(
    mut events: MessageReader<StatusParameterChanged>,
    mut commands: Commands,
) {
    for event in events.read() {
        if event.parameter != StatusParameter::Speed {
            continue;
        }

        let Ok(mut entity_commands) = commands.get_entity(event.entity) else {
            continue;
        };

        entity_commands.insert(MovementSpeed::from_server_speed(event.new_value as u16));
    }
}

/// Attacks root you in place. The server only fires an attack once the unit is
/// in range, but its stepper leads the client's interpolation, so the swing
/// packet lands while the in-flight move is still short of the attack cell.
/// When the move is nearly done, fast-forward to its destination so the swing
/// plays where the server attacked from. A farther destination may be a stale
/// ground-walk target (clicking a mob already in range mid-walk), so freeze at
/// the current position instead of teleporting; the server's authoritative
/// stop (`handle_server_stop_system`) corrects the exact cell.
#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::Stop,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn cancel_movement_on_attack(
    mut commands: Commands,
    mut query: Query<(Entity, &AnimationState, &MovementTarget, &mut Transform)>,
) {
    const SNAP_RANGE_CELLS: u16 = 3;

    for (entity, state, target, mut transform) in query.iter_mut() {
        if *state != AnimationState::Attacking {
            continue;
        }

        let (cell_x, cell_y) = world_position_to_spawn_coords(transform.translation, 0, 0);
        let remaining_cells = target
            .dest_x
            .abs_diff(cell_x)
            .max(target.dest_y.abs_diff(cell_y));

        if remaining_cells <= SNAP_RANGE_CELLS {
            transform.translation.x = target.dest_world_pos.x;
            transform.translation.z = target.dest_world_pos.z;
        }

        commands
            .entity(entity)
            .remove::<MovementTarget>()
            .remove::<WalkablePath>()
            .insert(MovementState::Idle);
    }
}

#[auto_observer(plugin = crate::app::movement_plugin::MovementDomainPlugin)]
pub fn handle_movement_stopped_observer(
    trigger: On<MovementStopped>,
    mut commands: Commands,
    movement_states: Query<&MovementState>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
) {
    let event = trigger.event();
    debug!(
        "Cleaning up movement for {:?}: reason {:?}",
        event.entity, event.reason
    );

    if let Ok(movement_state) = movement_states.get(event.entity) {
        if matches!(movement_state, MovementState::Idle) {
            debug!(
                "Skipping Idle transition for {:?}: already Idle",
                event.entity
            );
            return;
        }
    }

    let Ok(mut entity_commands) = commands.get_entity(event.entity) else {
        debug!(
            "Entity {:?} already despawned, skipping movement cleanup",
            event.entity
        );
        return;
    };

    debug!(
        "Transitioning to Idle animation for entity {:?}",
        event.entity
    );
    entity_commands
        .remove::<MovementTarget>()
        .remove::<WalkablePath>()
        .insert(MovementState::Idle);

    // Only walking returns to idle here. A trailing move-stop must not clobber a
    // busy animation (attacking, hit, sitting, dead) that took over the entity.
    if let Ok(mut behavior) = behaviors.get_mut(event.entity) {
        if *behavior.current() == AnimationState::Walking {
            behavior.start(AnimationState::Idle);
        }
    }
}

#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::TerrainAlignment,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn update_entity_altitude_system(
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Option<Res<Assets<RoAltitudeAsset>>>,
    mut grounded_entities: Query<&mut Transform, With<Grounded>>,
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

    for mut transform in grounded_entities.iter_mut() {
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

    fn cancel_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, cancel_movement_on_attack);
        app
    }

    fn moving_entity(app: &mut App, cell: (u16, u16), dest: (u16, u16)) -> Entity {
        let pos = spawn_coords_to_world_position(cell.0, cell.1, 0, 0);
        let target = MovementTarget::new(
            cell.0,
            cell.1,
            dest.0,
            dest.1,
            pos,
            spawn_coords_to_world_position(dest.0, dest.1, 0, 0),
            0,
        );

        app.world_mut()
            .spawn((
                AnimationState::Attacking,
                target,
                Transform::from_translation(pos),
                MovementState::Moving,
            ))
            .id()
    }

    #[test]
    fn attack_near_destination_fast_forwards_to_it() {
        let mut app = cancel_app();
        let entity = moving_entity(&mut app, (10, 10), (12, 10));

        app.update();

        let expected = spawn_coords_to_world_position(12, 10, 0, 0);
        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation.x, expected.x);
        assert_eq!(transform.translation.z, expected.z);
        assert!(app.world().get::<MovementTarget>(entity).is_none());
        assert_eq!(
            *app.world().get::<MovementState>(entity).unwrap(),
            MovementState::Idle
        );
    }

    #[test]
    fn attack_far_from_destination_freezes_in_place() {
        let mut app = cancel_app();
        let entity = moving_entity(&mut app, (10, 10), (20, 10));

        app.update();

        let expected = spawn_coords_to_world_position(10, 10, 0, 0);
        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation.x, expected.x);
        assert_eq!(transform.translation.z, expected.z);
        assert!(app.world().get::<MovementTarget>(entity).is_none());
    }

    #[test]
    fn non_attacking_entity_keeps_its_movement() {
        let mut app = cancel_app();
        let entity = moving_entity(&mut app, (10, 10), (12, 10));
        app.world_mut()
            .entity_mut(entity)
            .insert(AnimationState::Walking);

        app.update();

        assert!(app.world().get::<MovementTarget>(entity).is_some());
    }

    #[test]
    fn test_direction_from_movement() {
        assert_eq!(Direction::from_movement_vector(1.0, 0.0), Direction::East);
        assert_eq!(Direction::from_movement_vector(-1.0, 0.0), Direction::West);
        assert_eq!(Direction::from_movement_vector(0.0, 1.0), Direction::North);
        assert_eq!(Direction::from_movement_vector(0.0, -1.0), Direction::South);
        assert_eq!(
            Direction::from_movement_vector(1.0, -1.0),
            Direction::SouthEast
        );
        assert_eq!(
            Direction::from_movement_vector(-1.0, 1.0),
            Direction::NorthWest
        );
    }
}
