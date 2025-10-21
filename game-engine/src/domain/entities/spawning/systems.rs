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
        registry::EntityRegistry,
        spawning::events::{DespawnEntity, RequestEntityVanish, SpawnEntity},
    },
    infrastructure::networking::{protocol::zone::MovementConfirmedByServer, session::UserSession},
    utils::coordinates::spawn_coords_to_world_position,
};
use bevy::prelude::*;

/// Spawn network entities from SpawnEntity events
pub fn spawn_network_entity_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<SpawnEntity>,
    mut entity_registry: ResMut<EntityRegistry>,
    user_session: Option<Res<UserSession>>,
    mut sprite_spawn: MessageWriter<
        crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent,
    >,
    mut movement_events: MessageWriter<MovementConfirmedByServer>,
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
            char_data,
            appearance,
            equipment,
            CharacterSprite::default(),
            CharacterDirection {
                facing: Direction::from_u8(event.direction),
            },
            Transform::from_translation(world_pos),
            Visibility::default(),
            Name::new(format!(
                "{} ({:?}:{})",
                event.name, event.object_type, event.aid
            )),
        ));

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

            // Use new_with_elapsed to create movement target with correct timing
            let target = MovementTarget::new_with_elapsed(
                event.position.0,
                event.position.1,
                destination.0,
                destination.1,
                world_pos,
                dest_world_pos,
                event.move_start_time.unwrap_or(0),
                elapsed_ms,
            );

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

        sprite_spawn.write(
            crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent {
                character_entity: entity_id,
                spawn_position: world_pos,
            },
        );

        debug!(
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

/// Despawn network entities when they leave view
pub fn despawn_network_entity_system(
    mut commands: Commands,
    mut despawn_events: MessageReader<crate::domain::entities::spawning::events::DespawnEntity>,
    mut entity_registry: ResMut<EntityRegistry>,
    mut despawned_this_frame: Local<std::collections::HashSet<u32>>,
    character_trees: Query<
        &crate::domain::entities::character::sprite_hierarchy::CharacterObjectTree,
    >,
) {
    despawned_this_frame.clear();

    for event in despawn_events.read() {
        if despawned_this_frame.contains(&event.aid) {
            info!(
                "🔄 Skipping duplicate despawn event for AID: {} (already processed this frame)",
                event.aid
            );
            continue;
        }

        if let Some(entity) = entity_registry.get_entity(event.aid) {
            // Despawn sprite hierarchy first to prevent race condition
            // where update_sprite_layer_transforms tries to access a despawned character entity
            if let Ok(object_tree) = character_trees.get(entity) {
                commands.entity(object_tree.root).despawn();
            }

            commands.entity(entity).despawn();

            entity_registry.unregister_entity_by_aid(event.aid);
            despawned_this_frame.insert(event.aid);

            info!("✅ Despawned entity with AID: {}", event.aid);
        } else {
            warn!(
                "⚠️ Attempted to despawn unknown entity with AID: {}",
                event.aid
            );
        }
    }
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

/// Handle RequestEntityVanish events and defer despawn for moving entities
///
/// When an entity vanishes (moves out of range, dies, logs out, or teleports),
/// this system checks if the entity is currently moving. If so, it marks the
/// entity with PendingDespawn component to defer actual despawn until movement
/// completes. This prevents entities from disappearing mid-movement.
pub fn handle_vanish_request_system(
    mut commands: Commands,
    mut vanish_requests: MessageReader<RequestEntityVanish>,
    mut despawn_events: MessageWriter<DespawnEntity>,
    entity_registry: Res<EntityRegistry>,
    movement_query: Query<&MovementState>,
) {
    for request in vanish_requests.read() {
        let Some(entity) = entity_registry.get_entity(request.aid) else {
            warn!(
                "Vanish request for unknown entity AID: {} (may already be despawned)",
                request.aid
            );
            continue;
        };

        let vanish_reason = match request.vanish_type {
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
                    "Entity {} is moving, deferring despawn ({})",
                    request.aid, vanish_reason
                );

                commands
                    .entity(entity)
                    .insert(PendingDespawn::new(request.vanish_type));
                continue;
            }
        }

        // Entity is idle or missing MovementState - despawn immediately
        debug!(
            "Entity {} is idle, despawning immediately ({})",
            request.aid, vanish_reason
        );

        despawn_events.write(DespawnEntity { aid: request.aid });
    }
}

/// Check pending despawns and despawn entities when movement completes
///
/// This system runs every frame to check entities marked with PendingDespawn.
/// When an entity finishes moving (MovementState::Idle) or the timeout expires,
/// it emits a DespawnEntity event to trigger actual despawn.
pub fn check_pending_despawns_system(
    mut commands: Commands,
    query: Query<(Entity, &PendingDespawn, &MovementState, &NetworkEntity)>,
    mut despawn_events: MessageWriter<DespawnEntity>,
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
                "Pending despawn completed for AID {} ({})",
                network_entity.aid, reason
            );

            despawn_events.write(DespawnEntity {
                aid: network_entity.aid,
            });

            // Remove PendingDespawn component (entity will be despawned by despawn_network_entity_system)
            commands.entity(entity).remove::<PendingDespawn>();
        }
    }
}
