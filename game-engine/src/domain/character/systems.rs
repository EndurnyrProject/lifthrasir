use super::components::{CharacterSelectionState, MapLoadingTimer};
use super::events::*;
use crate::core::state::GameState;
use crate::domain::entities::character::components::CharacterInfo;
use crate::domain::system_sets::CharacterFlowSystems;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use crate::infrastructure::networking::char_messages::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::quic::character::CharacterRoster;
use crate::infrastructure::networking::quic::zone::QuicZoneState;
use crate::infrastructure::networking::session::UserSession;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::SpatialAudioReceiver;
use bevy_quinnet::client::QuinnetClient;
use std::time::{Duration, Instant};

/// Builds the slot-keyed character list event from the QUIC roster, resolving
/// job names and sprite paths. Shared by the roster-change and manual-request
/// paths so the mapping lives in one place.
fn build_character_list_event(
    roster: &CharacterRoster,
    job_registry: Option<&JobSpriteRegistry>,
) -> CharacterListReceivedEvent {
    let mut char_list = vec![None; 15];

    for net_char in &roster.characters {
        let slot = net_char.char_num as usize;
        if slot >= char_list.len() {
            continue;
        }

        let job_name = job_registry
            .and_then(|reg| reg.get_display_name(net_char.class as u32))
            .unwrap_or("Unknown")
            .to_string();

        let body_sprite_path = job_registry
            .and_then(|reg| reg.get_body_sprite_path(net_char.class as u32, net_char.sex))
            .unwrap_or_else(|| "data\\sprite\\인간족\\몸통\\남\\초보자_남.spr".to_string());

        let hair_sprite_path = job_registry
            .map(|reg| reg.get_hair_sprite_path(net_char.hair, net_char.sex))
            .unwrap_or_else(|| "data\\sprite\\인간족\\머리통\\남\\1_남.spr".to_string());

        let hair_palette_path = job_registry.and_then(|reg| {
            reg.get_hair_palette_path(net_char.hair, net_char.sex, net_char.hair_color)
        });

        char_list[slot] = Some(CharacterInfoWithJobName {
            base: net_char.clone(),
            job_name,
            body_sprite_path,
            hair_sprite_path,
            hair_palette_path,
        });
    }

    CharacterListReceivedEvent {
        characters: char_list,
        max_slots: 9,
        available_slots: 9,
        display_pages: roster.page_count.min(u8::MAX as u32) as u8,
    }
}

/// System to handle explicit character list requests
/// The character list is cached in `CharacterRoster` by the QUIC drain.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterList)
)]
pub fn handle_request_character_list(
    mut request_events: MessageReader<RequestCharacterListEvent>,
    roster: Res<CharacterRoster>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut list_events: MessageWriter<CharacterListReceivedEvent>,
) {
    for _event in request_events.read() {
        list_events.write(build_character_list_event(&roster, job_registry.as_deref()));
    }
}

/// System: Handle character server connection and transition to selection
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharServerConnection)
)]
pub fn handle_character_server_connected(
    mut connected_events: MessageReader<CharacterServerConnected>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for _ in connected_events.read() {
        next_state.set(GameState::CharacterSelection);
    }
}

/// System: Re-emit the UI character list whenever the roster mutates.
///
/// The `!roster.is_added()` guard skips the startup default-insert frame so we
/// don't emit an empty list; the first real list arrives when the QUIC drain
/// mutates the roster (is_changed, not is_added). Not gated on the selection
/// state, since the first `CharList` is processed the same frame the state
/// transition is queued.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterList)
)]
pub fn handle_character_roster_changed(
    roster: Res<CharacterRoster>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut list_events: MessageWriter<CharacterListReceivedEvent>,
) {
    if roster.is_changed() && !roster.is_added() {
        list_events.write(build_character_list_event(&roster, job_registry.as_deref()));
    }
}

/// System: Handle character creation success from protocol
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_created_protocol(
    mut protocol_events: MessageReader<CharacterCreated>,
    mut domain_events: MessageWriter<CharacterCreatedEvent>,
) {
    for event in protocol_events.read() {
        let char_info = CharacterInfo::from(event.character.clone());
        domain_events.write(CharacterCreatedEvent {
            character: char_info,
            slot: event.character.char_num,
        });
    }
}

