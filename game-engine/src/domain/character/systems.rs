use super::components::{CharacterSelectionState, MapLoadingTimer};
use super::events::*;
use crate::core::state::GameState;
use crate::domain::entities::character::components::CharacterInfo;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::networking::protocols::ro_char::ChMakeCharPacket;
use crate::infrastructure::networking::session::UserSession;
use crate::infrastructure::networking::{CharServerClient, CharServerEvent};
use bevy::prelude::*;
use std::time::{Duration, Instant};

/// System to handle explicit character list requests
/// The character list is cached in CharServerClient after connection
pub fn handle_request_character_list(
    mut request_events: EventReader<RequestCharacterListEvent>,
    char_client: Option<Res<CharServerClient>>,
    mut list_events: EventWriter<CharacterListReceivedEvent>,
) {
    for _event in request_events.read() {
        if let Some(client) = char_client.as_ref() {
            // Convert cached characters to domain model
            let mut char_list = vec![None; 15]; // Support up to 15 slots

            for net_char in &client.characters {
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

/// System 1: Handle character list received events
pub fn handle_character_list_events(
    mut char_events: EventReader<CharServerEvent>,
    mut list_events: EventWriter<CharacterListReceivedEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in char_events.read() {
        if let CharServerEvent::CharacterListReceived(characters) = event {
            let mut char_list = vec![None; 15];

            for net_char in characters {
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
}

/// System 2: Handle successful character operations
pub fn handle_character_operations_success(
    mut char_events: EventReader<CharServerEvent>,
    mut created_events: EventWriter<CharacterCreatedEvent>,
    mut deleted_events: EventWriter<CharacterDeletedEvent>,
) {
    for event in char_events.read() {
        match event {
            CharServerEvent::CharacterCreated(net_char) => {
                let char_info = CharacterInfo::from(net_char.clone());
                created_events.write(CharacterCreatedEvent {
                    character: char_info,
                    slot: net_char.char_num,
                });
            }
            CharServerEvent::CharacterDeleted => {
                deleted_events.write(CharacterDeletedEvent { character_id: 0 });
            }
            _ => {}
        }
    }
}

/// System 3: Handle character operation errors
pub fn handle_character_operations_errors(
    mut char_events: EventReader<CharServerEvent>,
    mut creation_failed_events: EventWriter<CharacterCreationFailedEvent>,
    mut deletion_failed_events: EventWriter<CharacterDeletionFailedEvent>,
) {
    for event in char_events.read() {
        match event {
            CharServerEvent::CharacterCreationFailed(error_code) => {
                let error_msg = match error_code {
                    0x00 => "Character name already exists",
                    0x01 => "Invalid character name",
                    0x02 => "Character slot is full",
                    0x03 => "Character creation denied",
                    _ => "Unknown error",
                };
                creation_failed_events.write(CharacterCreationFailedEvent {
                    slot: 0,
                    error: error_msg.to_string(),
                });
            }
            CharServerEvent::CharacterDeletionFailed(error_code) => {
                let error_msg = match error_code {
                    0x00 => "Character not found",
                    0x01 => "Character cannot be deleted",
                    0x02 => "Invalid email address",
                    _ => "Unknown error",
                };
                deletion_failed_events.write(CharacterDeletionFailedEvent {
                    character_id: 0,
                    error: error_msg.to_string(),
                });
            }
            _ => {}
        }
    }
}

/// System 4: Handle zone server information
pub fn handle_zone_server_info_events(
    mut char_events: EventReader<CharServerEvent>,
    user_session: Option<Res<UserSession>>,
    mut zone_events: EventWriter<ZoneServerInfoReceivedEvent>,
) {
    for event in char_events.read() {
        if let CharServerEvent::ZoneServerInfo {
            char_id,
            map_name,
            ip,
            port,
        } = event
        {
            let Some(session) = user_session.as_ref() else {
                error!("ZoneServerInfo received but UserSession not available - cannot proceed");
                continue;
            };

            let server_ip = format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
            zone_events.write(ZoneServerInfoReceivedEvent {
                char_id: *char_id,
                map_name: map_name.clone(),
                server_ip,
                server_port: *port,
                account_id: session.tokens.account_id,
                login_id1: session.tokens.login_id1,
                sex: session.sex,
            });
        }
    }
}

/// System 5: Log connection info events
pub fn log_connection_info_events(mut char_events: EventReader<CharServerEvent>) {
    for event in char_events.read() {
        match event {
            CharServerEvent::ConnectionError(error) => {
                error!("Character server connection error: {:?}", error);
            }
            CharServerEvent::CharacterSlotInfo {
                normal_slots,
                premium_slots,
                valid_slots,
            } => {
                debug!(
                    "Character slots - normal: {}, premium: {}, valid: {}",
                    normal_slots, premium_slots, valid_slots
                );
                // Slot info is primarily for UI display
            }
            CharServerEvent::BlockedCharacterList(blocked) => {
                if !blocked.is_empty() {
                    warn!("Received {} blocked characters", blocked.len());
                    for (char_id, expire_date) in blocked {
                        warn!("Character ID {} is blocked until: {}", char_id, expire_date);
                    }
                }
            }
            CharServerEvent::PincodeState { state, description } => {
                info!("Pincode state {}: {}", state, description);
                match state {
                    0 => {} // Pincode disabled or correct - continue normally
                    1 => warn!("Server requires pincode input - not yet implemented"),
                    2 | 4 => warn!("Server requires creating new pincode - not yet implemented"),
                    3 => warn!("Server requires changing pincode - not yet implemented"),
                    8 => error!("Pincode was incorrect"),
                    _ => warn!("Unknown pincode state: {}", state),
                }
            }
            _ => {}
        }
    }
}

/// System that spawns unified character entities from CharacterSelectedEvent
/// Creates a complete character entity with all three ECS components
pub fn spawn_unified_character_from_selection(
    mut events: EventReader<CharacterSelectedEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (char_data, appearance, meta) = event.character.clone().into_components();

        commands.spawn((
            char_data,
            appearance,
            meta,
            crate::domain::entities::character::components::visual::CharacterSprite::default(),
            Name::new(format!("Character_{}", event.character.char_id)),
        ));

        debug!(
            "Spawned character entity for char_id: {} ({})",
            event.character.char_id, event.character.name
        );
    }
}

pub fn handle_select_character(
    mut events: EventReader<SelectCharacterEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
    mut state: ResMut<CharacterSelectionState>,
    mut selected_events: EventWriter<CharacterSelectedEvent>,
) {
    for event in events.read() {
        state.selected_slot = Some(event.slot);

        if let Some(client) = char_client.as_deref_mut() {
            // Find the character data for this slot
            let character = client
                .characters
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
    mut events: EventReader<CreateCharacterRequestEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in events.read() {
        if let Err(e) = event.form.validate() {
            error!("Character creation validation failed: {:?}", e);
            continue;
        }

        if let Some(client) = char_client.as_deref_mut() {
            let packet = ChMakeCharPacket {
                name: event.form.name.clone(),
                slot: event.form.slot,
                hair_color: event.form.hair_color,
                hair_style: event.form.hair_style,
                starting_job: event.form.starting_job as u16,
                sex: event.form.sex as u8,
            };

            if let Err(e) = client.create_character(packet) {
                error!("Failed to create character: {:?}", e);
            }
        } else {
            error!("CharServerClient not initialized - cannot create character");
        }
    }
}

pub fn handle_delete_character(
    mut events: EventReader<DeleteCharacterRequestEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for event in events.read() {
        if let Some(client) = char_client.as_deref_mut() {
            if let Err(e) = client.delete_character(event.character_id, String::new()) {
                error!("Failed to delete character: {:?}", e);
            }
        } else {
            error!("CharServerClient not initialized - cannot delete character");
        }
    }
}

pub fn handle_zone_server_info(
    mut events: EventReader<ZoneServerInfoReceivedEvent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        info!("Connecting to zone server: {}", event.map_name);
        game_state.set(GameState::Connecting);
    }
}

pub fn handle_zone_auth_success(
    mut events: EventReader<ZoneAuthenticationSuccess>,
    zone_client: Option<Res<crate::infrastructure::networking::ZoneServerClient>>,
    mut commands: Commands,
    mut map_loading_events: EventWriter<MapLoadingStarted>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        if let Some(client) = zone_client.as_ref() {
            if let Some(session) = &client.session_data {
                info!("Loading map: {}", session.map_name);

                commands.insert_resource(MapSpawnContext::new(
                    session.map_name.clone(),
                    event.spawn_x,
                    event.spawn_y,
                    session.character_id,
                ));

                map_loading_events.write(MapLoadingStarted {
                    map_name: session.map_name.clone(),
                });

                game_state.set(GameState::Loading);
            }
        }
    }
}

pub fn detect_map_load_complete(
    query: Query<&crate::domain::world::map::MapData, Added<crate::domain::world::map::MapData>>,
    spawn_context: Option<Res<MapSpawnContext>>,
    mut events: EventWriter<MapLoadCompleted>,
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
    mut events: EventReader<MapLoadCompleted>,
    mut zone_client: Option<ResMut<crate::infrastructure::networking::ZoneServerClient>>,
    mut actor_init_events: EventWriter<ActorInitSent>,
) {
    for event in events.read() {
        debug!(
            "Map '{}' loaded, sending CZ_NOTIFY_ACTORINIT",
            event.map_name
        );

        if let Some(client) = zone_client.as_deref_mut() {
            if let Err(e) = client.send_actor_init() {
                error!("Failed to send actor init: {:?}", e);
            } else {
                actor_init_events.write(ActorInitSent);
            }
        }
    }
}

pub fn handle_actor_init_sent(
    mut events: EventReader<ActorInitSent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for _event in events.read() {
        info!("Entering game world");
        // TODO: Spawn player character entity here in future
        game_state.set(GameState::InGame);
    }
}

pub fn handle_character_created(
    mut events: EventReader<CharacterCreatedEvent>,
    mut state: ResMut<CharacterSelectionState>,
    mut refresh_events: EventWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        state.is_creating_character = false;
        state.creation_slot = None;
        refresh_events.write(RefreshCharacterListEvent);
    }
}

pub fn handle_character_deleted(
    mut events: EventReader<CharacterDeletedEvent>,
    mut refresh_events: EventWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        refresh_events.write(RefreshCharacterListEvent);
    }
}

pub fn handle_refresh_character_list(
    mut events: EventReader<RefreshCharacterListEvent>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    for _event in events.read() {
        if let Some(client) = char_client.as_deref_mut() {
            if let Err(e) = client.request_charlist() {
                error!("Failed to request character list refresh: {:?}", e);
            }
        } else {
            warn!("CharServerClient not initialized - cannot refresh character list");
        }
    }
}

pub fn update_char_client(char_client: Option<ResMut<CharServerClient>>) {
    if let Some(mut client) = char_client {
        let _ = client.send_keepalive();
    }
}

/// System to start map loading timer when map loading begins
pub fn start_map_loading_timer(mut events: EventReader<MapLoadingStarted>, mut commands: Commands) {
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
    mut zone_client: Option<ResMut<crate::infrastructure::networking::ZoneServerClient>>,
    mut failed_events: EventWriter<MapLoadingFailed>,
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
    mut spawn_events: EventWriter<
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

    // Add PlayerCharacter marker to identify this as the player's character
    commands
        .entity(character_entity)
        .insert(crate::domain::camera::components::PlayerCharacter);

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
