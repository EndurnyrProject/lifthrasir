use super::components::{CharServerPingTimer, CharacterSelectionState, MapLoadingTimer};
use super::events::*;
use crate::core::state::GameState;
use crate::domain::entities::character::components::CharacterInfo;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::networking::client::CharServerClient;
use crate::infrastructure::networking::protocol::character::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::session::UserSession;
use bevy::prelude::*;
use std::time::{Duration, Instant};

/// System to handle explicit character list requests
/// The character list is cached in CharServerClient after connection
pub fn handle_request_character_list(
    mut request_events: MessageReader<RequestCharacterListEvent>,
    char_client: Option<Res<CharServerClient>>,
    mut list_events: MessageWriter<CharacterListReceivedEvent>,
) {
    for _event in request_events.read() {
        if let Some(client) = char_client.as_ref() {
            // Convert cached characters to domain model
            let mut char_list = vec![None; 15]; // Support up to 15 slots

            for net_char in client.characters() {
                let char_info = CharacterInfo::from(net_char.clone());
                let slot = net_char.char_num as usize;
                if slot < char_list.len() {
                    char_list[slot] = Some(char_info);
                }
            }

            list_events.write(CharacterListReceivedEvent {
                characters: char_list,
                max_slots: 9,
                available_slots: 9,
            });
        } else {
            warn!("RequestCharacterListEvent received but CharServerClient not initialized");
        }
    }
}

/// System: Handle character server connection and emit character list
/// The new architecture receives character list upon HC_ACCEPT_ENTER
pub fn handle_character_server_connected(
    mut connected_events: MessageReader<CharacterServerConnected>,
    char_client: Option<Res<CharServerClient>>,
    mut list_events: MessageWriter<CharacterListReceivedEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for _event in connected_events.read() {
        let Some(client) = char_client.as_ref() else {
            continue;
        };

        // Initialize ping timer for 15-second keep-alive pings
        commands.insert_resource(CharServerPingTimer(Timer::from_seconds(
            15.0,
            TimerMode::Repeating,
        )));

        let mut char_list = vec![None; 15];

        for net_char in client.characters() {
            let char_info = CharacterInfo::from(net_char.clone());
            let slot = net_char.char_num as usize;
            if slot < char_list.len() {
                char_list[slot] = Some(char_info);
            }
        }

        list_events.write(CharacterListReceivedEvent {
            characters: char_list,
            max_slots: 9,
            available_slots: 9,
        });

        next_state.set(GameState::CharacterSelection);
    }
}

/// System: Handle character creation success from protocol
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
pub fn handle_character_deleted_protocol(
    mut protocol_events: MessageReader<CharacterDeleted>,
    mut domain_events: MessageWriter<CharacterDeletedEvent>,
) {
    for _event in protocol_events.read() {
        domain_events.write(CharacterDeletedEvent { character_id: 0 });
    }
}