/// System: Handle character deletion success from protocol
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterDeletion)
)]
pub fn handle_character_deleted_protocol(
    mut protocol_events: MessageReader<CharacterDeleted>,
    mut domain_events: MessageWriter<CharacterDeletedEvent>,
) {
    for event in protocol_events.read() {
        domain_events.write(CharacterDeletedEvent {
            character_id: event.char_id,
        });
    }
}

/// System: Handle character creation failures from protocol
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_creation_failed_protocol(
    mut protocol_events: MessageReader<CharacterCreationFailed>,
    mut domain_events: MessageWriter<CharacterCreationFailedEvent>,
) {
    for event in protocol_events.read() {
        use crate::infrastructure::networking::char_types::CharCreationError;
        let error_msg = match event.error {
            CharCreationError::NameExists => "Character name already exists",
            CharCreationError::InvalidName => "Invalid character name",
            CharCreationError::Unknown(_) => "Unknown error",
        };
        domain_events.write(CharacterCreationFailedEvent {
            slot: 0,
            error: error_msg.to_string(),
        });
    }
}

/// System: Handle character deletion failures from protocol
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterDeletion)
)]
pub fn handle_character_deletion_failed_protocol(
    mut protocol_events: MessageReader<CharacterDeletionFailed>,
    mut domain_events: MessageWriter<CharacterDeletionFailedEvent>,
) {
    for event in protocol_events.read() {
        domain_events.write(CharacterDeletionFailedEvent {
            character_id: event.char_id,
            error: event.error.description().to_string(),
        });
    }
}

/// System: Handle zone server info from protocol
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneServerInfo)
)]
pub fn handle_zone_server_info_protocol(
    mut protocol_events: MessageReader<ZoneServerInfoReceived>,
    user_session: Option<Res<UserSession>>,
    mut domain_events: MessageWriter<super::events::ZoneServerInfoReceivedEvent>,
) {
    for event in protocol_events.read() {
        let Some(session) = user_session.as_ref() else {
            error!("ZoneServerInfo received but UserSession not available - cannot proceed");
            continue;
        };

        let server_ip = event.zone_server_info.ip_string();
        domain_events.write(super::events::ZoneServerInfoReceivedEvent {
            char_id: event.zone_server_info.char_id,
            map_name: event.zone_server_info.map_name.clone(),
            server_ip,
            server_port: event.zone_server_info.port,
            account_id: session.tokens.account_id,
            login_id1: session.tokens.login_id1,
            sex: session.sex,
        });
    }
}

/// System that spawns unified character entities from CharacterSelectedEvent
/// Creates a complete character entity with all three ECS components
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterSelection)
)]
pub fn spawn_unified_character_from_selection(
    mut events: MessageReader<CharacterSelectedEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (char_data, appearance, meta) = event.character.clone().into_components();

        commands.spawn((
            char_data,
            appearance,
            meta,
            crate::domain::entities::character::components::visual::CharacterSprite::default(),
            crate::domain::entities::character::components::visual::CharacterDirection::default(),
            Name::new(format!("Character_{}", event.character.char_id)),
        ));

        debug!(
            "Spawned character entity for char_id: {} ({})",
            event.character.char_id, event.character.name
        );
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterSelection)
)]
pub fn handle_select_character(
    mut events: MessageReader<SelectCharacterEvent>,
    roster: Res<CharacterRoster>,
    mut state: ResMut<CharacterSelectionState>,
    mut selected_events: MessageWriter<CharacterSelectedEvent>,
) {
    for event in events.read() {
        state.selected_slot = Some(event.slot);

        let Some(net_char) = roster
            .characters
            .iter()
            .find(|c| c.char_num == event.slot)
            .cloned()
        else {
            error!("Character not found in slot {}", event.slot);
            continue;
        };

        selected_events.write(CharacterSelectedEvent {
            character: CharacterInfo::from(net_char),
            slot: event.slot,
        });
    }
}

/// Temporary resource to store zone session data for use after protocol events
#[derive(Resource, Debug, Clone)]
pub struct ZoneSessionData {
    pub map_name: String,
    pub character_id: u32,
    pub account_id: u32,
    pub character_name: String,
}

