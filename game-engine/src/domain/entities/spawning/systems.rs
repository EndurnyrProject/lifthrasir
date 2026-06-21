use crate::{
    core::state::GameState,
    domain::{
        combat::components::Combatant,
        entities::{
            character::{
                components::{
                    core::{CharacterAppearance, CharacterData, CharacterStats, Gender, Grounded},
                    equipment::EquipmentSet,
                    visual::{CharacterDirection, CharacterSprite, Direction},
                },
                states::{AnimationState, StatusEffects},
            },
            components::{NetworkEntity, PendingDespawn},
            markers::*,
            movement::components::{MovementSpeed, MovementState, MovementTarget},
            pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
            registry::EntityRegistry,
            spawning::events::{
                DespawnEntity, EntityVanishRequested, PendingSpawnBuffer,
            },
            sprite_rendering::{
                components::{EntitySpriteData, EntitySpriteInfo},
                events::RequestSpriteSpawn,
            },
        },
        system_sets::EntityLifecycleSystems,
    },
    infrastructure::{
        job::JobSpriteRegistry,
        networking::zone_messages::{UnitEntered, UnitLeft},
    },
    utils::coordinates::spawn_coords_to_world_position,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::SpatialAudioEmitter;

/// Legacy-shaped view of a `UnitEntered`, so the spawn body keeps its original
/// field names/types after the move/stand/new-entry collapse onto one event.
struct SpawnFields {
    aid: u32,
    gid: u32,
    object_type: crate::domain::entities::types::ObjectType,
    name: String,
    position: (u16, u16),
    direction: u8,
    destination: Option<(u16, u16)>,
    move_start_time: Option<u32>,
    current_server_tick: u32,
    job: u16,
    head: u16,
    gender: u8,
    head_palette: u16,
    body_palette: u16,
    weapon: u32,
    shield: u32,
    head_bottom: u16,
    head_mid: u16,
    head_top: u16,
    robe: u16,
    hp: u32,
    max_hp: u32,
    speed: u16,
    level: u16,
}

impl From<&UnitEntered> for SpawnFields {
    fn from(e: &UnitEntered) -> Self {
        // NOTE: `UnitEntered` collapses STAND/NEW/MOVE-entry; `moving` selects the walking
        // branch and the `dst_*`/`move_start_time` fields carry the move.
        let destination = if e.moving {
            Some((e.dst_x as u16, e.dst_y as u16))
        } else {
            None
        };
        // NOTE: the proto sends no per-spawn server "now"; with only move_start_time,
        // elapsed = now - start collapses to 0 (entity walks from its source cell).
        // Upgrade path: derive a current tick from the time-sync clock (Approach B).
        SpawnFields {
            aid: e.aid,
            gid: e.gid,
            object_type: crate::domain::entities::types::ObjectType::from(e.object_type as u8),
            name: e.name.clone(),
            position: (e.x as u16, e.y as u16),
            direction: e.dir as u8,
            destination,
            move_start_time: e.moving.then_some(e.move_start_time as u32),
            current_server_tick: e.move_start_time as u32,
            job: e.job as u16,
            head: e.head as u16,
            gender: e.sex as u8,
            head_palette: e.head_palette as u16,
            body_palette: e.body_palette as u16,
            weapon: e.weapon,
            shield: e.shield,
            head_bottom: e.accessory as u16,
            head_mid: e.accessory2 as u16,
            head_top: e.accessory3 as u16,
            robe: e.robe as u16,
            hp: e.hp,
            max_hp: e.max_hp,
            speed: e.speed as u16,
            level: e.clevel as u16,
        }
    }
}

/// Spawn network entities from UnitEntered events
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(
        in_set = EntityLifecycleSystems::Spawning,
        run_if = in_state(GameState::InGame)
    )
)]
#[allow(clippy::too_many_arguments)]
pub fn spawn_network_entity_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<UnitEntered>,
    mut entity_registry: ResMut<EntityRegistry>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    pathfinding_grid: Option<Res<CurrentMapPathfindingGrid>>,
) {
    for unit in spawn_events.read() {
        let event = SpawnFields::from(unit);

        // Check if entity already exists (e.g., spawned from character selection or re-entering view)
        if let Some(existing_entity) = entity_registry.get_entity(event.aid) {
            commands.entity(existing_entity).remove::<PendingDespawn>();

            // NOTE: a moving entity re-entering view keeps its existing interpolated
            // movement; the remote-position refresh that rode the legacy movement event
            // is now owned by the periodic Snapshot path (Approach B, deferred).
            debug!(
                "Entity AID {} re-entered view, canceling pending despawn",
                event.aid
            );
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

        let mut entity_cmd = commands.spawn((
            NetworkEntity::new(event.aid, event.gid, event.object_type),
            Transform::from_translation(world_pos),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
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
                entity_cmd.insert(RemotePlayer);
                debug!("Spawned remote player: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Npc => {
                entity_cmd.insert(Npc);
                debug!("Spawned NPC: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Mob => {
                entity_cmd.insert((Mob, SpatialAudioEmitter::default()));
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
            AnimationState::Idle,
            StatusEffects::default(),
            Combatant::new(150),
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
                AnimationState::Walking,
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
                let sprite_data = EntitySpriteData::Character {
                    job_id: event.job,
                    gender: Gender::from(event.gender),
                    head: event.head,
                };

                let sprite_info = EntitySpriteInfo { sprite_data };
                info!(
                    "Triggering RequestSpriteSpawn for PC entity {:?} (job={}, head={}) at position ({:.2}, {:.2}, {:.2})",
                    entity_id, event.job, event.head, world_pos.x, world_pos.y, world_pos.z
                );
                commands.trigger(RequestSpriteSpawn {
                    entity: entity_id,
                    position: world_pos,
                    sprite_info,
                });
            }
            crate::domain::entities::types::ObjectType::Mob
            | crate::domain::entities::types::ObjectType::Npc
            | crate::domain::entities::types::ObjectType::Homunculus
            | crate::domain::entities::types::ObjectType::Mercenary
            | crate::domain::entities::types::ObjectType::Elemental => {
                let sprite_name = if let Some(registry) = job_registry.as_ref() {
                    registry
                        .get_sprite_name(event.job as u32)
                        .unwrap_or("초보자")
                        .to_string()
                } else {
                    warn!("JobSpriteRegistry not loaded yet, using fallback");
                    "초보자".to_string()
                };

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

                let sprite_info = EntitySpriteInfo { sprite_data };
                info!(
                    "Triggering RequestSpriteSpawn for {:?} entity {:?} (job={}) at position ({:.2}, {:.2}, {:.2})",
                    event.object_type, entity_id, event.job, world_pos.x, world_pos.y, world_pos.z
                );
                commands.trigger(RequestSpriteSpawn {
                    entity: entity_id,
                    position: world_pos,
                    sprite_info,
                });
            }
        }

        debug!(
            "Spawned entity: {} ({:?}) at ({}, {}) - Entity ID: {:?}",
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
/// Handles DespawnEntity observer events and despawns the entity.
/// This observer is triggered when an entity needs to be removed from the game world.
#[auto_observer(plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin)]
pub fn on_despawn_entity(
    trigger: On<DespawnEntity>,
    mut commands: Commands,
    mut entity_registry: ResMut<EntityRegistry>,
) {
    let event = trigger.event();
    let entity = trigger.entity;

    commands.entity(entity).despawn();

    entity_registry.unregister_entity_by_aid(event.aid);

    debug!("Despawned entity {:?} with AID: {}", entity, event.aid);
}

/// Cleanup system: Remove entities from registry when they despawn
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(in_set = EntityLifecycleSystems::Despawning)
)]
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
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(in_set = EntityLifecycleSystems::Vanishing)
)]
pub fn bridge_vanish_requests_system(
    mut commands: Commands,
    mut vanish_requests: MessageReader<UnitLeft>,
    entity_registry: Res<EntityRegistry>,
    entity_query: Query<Entity>,
) {
    for request in vanish_requests.read() {
        let Some(entity) = entity_registry.get_entity(request.gid) else {
            warn!(
                "Vanish request for unknown entity GID: {} (may already be despawned)",
                request.gid
            );
            continue;
        };

        if entity_query.get(entity).is_err() {
            debug!(
                "Vanish request for GID {} but entity {:?} not in ECS yet (registry desync)",
                request.gid, entity
            );
            continue;
        }

        commands.trigger(EntityVanishRequested {
            entity,
            aid: request.gid,
            vanish_type: request.reason as u8,
        });
    }
}

/// Observer for entity vanish requests
///
/// When an entity vanishes (moves out of range, dies, logs out, or teleports),
/// this observer checks if the entity is currently moving. If so, it marks the
/// entity with PendingDespawn component to defer actual despawn until movement
/// completes. This prevents entities from disappearing mid-movement.
#[auto_observer(plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin)]
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

    let is_moving = movement_query
        .get(entity)
        .is_ok_and(|state| matches!(state, MovementState::Moving));

    // Death (vanish_type 1) plays its animation via combat::handle_death, and moving
    // entities finish their move first - both defer despawn instead of removing now.
    if event.vanish_type == 1 || is_moving {
        debug!(
            "Entity {:?} (AID {}) deferring despawn ({})",
            entity, event.aid, vanish_reason
        );

        commands
            .entity(entity)
            .insert(PendingDespawn::new(event.vanish_type));
        return;
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
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(in_set = EntityLifecycleSystems::Spawning)
)]
pub fn check_pending_despawns_system(
    mut commands: Commands,
    query: Query<(Entity, &PendingDespawn, &MovementState, &NetworkEntity)>,
) {
    for (entity, pending, movement_state, network_entity) in query.iter() {
        // Dead entities are idle while their death animation plays, so they despawn
        // only on timeout; others despawn as soon as movement completes.
        let should_despawn = if pending.vanish_type == 1 {
            pending.has_timed_out()
        } else {
            matches!(movement_state, MovementState::Idle) || pending.has_timed_out()
        };

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

/// Capture spawn events when not in InGame state
///
/// This system runs during Connecting and Loading states to buffer spawn events
/// that arrive before the game is ready to process them. The events are stored
/// in PendingSpawnBuffer and replayed when entering InGame state.
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(
        run_if = not(in_state(GameState::InGame))
    )
)]
pub fn buffer_spawn_events_system(
    mut spawn_events: MessageReader<UnitEntered>,
    mut buffer: ResMut<PendingSpawnBuffer>,
) {
    for event in spawn_events.read() {
        debug!(
            "Buffering spawn event for {} (AID {}) - not in InGame state",
            event.name, event.aid
        );
        buffer.events.push(event.clone());
    }
}

/// Drain buffered spawn events when entering InGame state
///
/// This system runs once when the game transitions to InGame state.
/// It replays all buffered spawn events so entities that appeared
/// during Connecting/Loading states are properly spawned.
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = OnEnter(GameState::InGame)
)]
pub fn drain_spawn_buffer_system(
    mut buffer: ResMut<PendingSpawnBuffer>,
    mut spawn_writer: MessageWriter<UnitEntered>,
) {
    let count = buffer.events.len();
    if count > 0 {
        info!(
            "Draining {} buffered spawn events on entering InGame",
            count
        );
        for event in buffer.events.drain(..) {
            spawn_writer.write(event);
        }
    }
}