/// System: Handle character creation failures from protocol
pub fn handle_character_creation_failed_protocol(
    mut protocol_events: MessageReader<CharacterCreationFailed>,
    mut domain_events: MessageWriter<CharacterCreationFailedEvent>,
) {
    for event in protocol_events.read() {
        use crate::infrastructure::networking::protocol::character::CharCreationError;
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
pub fn handle_character_deletion_failed_protocol(
    mut protocol_events: MessageReader<CharacterDeletionFailed>,
    mut domain_events: MessageWriter<CharacterDeletionFailedEvent>,
) {
    for event in protocol_events.read() {
        use crate::infrastructure::networking::protocol::character::CharDeletionError;
        let error_msg = match event.error {
            CharDeletionError::NotEligible => "Not eligible to delete",
            CharDeletionError::Unknown(_) => "Unknown error",
        };
        domain_events.write(CharacterDeletionFailedEvent {
            character_id: 0,
            error: error_msg.to_string(),
        });
    }
}

/// System: Handle zone server info from protocol
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

pub fn handle_select_character(
    mut events: MessageReader<SelectCharacterEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
    mut state: ResMut<CharacterSelectionState>,
    mut selected_events: MessageWriter<CharacterSelectedEvent>,
) {
    for event in events.read() {
        state.selected_slot = Some(event.slot);

        if let Some(client) = char_client.as_deref_mut() {
            // Find the character data for this slot
            let character = client
                .characters()
                .iter()
                .find(|c| c.char_num == event.slot)
                .cloned();

            if let Some(net_char) = character {
                if let Err(e) = client.select_character(event.slot) {
                    error!("Failed to select character: {:?}", e);
                } else {
                    // Emit success event with character data so Tauri bridge can respond
                    selected_events.write(CharacterSelectedEvent {
                        character: CharacterInfo::from(net_char),
                        slot: event.slot,
                    });
                }
            } else {
                error!("Character not found in slot {}", event.slot);
            }
        } else {
            error!("CharServerClient not initialized - cannot select character");
        }
    }
}

pub fn handle_create_character(
    mut events: MessageReader<CreateCharacterRequestEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in events.read() {
        if let Err(e) = event.form.validate() {
            error!("Character creation validation failed: {:?}", e);
            continue;
        }

        if let Some(client) = char_client.as_deref_mut() {
            if let Err(e) = client.create_character(
                &event.form.name,
                event.form.slot,
                event.form.hair_color,
                event.form.hair_style,
                event.form.starting_job as u16,
            ) {
                error!("Failed to create character: {:?}", e);
            }
        } else {
            error!("CharServerClient not initialized - cannot create character");
        }
    }
}

pub fn handle_delete_character(
    mut events: MessageReader<DeleteCharacterRequestEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in events.read() {
        if let Some(client) = char_client.as_deref_mut() {
            if let Err(e) = client.delete_character(event.character_id, "") {
                error!("Failed to delete character: {:?}", e);
            }
        } else {
            error!("CharServerClient not initialized - cannot delete character");
        }
    }
}

/// Temporary resource to store zone session data for use after protocol events
#[derive(Resource, Debug, Clone)]
pub struct ZoneSessionData {
    pub map_name: String,
    pub character_id: u32,
    pub account_id: u32,
}

/// System: Handle zone server info and connect to zone server
/// Uses the new ZoneServerClient architecture
pub fn handle_zone_server_info(
    mut events: MessageReader<ZoneServerInfoReceivedEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for event in events.read() {
        info!("Connecting to zone server for map: {}", event.map_name);

        // Store session data for use after protocol events
        commands.insert_resource(ZoneSessionData {
            map_name: event.map_name.clone(),
            character_id: event.char_id,
            account_id: event.account_id,
        });

        // Disconnect from character server (no longer needed)
        if let Some(ref mut client) = char_client.as_deref_mut() {
            info!("Disconnecting from character server");
            client.disconnect();
        }

        // Create ZoneServerClient with session data
        let mut zone_client_instance =
            crate::infrastructure::networking::client::ZoneServerClient::with_session(
                event.account_id,
                event.char_id,
            );

        // Connect immediately using the newly created instance
        let server_address = format!("{}:{}", event.server_ip, event.server_port);

        match zone_client_instance.connect(&server_address) {
            Ok(()) => {
                info!("Connected to zone server at {}", server_address);

                // Send CZ_ENTER2 packet
                if let Err(e) = zone_client_instance.enter_world(
                    event.account_id,
                    event.char_id,
                    event.login_id1,
                    0, // client_time (can be 0)
                    event.sex,
                ) {
                    error!("Failed to send zone entry packet: {:?}", e);
                } else {
                    info!("Sent CZ_ENTER2 to zone server");
                    game_state.set(GameState::Connecting);
                }
            }
            Err(e) => {
                error!("Failed to connect to zone server: {:?}", e);
            }
        }

        // Insert the connected client as a resource for other systems to use
        commands.insert_resource(zone_client_instance);
    }
}

/// System: Handle successful zone connection from protocol events
/// Replaces the old zone_packet_handler_system
pub fn handle_zone_server_connected_protocol(
    mut protocol_events: MessageReader<
        crate::infrastructure::networking::protocol::zone::ZoneServerConnected,
    >,
    zone_session: Option<Res<ZoneSessionData>>,
    mut domain_events: MessageWriter<ZoneAuthenticationSuccess>,
    mut commands: Commands,
    mut map_loading_events: MessageWriter<MapLoadingStarted>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in protocol_events.read() {
        info!(
            "Zone server accepted entry! Spawning at ({}, {}) facing {}",
            event.spawn_data.position.x, event.spawn_data.position.y, event.spawn_data.position.dir
        );

        // Get session data from resource
        let Some(session) = zone_session.as_ref() else {
            error!("ZoneSessionData not available - cannot proceed with spawn");
            continue;
        };

        // Store spawn context
        commands.insert_resource(MapSpawnContext::new(
            session.map_name.clone(),
            event.spawn_data.position.x,
            event.spawn_data.position.y,
            session.character_id,
        ));

        // Emit domain event for compatibility
        domain_events.write(ZoneAuthenticationSuccess {
            spawn_x: event.spawn_data.position.x,
            spawn_y: event.spawn_data.position.y,
            spawn_dir: event.spawn_data.position.dir,
            server_tick: event.spawn_data.server_tick,
        });

        // Emit map loading started
        map_loading_events.write(MapLoadingStarted {
            map_name: session.map_name.clone(),
        });

        game_state.set(GameState::Loading);
    }
}

/// System: Handle zone entry refused
pub fn handle_zone_entry_refused_protocol(
    mut protocol_events: MessageReader<
        crate::infrastructure::networking::protocol::zone::ZoneEntryRefused,
    >,
    mut zone_client: Option<ResMut<crate::infrastructure::networking::client::ZoneServerClient>>,
    mut domain_events: MessageWriter<ZoneAuthenticationFailed>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in protocol_events.read() {
        warn!(
            "Zone entry refused: {:?} - {}",
            event.error, event.error_description
        );

        // Convert ZoneEntryError to a simple error code for the domain event
        let error_code = match event.error {
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::Normal => 0,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::ServerClosed => 1,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::AlreadyLoggedIn => 2,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::AlreadyLoggedInAlt => 3,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::EnvironmentError => 4,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::PreviousConnectionActive => 8,
            crate::infrastructure::networking::protocol::zone::ZoneEntryError::Unknown(code) => code,
        };

        // Emit domain event
        domain_events.write(ZoneAuthenticationFailed { error_code });

        // Disconnect zone client
        if let Some(ref mut client) = zone_client.as_deref_mut() {
            client.disconnect();
        }

        // Return to character selection
        game_state.set(GameState::CharacterSelection);
    }
}

/// System: Handle account ID received (ZC_AID packet)
/// This is informational but we log it for debugging
pub fn handle_account_id_received_protocol(
    mut protocol_events: MessageReader<
        crate::infrastructure::networking::protocol::zone::AccountIdReceived,
    >,
) {
    for event in protocol_events.read() {
        debug!("Account ID confirmed by zone server: {}", event.account_id);
    }
}

pub fn detect_map_load_complete(
    query: Query<&crate::domain::world::map::MapData, Added<crate::domain::world::map::MapData>>,
    spawn_context: Option<Res<MapSpawnContext>>,
    mut events: MessageWriter<MapLoadCompleted>,
) {
    for _map_data in query.iter() {
        let Some(context) = spawn_context.as_ref() else {
            warn!("MapData added but MapSpawnContext not available - skipping");
            continue;
        };

        debug!("Map loading completed: {}", context.map_name);
        events.write(MapLoadCompleted {
            map_name: context.map_name.clone(),
        });
    }
}

pub fn handle_map_load_complete(
    mut events: MessageReader<MapLoadCompleted>,
    mut zone_client: Option<ResMut<crate::infrastructure::networking::client::ZoneServerClient>>,
    mut actor_init_events: MessageWriter<ActorInitSent>,
) {
    for event in events.read() {
        debug!(
            "Map '{}' loaded, sending CZ_NOTIFY_ACTORINIT",
            event.map_name
        );

        if let Some(client) = zone_client.as_deref_mut() {
            if let Err(e) = client.notify_ready() {
                error!("Failed to send actor init: {:?}", e);
            } else {
                info!("Sent CZ_NOTIFY_ACTORINIT to zone server");
                actor_init_events.write(ActorInitSent);
            }
        }
    }
}

pub fn handle_actor_init_sent(
    mut events: MessageReader<ActorInitSent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for _event in events.read() {
        info!("Entering game world");
        // TODO: Spawn player character entity here in future
        game_state.set(GameState::InGame);
    }
}

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

pub fn handle_character_deleted(
    mut events: MessageReader<CharacterDeletedEvent>,
    mut refresh_events: MessageWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        refresh_events.write(RefreshCharacterListEvent);
    }
}

pub fn handle_refresh_character_list(
    mut events: MessageReader<RefreshCharacterListEvent>,
    char_client: Option<Res<CharServerClient>>,
    mut list_events: MessageWriter<CharacterListReceivedEvent>,
) {
    for _event in events.read() {
        // New architecture: Characters are automatically updated in the context by handlers
        // Just re-emit the current character list
        if let Some(client) = char_client.as_ref() {
            let mut char_list = vec![None; 15];

            for net_char in client.characters() {
                let char_info = CharacterInfo::from(net_char.clone());
                let slot = net_char.char_num as usize;
                if slot < char_list.len() {
                    char_list[slot] = Some(char_info);
                }
            }

            list_events.write(CharacterListReceivedEvent {
                characters: char_list,
                max_slots: 9,
                available_slots: 9,
            });
        }
    }
}

/// System that sends periodic pings to keep the connection alive
/// Throttled to every 15 seconds using CharServerPingTimer
pub fn update_char_client(
    char_client: Option<ResMut<CharServerClient>>,
    time: Res<Time>,
    ping_timer: Option<ResMut<CharServerPingTimer>>,
) {
    if let (Some(mut client), Some(mut timer)) = (char_client, ping_timer) {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            let _ = client.send_ping();
        }
    }
}