/// System: Handle zone server info and open the QUIC zone connection.
///
/// Closes the char connection, opens the QUIC zone connection, and arms
/// `QuicZoneState` with the session credentials (sourced from the live
/// `UserSession` for `login_id2`, which the handoff event does not carry) and
/// the target map. The handshake (`Hello`/`SessionAuth`/`EnterAck`) is driven
/// from there by `quic::zone::flow::handshake`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneServerInfo)
)]
pub fn handle_zone_server_info(
    mut events: MessageReader<ZoneServerInfoReceivedEvent>,
    mut quinnet: ResMut<QuinnetClient>,
    mut zone_state: ResMut<QuicZoneState>,
    user_session: Res<UserSession>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    characters: Query<(
        &crate::domain::entities::character::components::CharacterMeta,
        &crate::domain::entities::character::components::CharacterData,
    )>,
) {
    for event in events.read() {
        info!("Connecting to zone server for map: {}", event.map_name);

        let character_name = characters
            .iter()
            .find(|(meta, _)| meta.char_id == event.char_id)
            .map(|(_, data)| data.name.clone())
            .unwrap_or_else(|| {
                warn!("Character name not found for char_id: {}", event.char_id);
                "Unknown".to_string()
            });

        commands.insert_resource(ZoneSessionData {
            map_name: event.map_name.clone(),
            character_id: event.char_id,
            account_id: event.account_id,
            character_name,
        });

        info!("Closing QUIC character connection before zone handoff");

        let server_address = format!("{}:{}", event.server_ip, event.server_port);

        if let Err(e) =
            crate::infrastructure::networking::quic::zone::connect(&mut quinnet, &server_address)
        {
            error!(
                "Failed to connect to zone server at {}: {:?}",
                server_address, e
            );
            continue;
        }

        info!("Opened QUIC zone connection to {}", server_address);

        zone_state.start_connecting(
            crate::infrastructure::networking::quic::zone::ZoneAuth {
                account_id: event.account_id,
                login_id1: event.login_id1,
                login_id2: user_session.tokens.login_id2,
                sex: event.sex as u32,
                char_id: event.char_id,
            },
            event.map_name.clone(),
        );

        game_state.set(GameState::Connecting);
    }
}

/// System: Handle zone entry accepted (QUIC `ZoneEntered` from the handshake).
///
/// The QUIC handshake emits `ZoneEntered` on `EnterAck` with the spawn cell;
/// the map name and char id come from the armed `QuicZoneState`. This feeds the
/// existing map-load machinery (`MapSpawnContext` + `MapLoadingStarted`) and
/// transitions to `Loading`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneConnection)
)]
pub fn handle_zone_entered(
    mut entered_events: MessageReader<
        crate::infrastructure::networking::zone_messages::ZoneEntered,
    >,
    zone_state: Res<QuicZoneState>,
    mut commands: Commands,
    mut map_loading_events: MessageWriter<MapLoadingStarted>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in entered_events.read() {
        info!(
            "Zone server accepted entry! Spawning at ({}, {}) facing {}",
            event.x, event.y, event.dir
        );

        let map_name = zone_state.map_name.clone();
        let character_id = zone_state.auth.char_id;

        commands.insert_resource(MapSpawnContext::new(
            map_name.clone(),
            event.x as u16,
            event.y as u16,
            character_id,
        ));

        map_loading_events.write(MapLoadingStarted {
            map_name: map_name.clone(),
        });

        game_state.set(GameState::Loading);
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadDetect)
)]
pub fn detect_map_load_complete(
    query: Query<
        (Entity, &crate::domain::world::map::MapData),
        Added<crate::domain::world::map::MapData>,
    >,
    spawn_context: Option<Res<MapSpawnContext>>,
    mut events: MessageWriter<MapLoadCompleted>,
) {
    for (entity, _map_data) in query.iter() {
        let Some(context) = spawn_context.as_ref() else {
            warn!(
                "MapData spawned (entity {:?}) but MapSpawnContext not available - skipping",
                entity
            );
            continue;
        };

        debug!(
            "Map loading completed: {} (entity {:?})",
            context.map_name, entity
        );
        events.write(MapLoadCompleted {
            map_name: context.map_name.clone(),
        });
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadComplete)
)]
pub fn handle_map_load_complete(
    mut events: MessageReader<MapLoadCompleted>,
    mut actor_init_events: MessageWriter<ActorInitSent>,
) {
    for event in events.read() {
        debug!(
            "Map '{}' loaded; MapLoaded already sent by zone_send_map_loaded",
            event.map_name
        );
        actor_init_events.write(ActorInitSent);
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ActorInit)
)]
pub fn handle_actor_init_sent(
    mut events: MessageReader<ActorInitSent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for _event in events.read() {
        info!("Entering game world");
        game_state.set(GameState::InGame);
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_created(
    mut events: MessageReader<CharacterCreatedEvent>,
    mut state: ResMut<CharacterSelectionState>,
    mut refresh_events: MessageWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        state.is_creating_character = false;
        state.creation_slot = None;
        refresh_events.write(RefreshCharacterListEvent);
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterDeletion)
)]
pub fn handle_character_deleted(
    mut events: MessageReader<CharacterDeletedEvent>,
    mut refresh_events: MessageWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        refresh_events.write(RefreshCharacterListEvent);
    }
}

