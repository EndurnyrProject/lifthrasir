use crate::{
    core::state::GameState,
    domain::{
        entities::{
            character::{
                components::{
                    core::{CharacterAppearance, CharacterData, CharacterStats, Gender, Grounded},
                    equipment::EquipmentSet,
                    visual::{CharacterDirection, CharacterSprite, Direction},
                },
                states::{AnimationState, StatusEffects},
            },
            components::{GuildIdentity, NetworkEntity, PendingDespawn, SpawnGuildIdentityKnown},
            markers::*,
            movement::components::{MovementSpeed, MovementState},
            registry::EntityRegistry,
            spawning::events::{DespawnEntity, EntityVanishRequested, PendingSpawnBuffer},
            sprite_rendering::{
                components::{EntitySpriteData, EntitySpriteInfo},
                events::RequestSpriteSpawn,
            },
        },
        system_sets::EntityLifecycleSystems,
        world::map_scoped::MapScoped,
    },
    infrastructure::job::{registry::WARP_JOB_ID, JobSpriteRegistry},
    utils::coordinates::spawn_coords_to_world_position,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::SpatialAudioEmitter;
use net_contract::events::{UnitEntered, UnitLeft};

/// Legacy-shaped view of a `UnitEntered`, so the spawn body keeps its original
/// field names/types after the move/stand/new-entry collapse onto one event.
struct SpawnFields {
    aid: u32,
    gid: u32,
    object_type: crate::domain::entities::types::ObjectType,
    name: String,
    position: (u16, u16),
    direction: u8,
    /// Server movement speed in ms per cell (lower = faster). Drives walk-animation cadence.
    speed: u16,
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
    level: u16,
    guild_id: u32,
    guild_name: String,
    emblem_id: u32,
}

impl From<&UnitEntered> for SpawnFields {
    fn from(e: &UnitEntered) -> Self {
        // remote movement is driven by snapshot interpolation, so the spawn path
        // always sets up a standing entity; `UnitEntered`'s move fields are ignored here.
        SpawnFields {
            aid: e.aid,
            gid: e.gid,
            object_type: crate::domain::entities::types::ObjectType::from(e.object_type as u8),
            name: e.name.clone(),
            position: (e.x as u16, e.y as u16),
            direction: e.dir as u8,
            speed: e.speed as u16,
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
            level: e.clevel as u16,
            guild_id: e.guild_id,
            guild_name: e.guild_name.clone(),
            emblem_id: e.emblem_id,
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
) {
    for unit in spawn_events.read() {
        let event = SpawnFields::from(unit);

        // Check if entity already exists (e.g., spawned from character selection or re-entering view)
        if let Some(existing_entity) = entity_registry.get_entity(event.gid) {
            // Re-entering view: de-queue any pending despawn. Remote movement is now driven
            // by snapshot interpolation, so we no longer forward a per-step destination here.
            let mut entity = commands.entity(existing_entity);
            entity.remove::<PendingDespawn>();
            if event.object_type == crate::domain::entities::types::ObjectType::Pc {
                entity.insert(SpawnGuildIdentityKnown);
                if event.guild_id != 0 {
                    entity.insert(GuildIdentity {
                        guild_id: event.guild_id,
                        guild_name: event.guild_name,
                        emblem_id: event.emblem_id,
                    });
                } else {
                    entity.remove::<GuildIdentity>();
                }
            } else {
                entity.remove::<(GuildIdentity, SpawnGuildIdentityKnown)>();
            }
            continue;
        }

        let is_warp = matches!(
            event.object_type,
            crate::domain::entities::types::ObjectType::Npc
        ) && event.job as u32 == WARP_JOB_ID;

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
            MapScoped,
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
            crate::domain::entities::types::ObjectType::SkillUnit => {
                unreachable!("SkillUnit is client-spawned only, never parsed from UnitEntered")
            }
        }

        match event.object_type {
            crate::domain::entities::types::ObjectType::Pc => {
                entity_cmd.insert((
                    RemotePlayer,
                    SpatialAudioEmitter::default(),
                    SpawnGuildIdentityKnown,
                ));
                if event.guild_id != 0 {
                    entity_cmd.insert(GuildIdentity {
                        guild_id: event.guild_id,
                        guild_name: event.guild_name.clone(),
                        emblem_id: event.emblem_id,
                    });
                }
                debug!("Spawned remote player: {} (AID: {})", event.name, event.aid);
            }
            crate::domain::entities::types::ObjectType::Npc => {
                entity_cmd.insert(Npc);
                if is_warp {
                    entity_cmd.insert(WarpPortal);
                }
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
            crate::domain::entities::types::ObjectType::SkillUnit => {
                unreachable!("SkillUnit is client-spawned only, never parsed from UnitEntered")
            }
        }

        entity_cmd.insert((AnimationState::Idle, StatusEffects::default()));

        // Remote entities are placed standing; their position is driven by snapshot
        // interpolation via `interpolate_remote_entities_system`.
        entity_cmd.insert((
            MovementState::Idle,
            MovementSpeed::from_server_speed(event.speed),
            Grounded,
        ));

        debug!(
            "Entity {} spawned IDLE at ({}, {})",
            event.name, event.position.0, event.position.1
        );

        let entity_id = entity_cmd.id();

        entity_registry.register_entity(event.gid, entity_id);

        // Warp portals render as a 3D VFX (WarpPortal -> PortalVfx), not a sprite.
        if is_warp {
            continue;
        }

        // Route sprite spawning based on entity type
        match event.object_type {
            crate::domain::entities::types::ObjectType::Pc => {
                let sprite_data = EntitySpriteData::Character {
                    job_id: event.job,
                    gender: Gender::from(event.gender),
                    head: event.head,
                };

                let sprite_info = EntitySpriteInfo { sprite_data };
                debug!(
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
                debug!(
                    "Triggering RequestSpriteSpawn for {:?} entity {:?} (job={}) at position ({:.2}, {:.2}, {:.2})",
                    event.object_type, entity_id, event.job, world_pos.x, world_pos.y, world_pos.z
                );
                commands.trigger(RequestSpriteSpawn {
                    entity: entity_id,
                    position: world_pos,
                    sprite_info,
                });
            }
            crate::domain::entities::types::ObjectType::SkillUnit => {
                unreachable!("SkillUnit is client-spawned only, never parsed from UnitEntered")
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

    commands.entity(entity).try_despawn();

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
/// death (vanish_type 1) is deferred via PendingDespawn so combat::handle_death can
/// play the death animation; every other vanish despawns immediately.
#[auto_observer(plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin)]
pub fn on_entity_vanish_request(trigger: On<EntityVanishRequested>, mut commands: Commands) {
    let event = trigger.event();
    let entity = trigger.entity;

    let vanish_reason = match event.vanish_type {
        0 => "out of sight",
        1 => "died",
        2 => "logged out",
        3 => "teleported",
        _ => "unknown",
    };

    // Only death defers. Remote entities are snapshot-interpolated, so a vanished unit
    // receives no further updates to "finish" a move with - deferring on a stale Moving
    // state (its last snapshot was mid-walk) leaves it frozen on screen until the timeout.
    if event.vanish_type == 1 {
        debug!(
            "Entity {:?} (AID {}) deferring despawn ({})",
            entity, event.aid, vanish_reason
        );

        commands
            .entity(entity)
            .insert(PendingDespawn::new(event.vanish_type));
        return;
    }

    debug!(
        "Entity {:?} (AID {}) despawning immediately ({})",
        entity, event.aid, vanish_reason
    );

    commands.trigger(DespawnEntity {
        entity,
        aid: event.aid,
    });
}

/// Despawn death entities once their deferral timeout expires.
///
/// Only death (vanish_type 1) entities carry PendingDespawn (see on_entity_vanish_request);
/// they stay on screen for their death animation and despawn when the timeout elapses. The
/// DespawnEntity observer despawns the whole entity, so there is no component to remove here.
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(in_set = EntityLifecycleSystems::Spawning)
)]
pub fn check_pending_despawns_system(
    mut commands: Commands,
    query: Query<(Entity, &PendingDespawn, &NetworkEntity)>,
) {
    for (entity, pending, network_entity) in query.iter() {
        if !pending.has_timed_out() {
            continue;
        }

        debug!(
            "Pending despawn timed out for entity {:?} (AID {})",
            entity, network_entity.aid
        );

        commands.trigger(DespawnEntity {
            entity,
            aid: network_entity.gid,
        });
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
        debug!(
            "Draining {} buffered spawn events on entering InGame",
            count
        );
        for event in buffer.events.drain(..) {
            spawn_writer.write(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        entities::{
            components::{EntityName, GuildIdentity},
            name_request_system::name_response_handler_system,
        },
        system_sets::{EntityInteractionSystems, EntityLifecycleSystems},
    };
    use net_contract::events::EntityNamed;

    fn unit(guild_id: u32, guild_name: &str, emblem_id: u32) -> UnitEntered {
        UnitEntered {
            gid: 150_001,
            aid: 2_000_001,
            object_type: 0,
            job: 7,
            x: 100,
            y: 200,
            dir: 4,
            speed: 150,
            hp: 4_000,
            max_hp: 4_200,
            clevel: 99,
            body_state: 0,
            health_state: 0,
            effect_state: 0,
            head: 12,
            weapon: 0,
            shield: 0,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            head_palette: 0,
            body_palette: 0,
            head_dir: 0,
            robe: 0,
            guild_id,
            guild_name: guild_name.into(),
            emblem_id,
            sex: 1,
            is_boss: false,
            name: "Alice".into(),
            moving: false,
            dst_x: 0,
            dst_y: 0,
            move_start_time: 0,
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<UnitEntered>()
            .add_message::<EntityNamed>()
            .init_resource::<EntityRegistry>()
            .add_systems(
                Update,
                (
                    spawn_network_entity_system.in_set(EntityLifecycleSystems::Spawning),
                    ApplyDeferred
                        .after(EntityLifecycleSystems::Spawning)
                        .before(EntityInteractionSystems::Naming),
                    name_response_handler_system.in_set(EntityInteractionSystems::Naming),
                ),
            );
        app
    }

    #[test]
    fn guilded_remote_pc_receives_spawn_identity() {
        let mut app = app();
        app.world_mut()
            .write_message(unit(42, "Knights of Midgard", 9));

        app.update();

        let mut query = app.world_mut().query::<&GuildIdentity>();
        let identity = query.single(app.world()).unwrap();
        assert_eq!(identity.guild_id, 42);
        assert_eq!(identity.guild_name, "Knights of Midgard");
        assert_eq!(identity.emblem_id, 9);
    }

    #[test]
    fn unguilded_remote_pc_has_no_guild_identity() {
        let mut app = app();
        app.world_mut().write_message(unit(0, "", 0));

        app.update();

        let mut query = app.world_mut().query::<&GuildIdentity>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    #[test]
    fn non_pc_has_no_guild_identity_even_when_spawn_carries_guild_fields() {
        let mut app = app();
        let mut npc = unit(42, "Knights of Midgard", 9);
        npc.object_type = 1;
        app.world_mut().write_message(npc);

        app.update();

        let mut query = app.world_mut().query::<&GuildIdentity>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    #[test]
    fn visibility_respawn_replaces_guild_identity_with_new_spawn_version() {
        let mut app = app();
        app.world_mut().write_message(unit(42, "Old Guild", 9));
        app.update();

        app.world_mut().write_message(unit(77, "New Guild", 10));
        app.update();

        let mut query = app.world_mut().query::<&GuildIdentity>();
        let identity = query.single(app.world()).unwrap();
        assert_eq!(identity.guild_id, 77);
        assert_eq!(identity.guild_name, "New Guild");
        assert_eq!(identity.emblem_id, 10);
    }

    #[test]
    fn visibility_respawn_as_unguilded_removes_old_identity() {
        let mut app = app();
        app.world_mut().write_message(unit(42, "Old Guild", 9));
        app.update();

        app.world_mut().write_message(unit(0, "", 0));
        app.update();

        let mut query = app.world_mut().query::<&GuildIdentity>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    #[test]
    fn stale_name_response_cannot_restore_guild_after_unguilded_respawn() {
        let mut app = app();
        app.world_mut().write_message(unit(42, "Old Guild", 9));
        app.update();

        app.world_mut().write_message(unit(0, "", 0));
        app.update();

        app.world_mut().write_message(EntityNamed {
            gid: 150_001,
            name: "Alice".into(),
            party_name: String::new(),
            guild_name: "Old Guild".into(),
            position_name: "Old Position".into(),
        });
        app.update();

        let entity = app
            .world()
            .resource::<EntityRegistry>()
            .get_entity(150_001)
            .unwrap();
        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.get::<GuildIdentity>().is_none());
        assert_eq!(
            entity_ref
                .get::<EntityName>()
                .unwrap()
                .guild_name
                .as_deref(),
            None
        );
    }

    #[test]
    fn same_frame_unguilded_respawn_precedes_stale_name_response() {
        let mut app = app();
        app.world_mut().write_message(unit(42, "Old Guild", 9));
        app.update();

        app.world_mut().write_message(unit(0, "", 0));
        app.world_mut().write_message(EntityNamed {
            gid: 150_001,
            name: "Alice".into(),
            party_name: String::new(),
            guild_name: "Old Guild".into(),
            position_name: "Old Position".into(),
        });
        app.update();

        let entity = app
            .world()
            .resource::<EntityRegistry>()
            .get_entity(150_001)
            .unwrap();
        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.get::<GuildIdentity>().is_none());
        assert_eq!(
            entity_ref
                .get::<EntityName>()
                .unwrap()
                .guild_name
                .as_deref(),
            None
        );
    }
}