/// System to start map loading timer when map loading begins
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
pub fn detect_map_loading_timeout(
    timer: Option<Res<MapLoadingTimer>>,
    map_data_query: Query<&crate::domain::world::map::MapData>,
    mut zone_client: Option<ResMut<crate::infrastructure::networking::client::ZoneServerClient>>,
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

        if let Some(client) = zone_client.as_deref_mut() {
            debug!("Disconnecting from zone server due to timeout");
            client.disconnect();
        }

        commands.remove_resource::<MapLoadingTimer>();

        game_state.set(GameState::CharacterSelection);
    }
}

/// System that spawns character sprite hierarchy when entering InGame state
/// This bridges the character entity creation with the unified sprite system
pub fn spawn_character_sprite_on_game_start(
    mut commands: Commands,
    mut spawn_events: MessageWriter<
        crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent,
    >,
    spawn_context: Res<MapSpawnContext>,
    characters: Query<(
        Entity,
        &crate::domain::entities::character::components::CharacterMeta,
    )>,
    map_loader_query: Query<&crate::domain::world::components::MapLoader>,
    ground_assets: Res<Assets<crate::infrastructure::assets::loaders::RoGroundAsset>>,
) {
    // Find character entity matching spawn context
    let Some((character_entity, _meta)) = characters
        .iter()
        .find(|(_, meta)| meta.char_id == spawn_context.character_id)
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

    // Add PlayerCharacter marker
    commands
        .entity(character_entity)
        .insert(crate::domain::camera::components::PlayerCharacter);

    info!(
        "✅ Added gameplay components to player character entity {:?}",
        character_entity
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

    // Emit event to spawn sprite hierarchy
    spawn_events.write(
        crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent {
            character_entity,
            spawn_position: world_pos,
        },
    );

    debug!("Spawning character sprite at {:?}", world_pos);
}