/// System to start map loading timer when map loading begins
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadStart)
)]
pub fn start_map_loading_timer(
    mut events: MessageReader<MapLoadingStarted>,
    mut commands: Commands,
) {
    for event in events.read() {
        debug!("Starting map loading timeout timer for: {}", event.map_name);
        commands.insert_resource(MapLoadingTimer {
            started: Instant::now(),
            map_name: event.map_name.clone(),
        });
    }
}

/// System to detect map loading timeout
/// Checks if map assets haven't loaded within 30 seconds
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadTimeout)
)]
pub fn detect_map_loading_timeout(
    timer: Option<Res<MapLoadingTimer>>,
    map_data_query: Query<&crate::domain::world::map::MapData>,
    mut failed_events: MessageWriter<MapLoadingFailed>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
) {
    const MAP_LOADING_TIMEOUT: Duration = Duration::from_secs(30);

    let Some(timer_res) = timer else {
        return;
    };

    if !map_data_query.is_empty() {
        debug!(
            "Map '{}' loaded successfully, removing timeout timer",
            timer_res.map_name
        );
        commands.remove_resource::<MapLoadingTimer>();
        return;
    }

    if timer_res.started.elapsed() > MAP_LOADING_TIMEOUT {
        error!(
            "Map loading timeout for '{}' - assets failed to load within 30 seconds",
            timer_res.map_name
        );

        let map_name = timer_res.map_name.clone();

        failed_events.write(MapLoadingFailed {
            map_name: map_name.clone(),
            reason: format!(
                "Map assets for '{}' failed to load within 30 seconds. The map files may be missing or corrupted.",
                map_name
            ),
        });

        commands.remove_resource::<MapLoadingTimer>();

        game_state.set(GameState::CharacterSelection);
    }
}

