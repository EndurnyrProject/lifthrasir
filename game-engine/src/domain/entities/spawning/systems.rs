use bevy::prelude::*;
use crate::{
    domain::{
        entities::{
            components::NetworkEntity,
            markers::*,
            movement::components::{MovementSpeed, MovementState, MovementTarget},
            character::components::{
                core::{CharacterData, CharacterAppearance, CharacterStats, Grounded, Gender},
                equipment::EquipmentSet,
                visual::{CharacterSprite, CharacterDirection, Direction},
            },
            registry::EntityRegistry,
            spawning::events::SpawnEntity,
        },
    },
    infrastructure::networking::session::UserSession,
    utils::coordinates::spawn_coords_to_world_position,
};

/// Spawn network entities from SpawnEntity events
pub fn spawn_network_entity_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<SpawnEntity>,
    mut entity_registry: ResMut<EntityRegistry>,
    user_session: Option<Res<UserSession>>,
    mut sprite_spawn: MessageWriter<
        crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent
    >,
) {
    for event in spawn_events.read() {
        if let Some(existing_entity) = entity_registry.get_entity(event.aid) {
            warn!(
                "Entity with AID {} already exists as {:?}, skipping spawn",
                event.aid, existing_entity
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
            } else { None },
            head_mid: if event.head_mid > 0 {
                Some(create_equipment_item(event.head_mid as u32))
            } else { None },
            head_bottom: if event.head_bottom > 0 {
                Some(create_equipment_item(event.head_bottom as u32))
            } else { None },
            weapon: if event.weapon > 0 {
                Some(create_equipment_item(event.weapon))
            } else { None },
            shield: if event.shield > 0 {
                Some(create_equipment_item(event.shield))
            } else { None },
            armor: None,
            garment: if event.robe > 0 {
                Some(create_equipment_item(event.robe as u32))
            } else { None },
            shoes: None,
            accessories: [None, None],
        };

        let world_pos = spawn_coords_to_world_position(
            event.position.0,
            event.position.1,
            0,
            0,
        );

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
                    info!("Spawned LOCAL PLAYER: {} (AID: {})", event.name, event.aid);
                } else {
                    entity_cmd.insert(RemotePlayer);
                    info!("Spawned remote player: {} (AID: {})", event.name, event.aid);
                }
            }
            crate::domain::entities::types::ObjectType::Npc => {
                entity_cmd.insert(Npc);
                info!("Spawned NPC: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Mob => {
                entity_cmd.insert(Mob);
                info!("Spawned mob: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Homunculus => {
                entity_cmd.insert(Homunculus);
                info!("Spawned homunculus: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Mercenary => {
                entity_cmd.insert(Mercenary);
                info!("Spawned mercenary: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Elemental => {
                entity_cmd.insert(Elemental);
                info!("Spawned elemental: {} (AID: {})", event.name, event.aid);
            }
        }

        if let Some(destination) = event.destination {
            let dest_world_pos = spawn_coords_to_world_position(
                destination.0,
                destination.1,
                0,
                0,
            );

            let target = MovementTarget::new(
                event.position.0,
                event.position.1,
                destination.0,
                destination.1,
                world_pos,
                dest_world_pos,
                event.move_start_time.unwrap_or(0),
            );

            entity_cmd.insert((
                target,
                MovementState::Moving,
                MovementSpeed::from_server_speed(event.speed),
                Grounded,
            ));

            debug!(
                "Entity {} spawned MOVING: ({}, {}) -> ({}, {})",
                event.name,
                event.position.0, event.position.1,
                destination.0, destination.1
            );
        } else {
            entity_cmd.insert((
                MovementState::Idle,
                MovementSpeed::default_walk(),
                Grounded,
            ));

            debug!(
                "Entity {} spawned IDLE at ({}, {})",
                event.name,
                event.position.0, event.position.1
            );
        }

        let entity_id = entity_cmd.id();

        entity_registry.register_entity(event.aid, entity_id);

        sprite_spawn.write(
            crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent {
                character_entity: entity_id,
                spawn_position: world_pos,
            }
        );

        info!(
            "✅ Spawned entity: {} ({:?}) at ({}, {}) - Entity ID: {:?}",
            event.name, event.object_type,
            event.position.0, event.position.1,
            entity_id
        );
    }
}

/// Helper to create equipment item from ID
fn create_equipment_item(item_id: u32) -> crate::domain::entities::character::components::equipment::EquipmentItem {
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
) {
    for event in despawn_events.read() {
        if let Some(entity) = entity_registry.get_entity(event.aid) {
            commands.entity(entity).despawn();

            entity_registry.unregister_entity_by_aid(event.aid);

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
