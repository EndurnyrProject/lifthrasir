use crate::{
    domain::entities::{
        character::{
            components::{
                core::{CharacterAppearance, CharacterData, CharacterStats, Gender, Grounded},
                equipment::EquipmentSet,
                visual::{CharacterDirection, CharacterSprite, Direction},
            },
            states::{AnimationState, ContextState, GameplayState, StartWalking},
        },
        components::{NetworkEntity, PendingDespawn},
        markers::*,
        movement::components::{MovementSpeed, MovementState, MovementTarget},
        pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
        registry::EntityRegistry,
        spawning::events::{
            DespawnEntity, EntityVanishRequested, RequestEntityVanish, SpawnEntity,
        },
        sprite_rendering::{
            components::{EntitySpriteData, EntitySpriteInfo, EntitySpriteNames},
            events::SpawnSpriteEvent,
        },
    },
    infrastructure::networking::{protocol::zone::MovementConfirmedByServer, session::UserSession},
    utils::coordinates::spawn_coords_to_world_position,
};
use bevy::ecs::query::{SpawnDetails, Spawned};
use bevy::prelude::*;

/// Spawn network entities from SpawnEntity events
#[allow(clippy::too_many_arguments)]
pub fn spawn_network_entity_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<SpawnEntity>,
    mut entity_registry: ResMut<EntityRegistry>,
    user_session: Option<Res<UserSession>>,
    mut sprite_spawn_generic: MessageWriter<SpawnSpriteEvent>,
    entity_sprite_names: Res<EntitySpriteNames>,
    mut movement_events: MessageWriter<MovementConfirmedByServer>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
) {
    for event in spawn_events.read() {
        if let Some(existing_entity) = entity_registry.get_entity(event.aid) {
            commands.entity(existing_entity).remove::<PendingDespawn>();

            if let Some(destination) = event.destination {
                debug!(
                    "Entity AID {} re-entered view, canceling pending despawn and updating movement: ({}, {}) -> ({}, {})",
                    event.aid, event.position.0, event.position.1, destination.0, destination.1
                );

                movement_events.write(MovementConfirmedByServer {
                    aid: event.aid,
                    src_x: event.position.0,
                    src_y: event.position.1,
                    dest_x: destination.0,
                    dest_y: destination.1,
                    server_tick: event.move_start_time.unwrap_or(0),
                });
            } else {
                debug!(
                    "Entity AID {} re-entered view, canceling pending despawn",
                    event.aid
                );
            }
            continue;
        }

        let char_data = CharacterData {
            name: event.name.clone(),
            job_id: event.job,
            level: event.level as u32,
            experience: 0,
            stats: CharacterStats {
                str: 1,
                agi: 1,
                vit: 1,
                int: 1,
                dex: 1,
                luk: 1,
                max_hp: event.max_hp,
                current_hp: event.hp,
                max_sp: 0,
                current_sp: 0,
            },
            slot: 0,
        };

        let appearance = CharacterAppearance {
            gender: Gender::from(event.gender),
            hair_style: event.head,
            hair_color: event.head_palette,
            clothes_color: event.body_palette,
        };

        let equipment = EquipmentSet {
            head_top: if event.head_top > 0 {
                Some(create_equipment_item(event.head_top as u32))
            } else {
                None
            },
            head_mid: if event.head_mid > 0 {
                Some(create_equipment_item(event.head_mid as u32))
            } else {
                None
            },
            head_bottom: if event.head_bottom > 0 {
                Some(create_equipment_item(event.head_bottom as u32))
            } else {
                None
            },
            weapon: if event.weapon > 0 {
                Some(create_equipment_item(event.weapon))
            } else {
                None
            },
            shield: if event.shield > 0 {
                Some(create_equipment_item(event.shield))
            } else {
                None
            },
            armor: None,
            garment: if event.robe > 0 {
                Some(create_equipment_item(event.robe as u32))
            } else {
                None
            },
            shoes: None,
            accessories: [None, None],
        };

        let world_pos = spawn_coords_to_world_position(event.position.0, event.position.1, 0, 0);

        let is_local_player = user_session
            .as_ref()
            .map(|session| event.aid == session.tokens.account_id)
            .unwrap_or(false);

        let mut entity_cmd = commands.spawn((
            NetworkEntity::new(event.aid, event.gid, event.object_type),
            Transform::from_translation(world_pos),
            Visibility::default(),
            Name::new(format!(
                "{} ({:?}:{})",
                event.name, event.object_type, event.aid
            )),
            CharacterDirection {
                facing: Direction::from_u8(event.direction),
            },
        ));

        // Add character-specific components only for PCs
        match event.object_type {
            crate::domain::entities::types::ObjectType::Pc => {
                entity_cmd.insert((char_data, appearance, equipment, CharacterSprite::default()));
            }
            crate::domain::entities::types::ObjectType::Mob
            | crate::domain::entities::types::ObjectType::Npc
            | crate::domain::entities::types::ObjectType::Homunculus
            | crate::domain::entities::types::ObjectType::Mercenary
            | crate::domain::entities::types::ObjectType::Elemental => {
                // Simple entities don't need character-specific components
                // They will get EntitySpriteInfo added via sprite spawn event
            }
        }

        match event.object_type {
            crate::domain::entities::types::ObjectType::Pc => {
                if is_local_player {
                    entity_cmd.insert(LocalPlayer);
                    debug!("Spawned LOCAL PLAYER: {} (AID: {})", event.name, event.aid);
                } else {
                    entity_cmd.insert(RemotePlayer);
                    debug!("Spawned remote player: {} (AID: {})", event.name, event.aid);
                }
            }
            crate::domain::entities::types::ObjectType::Npc => {
                entity_cmd.insert(Npc);
                debug!("Spawned NPC: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Mob => {
                entity_cmd.insert(Mob);
                debug!("Spawned mob: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Homunculus => {
                entity_cmd.insert(Homunculus);
                debug!("Spawned homunculus: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Mercenary => {
                entity_cmd.insert(Mercenary);
                debug!("Spawned mercenary: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Elemental => {
                entity_cmd.insert(Elemental);
                debug!("Spawned elemental: {} (AID: {})", event.name, event.aid);
            }
        }

        entity_cmd.insert((
            crate::domain::entities::character::states::create_animation_state_machine(),
            AnimationState::Idle,
            GameplayState::Normal,
            ContextState::InGame,
        ));

        if let Some(destination) = event.destination {
            let dest_world_pos = spawn_coords_to_world_position(destination.0, destination.1, 0, 0);

            // Calculate elapsed time since movement started
            let elapsed_ms = event
                .current_server_tick
                .saturating_sub(event.move_start_time.unwrap_or(0));

            // Calculate movement progress to determine spawn position
            let speed_ms_per_cell = event.speed as f32;
            let dx = (destination.0 as f32) - (event.position.0 as f32);
            let dy = (destination.1 as f32) - (event.position.1 as f32);
            let total_distance = (dx * dx + dy * dy).sqrt();

            let progress = if total_distance > 0.0 && speed_ms_per_cell > 0.0 {
                (elapsed_ms as f32 / (total_distance * speed_ms_per_cell)).min(1.0)
            } else {
                0.0
            };

            // Interpolate spawn position - entity appears mid-movement
            let spawn_x = event.position.0 as f32 + dx * progress;
            let spawn_y = event.position.1 as f32 + dy * progress;
            let interpolated_world_pos =
                spawn_coords_to_world_position(spawn_x as u16, spawn_y as u16, 0, 0);

            debug!(
                "Entity {} spawning mid-movement: progress={:.2}% ({:.1}, {:.1}) -> ({}, {}), elapsed={}ms",
                event.name,
                progress * 100.0,
                spawn_x,
                spawn_y,
                destination.0,
                destination.1,
                elapsed_ms
            );

            // Generate pathfinding waypoints for smooth movement
            let waypoints = if let Some(grid) = pathfinding_grid.as_ref() {
                if let Some(waypoints) = find_path(
                    &grid.0,
                    (event.position.0, event.position.1),
                    (destination.0, destination.1),
                ) {
                    if waypoints.len() > 1 {
                        debug!(
                            "Generated path for spawning entity {} with {} waypoints",
                            event.name,
                            waypoints.len()
                        );
                        entity_cmd.insert(WalkablePath::new(
                            waypoints.clone(),
                            (destination.0, destination.1),
                        ));
                        Some(waypoints)
                    } else {
                        None
                    }
                } else {
                    warn!(
                        "Could not find path for entity {} from ({}, {}) to ({}, {}) - will use direct movement",
                        event.name, event.position.0, event.position.1, destination.0, destination.1
                    );
                    None
                }
            } else {
                warn!(
                    "Pathfinding grid not available for entity {} spawn - will use direct movement",
                    event.name
                );
                None
            };

            let target = if let Some(waypoints) = waypoints {
                let waypoint_world_positions: Vec<Vec3> = waypoints
                    .iter()
                    .map(|(x, y)| spawn_coords_to_world_position(*x, *y, 0, 0))
                    .collect();

                MovementTarget::new_with_waypoints_and_elapsed(
                    event.position.0,
                    event.position.1,
                    destination.0,
                    destination.1,
                    world_pos,
                    dest_world_pos,
                    event.move_start_time.unwrap_or(0),
                    elapsed_ms,
                    waypoint_world_positions,
                    waypoints,
                )
            } else {
                MovementTarget::new_with_elapsed(
                    event.position.0,
                    event.position.1,
                    destination.0,
                    destination.1,
                    world_pos,
                    dest_world_pos,
                    event.move_start_time.unwrap_or(0),
                    elapsed_ms,
                )
            };

            // Update entity transform to spawn at interpolated position
            entity_cmd.insert(Transform::from_translation(interpolated_world_pos));

            entity_cmd.insert((
                target,
                MovementState::Moving,
                MovementSpeed::from_server_speed(event.speed),
                Grounded,
                StartWalking,
            ));

            debug!(
                "Entity {} spawned MOVING at ({:.1}, {:.1}) heading to ({}, {})",
                event.name, spawn_x, spawn_y, destination.0, destination.1
            );
        } else {
            entity_cmd.insert((MovementState::Idle, MovementSpeed::default_walk(), Grounded));

            debug!(
                "Entity {} spawned IDLE at ({}, {})",
                event.name, event.position.0, event.position.1
            );
        }

        let entity_id = entity_cmd.id();

        entity_registry.register_entity(event.aid, entity_id);

        // Route sprite spawning based on entity type
        match event.object_type {
            crate::domain::entities::types::ObjectType::Pc => {
                // Phase 3: Use new generic system for PCs
                let sprite_data = EntitySpriteData::Character {
                    job_class: event.job,
                    gender: Gender::from(event.gender),
                    head: event.head,
                };

                sprite_spawn_generic.write(SpawnSpriteEvent {
                    entity: entity_id,
                    position: world_pos,
                    sprite_info: EntitySpriteInfo { sprite_data },
                });
            }
            crate::domain::entities::types::ObjectType::Mob
            | crate::domain::entities::types::ObjectType::Npc
            | crate::domain::entities::types::ObjectType::Homunculus
            | crate::domain::entities::types::ObjectType::Mercenary
            | crate::domain::entities::types::ObjectType::Elemental => {
                // Use EntitySpriteNames to lookup sprite name
                let sprite_name = entity_sprite_names
                    .monsters
                    .get(&event.job)
                    .or_else(|| entity_sprite_names.npcs.get(&event.job))
                    .cloned()
                    .unwrap_or_else(|| {
                        warn!(
                            "Unknown entity job ID: {} for {:?}, using fallback",
                            event.job, event.object_type
                        );
                        format!("entity_{}", event.job)
                    });

                let sprite_data = match event.object_type {
                    crate::domain::entities::types::ObjectType::Mob
                    | crate::domain::entities::types::ObjectType::Homunculus
                    | crate::domain::entities::types::ObjectType::Mercenary
                    | crate::domain::entities::types::ObjectType::Elemental => {
                        EntitySpriteData::Mob { sprite_name }
                    }
                    crate::domain::entities::types::ObjectType::Npc => {
                        EntitySpriteData::Npc { sprite_name }
                    }
                    _ => unreachable!(),
                };

                sprite_spawn_generic.write(SpawnSpriteEvent {
                    entity: entity_id,
                    position: world_pos,
                    sprite_info: EntitySpriteInfo { sprite_data },
                });
            }
        }

        info!(
            "✅ Spawned entity: {} ({:?}) at ({}, {}) - Entity ID: {:?}",
            event.name, event.object_type, event.position.0, event.position.1, entity_id
        );
    }
}

/// Helper to create equipment item from ID
fn create_equipment_item(
    item_id: u32,
) -> crate::domain::entities::character::components::equipment::EquipmentItem {
    use crate::domain::entities::character::components::equipment::EquipmentItem;

    EquipmentItem {
        item_id,
        sprite_id: item_id as u16,
        refinement: 0,
        enchantments: vec![],
        options: vec![],
    }
}

/// Observer for entity despawn events
///
/// Handles DespawnEntity observer events and despawns the entity and its sprite hierarchy.
/// This observer is triggered when an entity needs to be removed from the game world.
pub fn on_despawn_entity(
    trigger: On<DespawnEntity>,
    mut commands: Commands,
    mut entity_registry: ResMut<EntityRegistry>,
    sprite_trees: Query<&crate::domain::entities::sprite_rendering::components::SpriteObjectTree>,
) {
    let event = trigger.event();
    let entity = trigger.entity;

    // Despawn sprite hierarchy first to prevent race condition
    // where update_sprite_transforms tries to access a despawned entity
    if let Ok(object_tree) = sprite_trees.get(entity) {
        commands.entity(object_tree.root).despawn();
    }

    commands.entity(entity).despawn();

    entity_registry.unregister_entity_by_aid(event.aid);

    info!("✅ Despawned entity {:?} with AID: {}", entity, event.aid);
}

/// Cleanup system: Remove entities from registry when they despawn
pub fn cleanup_despawned_entities_system(
    mut entity_registry: ResMut<EntityRegistry>,
    mut removed: RemovedComponents<NetworkEntity>,
) {
    for entity in removed.read() {
        if let Some(aid) = entity_registry.get_account_id(entity) {
            entity_registry.unregister_entity(entity);
            debug!("Cleaned up despawned entity from registry: AID {}", aid);
        }
    }
}

/// Bridge system: Convert network RequestEntityVanish to EntityVanishRequested observer
///
/// This system reads RequestEntityVanish messages from the network layer,
/// looks up the corresponding entity from EntityRegistry, and triggers
/// EntityVanishRequested observer events.
pub fn bridge_vanish_requests_system(
    mut commands: Commands,
    mut vanish_requests: MessageReader<RequestEntityVanish>,
    entity_registry: Res<EntityRegistry>,
) {
    for request in vanish_requests.read() {
        let Some(entity) = entity_registry.get_entity(request.aid) else {
            warn!(
                "Vanish request for unknown entity AID: {} (may already be despawned)",
                request.aid
            );
            continue;
        };

        commands.trigger(EntityVanishRequested {
            entity,
            aid: request.aid,
            vanish_type: request.vanish_type,
        });
    }
}

/// Observer for entity vanish requests
///
/// When an entity vanishes (moves out of range, dies, logs out, or teleports),
/// this observer checks if the entity is currently moving. If so, it marks the
/// entity with PendingDespawn component to defer actual despawn until movement
/// completes. This prevents entities from disappearing mid-movement.
pub fn on_entity_vanish_request(
    trigger: On<EntityVanishRequested>,
    mut commands: Commands,
    movement_query: Query<&MovementState>,
) {
    let event = trigger.event();
    let entity = trigger.entity;

    let vanish_reason = match event.vanish_type {
        0 => "out of sight",
        1 => "died",
        2 => "logged out",
        3 => "teleported",
        _ => "unknown",
    };

    // Check if entity is moving
    if let Ok(movement_state) = movement_query.get(entity) {
        if matches!(movement_state, MovementState::Moving) {
            // Entity is moving - defer despawn until movement completes
            debug!(
                "Entity {:?} (AID {}) is moving, deferring despawn ({})",
                entity, event.aid, vanish_reason
            );

            commands
                .entity(entity)
                .insert(PendingDespawn::new(event.vanish_type));
            return;
        }
    }

    // Entity is idle or missing MovementState - despawn immediately
    debug!(
        "Entity {:?} (AID {}) is idle, despawning immediately ({})",
        entity, event.aid, vanish_reason
    );

    commands.trigger(DespawnEntity {
        entity,
        aid: event.aid,
    });
}

/// Check pending despawns and despawn entities when movement completes
///
/// This system runs every frame to check entities marked with PendingDespawn.
/// When an entity finishes moving (MovementState::Idle) or the timeout expires,
/// it triggers a DespawnEntity observer to handle actual despawn.
pub fn check_pending_despawns_system(
    mut commands: Commands,
    query: Query<(Entity, &PendingDespawn, &MovementState, &NetworkEntity)>,
) {
    for (entity, pending, movement_state, network_entity) in query.iter() {
        let should_despawn =
            matches!(movement_state, MovementState::Idle) || pending.has_timed_out();

        if should_despawn {
            let reason = if pending.has_timed_out() {
                "timeout"
            } else {
                "movement complete"
            };

            debug!(
                "Pending despawn completed for entity {:?} (AID {}, reason: {})",
                entity, network_entity.aid, reason
            );

            commands.trigger(DespawnEntity {
                entity,
                aid: network_entity.aid,
            });

            // Remove PendingDespawn component (entity will be despawned by observer)
            commands.entity(entity).remove::<PendingDespawn>();
        }
    }
}

/// Debug system to log spawn details for newly spawned network entities
///
/// Uses Bevy 0.17's SpawnDetails to track entity spawn timing.
/// This is useful for debugging spawn order, performance analysis, and
/// understanding entity creation patterns.
///
/// Enable this system in debug builds or when investigating spawn issues.
#[allow(dead_code)]
pub fn debug_entity_spawn_timing_system(
    query: Query<(Entity, &NetworkEntity, SpawnDetails), Spawned>,
) {
    for (entity, network_entity, spawn_details) in query.iter() {
        debug!(
            "Entity spawned: {:?} (AID: {}, GID: {}, Type: {:?}) at tick {:?}",
            entity,
            network_entity.aid,
            network_entity.gid,
            network_entity.object_type,
            spawn_details.spawn_tick()
        );
    }
}