/// System that spawns character sprite hierarchy when entering InGame state
/// This bridges the character entity creation with the unified sprite system
#[allow(clippy::too_many_arguments)]
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = OnEnter(GameState::InGame)
)]
pub fn spawn_character_sprite_on_game_start(
    mut commands: Commands,
    mut spawn_events: MessageWriter<crate::domain::entities::character::SpawnCharacterSpriteEvent>,
    spawn_context: Res<MapSpawnContext>,
    mut entity_registry: ResMut<crate::domain::entities::registry::EntityRegistry>,
    user_session: Res<UserSession>,
    characters: Query<(
        Entity,
        &crate::domain::entities::character::components::CharacterMeta,
        &crate::domain::entities::character::components::CharacterData,
    )>,
    map_loader_query: Query<&crate::domain::world::components::MapLoader>,
    ground_assets: Res<Assets<crate::infrastructure::assets::loaders::RoGroundAsset>>,
) {
    // Find character entity matching spawn context
    let Some((character_entity, _meta, char_data)) = characters
        .iter()
        .find(|(_, meta, _)| meta.char_id == spawn_context.character_id)
    else {
        error!(
            "Character entity not found for char_id: {}",
            spawn_context.character_id
        );
        return;
    };

    // Add gameplay components (movement, state machine, animation states)
    crate::domain::entities::character::add_gameplay_components_to_entity(
        &mut commands.entity(character_entity),
    );

    // Add NetworkEntity component for hover system and combat lookups
    // Note: For player characters, GID == AID (account_id) in RO protocol
    // The server uses account_id as the entity identifier in combat packets
    let account_id = user_session.tokens.account_id;
    commands.entity(character_entity).insert(
        crate::domain::entities::components::NetworkEntity::new(
            account_id,
            account_id, // GID should be account_id for combat packet lookups
            crate::domain::entities::types::ObjectType::Pc,
        ),
    );

    // Add LocalPlayer marker and CharacterStatus
    // Note: Server doesn't send STANDENTRY for local player, so we add these immediately
    commands.entity(character_entity).insert((
        crate::domain::entities::markers::LocalPlayer,
        crate::domain::entities::character::components::status::CharacterStatus::default(),
        crate::domain::entities::components::EntityName::new(char_data.name.clone()),
        SpatialAudioReceiver,
        crate::domain::input::PlayerAction::default_input_map(),
    ));
    entity_registry.set_local_player(character_entity, account_id);
    info!(
        "Spawned LOCAL PLAYER entity {:?} (AID: {}) '{}' with LocalPlayer + CharacterStatus + EntityName components",
        character_entity, account_id, char_data.name
    );

    // Get map dimensions from loaded ground data
    let (map_width, map_height) = if let Ok(map_loader) = map_loader_query.single() {
        if let Some(ground_asset) = ground_assets.get(&map_loader.ground) {
            (ground_asset.ground.width, ground_asset.ground.height)
        } else {
            panic!("MapLoader entity found but ground asset not loaded yet");
        }
    } else {
        panic!("No MapLoader entity found - cannot determine map dimensions");
    };

    // Calculate world position from spawn coordinates
    let world_pos = crate::utils::coordinates::spawn_coords_to_world_position(
        spawn_context.spawn_x,
        spawn_context.spawn_y,
        map_width,
        map_height,
    );

    // Add Transform to the character entity for positioning and camera tracking
    commands.entity(character_entity).insert((
        Transform::from_translation(world_pos),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Emit event to spawn sprite hierarchy
    spawn_events.write(
        crate::domain::entities::character::SpawnCharacterSpriteEvent {
            character_entity,
            spawn_position: world_pos,
        },
    );

    debug!("Spawning character sprite at {:?}", world_pos);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::proto::aesir::net;

    fn char_list_with_one(char_num: u32, name: &str) -> net::CharList {
        net::CharList {
            account_id: 2000001,
            normal_slots: 9,
            premium_slots: 0,
            billing_slots: 0,
            producible_slots: 9,
            valid_slots: 9,
            characters: vec![net::Character {
                gid: 150001,
                name: name.into(),
                class: 7,
                base_level: 42,
                job_level: 10,
                base_exp: 0,
                job_exp: 0,
                zeny: 0,
                hp: 0,
                max_hp: 0,
                sp: 0,
                max_sp: 0,
                str: 0,
                agi: 0,
                vit: 0,
                int: 0,
                dex: 0,
                luk: 0,
                status_point: 0,
                skill_point: 0,
                hair: 0,
                hair_color: 0,
                clothes_color: 0,
                weapon: 0,
                shield: 0,
                head_top: 0,
                head_mid: 0,
                head_bottom: 0,
                robe: 0,
                char_num,
                last_map: "prontera".into(),
                sex: 0,
                option: 0,
                karma: 0,
                manner: 0,
                rename: 0,
                delete_date: 0,
            }],
            page_count: 3,
            pincode_enabled: false,
        }
    }

    #[test]
    fn build_character_list_event_places_character_at_its_slot() {
        let mut roster = CharacterRoster::default();
        roster.update_from_char_list(&char_list_with_one(2, "Vidar"));

        let event = build_character_list_event(&roster, None);

        assert_eq!(event.display_pages, roster.page_count as u8);
        assert!(event.characters[0].is_none());
        let placed = event.characters[2]
            .as_ref()
            .expect("character should land in slot 2");
        assert_eq!(placed.base.name, "Vidar");
        assert_eq!(placed.base.char_num, 2);
    }
}
