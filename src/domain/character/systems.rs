use super::components::CharacterSelectionState;
use super::events::*;
use super::models::CharacterData;
use crate::core::state::GameState;
use crate::infrastructure::networking::protocols::ro_char::ChMakeCharPacket;
use crate::infrastructure::networking::{CharServerClient, CharServerEvent};
use bevy::prelude::*;

pub fn handle_char_server_events(
    mut char_events: EventReader<CharServerEvent>,
    mut list_events: EventWriter<CharacterListReceivedEvent>,
    mut zone_events: EventWriter<ZoneServerInfoReceivedEvent>,
    mut created_events: EventWriter<CharacterCreatedEvent>,
    mut deleted_events: EventWriter<CharacterDeletedEvent>,
    mut creation_failed_events: EventWriter<CharacterCreationFailedEvent>,
    mut deletion_failed_events: EventWriter<CharacterDeletionFailedEvent>,
) {
    for event in char_events.read() {
        match event {
            CharServerEvent::CharacterListReceived(characters) => {
                info!(
                    "Processing character list with {} characters",
                    characters.len()
                );

                // Convert network characters to domain model
                let mut char_list = vec![None; 15]; // Support up to 15 slots

                for net_char in characters {
                    let char_data = CharacterData::from(net_char.clone());
                    let slot = net_char.char_num as usize;
                    if slot < char_list.len() {
                        char_list[slot] = Some(char_data);
                    }
                }

                list_events.write(CharacterListReceivedEvent {
                    characters: char_list,
                    max_slots: 9, // Default to 9 visible slots
                    available_slots: 9,
                });
            }
            CharServerEvent::ZoneServerInfo {
                char_id,
                map_name,
                ip,
                port,
            } => {
                let server_ip = format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
                zone_events.write(ZoneServerInfoReceivedEvent {
                    char_id: *char_id,
                    map_name: map_name.clone(),
                    server_ip,
                    server_port: *port,
                });
            }
            CharServerEvent::CharacterCreated(char_info) => {
                let char_data = CharacterData::from(char_info.clone());
                created_events.write(CharacterCreatedEvent {
                    character: char_data,
                    slot: char_info.char_num,
                });
            }
            CharServerEvent::CharacterDeleted => {
                // We'll need to track which character was deleted
                deleted_events.write(CharacterDeletedEvent { character_id: 0 });
            }
            CharServerEvent::CharacterCreationFailed(error_code) => {
                let error_msg = match error_code {
                    0x00 => "Character name already exists",
                    0x01 => "Invalid character name",
                    0x02 => "Character slot is full",
                    0x03 => "Character creation denied",
                    _ => "Unknown error",
                };
                creation_failed_events.write(CharacterCreationFailedEvent {
                    error: super::models::CharacterCreationError::ServerError(
                        error_msg.to_string(),
                    ),
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
            CharServerEvent::ConnectionError(error) => {
                error!("Character server connection error: {:?}", error);
            }
            CharServerEvent::CharacterSlotInfo {
                normal_slots,
                premium_slots,
                valid_slots,
            } => {
                info!(
                    "Character slots configured - normal: {}, premium: {}, valid: {}",
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
        }
    }
}

pub fn handle_select_character(
    mut events: EventReader<SelectCharacterEvent>,
    mut char_client: ResMut<CharServerClient>,
    mut state: ResMut<CharacterSelectionState>,
) {
    for event in events.read() {
        info!("Selecting character in slot {}", event.slot);
        state.selected_slot = Some(event.slot);

        if let Err(e) = char_client.select_character(event.slot) {
            error!("Failed to select character: {:?}", e);
        }
    }
}

pub fn handle_create_character(
    mut events: EventReader<CreateCharacterRequestEvent>,
    mut char_client: ResMut<CharServerClient>,
) {
    for event in events.read() {
        // Validate form before sending
        if let Err(e) = event.form.validate() {
            error!("Character creation validation failed: {:?}", e);
            continue;
        }

        let packet = ChMakeCharPacket {
            name: event.form.name.clone(),
            str: event.form.str,
            agi: event.form.agi,
            vit: event.form.vit,
            int: event.form.int,
            dex: event.form.dex,
            luk: event.form.luk,
            char_num: event.form.slot,
            hair_color: event.form.hair_color,
            hair_style: event.form.hair_style,
            starting_job: event.form.starting_job as u16,
            sex: event.form.sex as u8,
        };

        if let Err(e) = char_client.create_character(packet) {
            error!("Failed to create character: {:?}", e);
        }
    }
}

pub fn handle_delete_character(
    mut events: EventReader<DeleteCharacterRequestEvent>,
    mut char_client: ResMut<CharServerClient>,
) {
    for event in events.read() {
        info!("Deleting character with ID {}", event.character_id);

        if let Err(e) = char_client.delete_character(event.character_id, event.email.clone()) {
            error!("Failed to delete character: {:?}", e);
        }
    }
}

pub fn handle_zone_server_info(
    mut events: EventReader<ZoneServerInfoReceivedEvent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        info!(
            "Received zone server info - Map: {}, Server: {}:{}",
            event.map_name, event.server_ip, event.server_port
        );

        // Transition to InGame state
        game_state.set(GameState::InGame);

        // TODO: Connect to zone server with the provided info
    }
}

pub fn handle_character_created(
    mut events: EventReader<CharacterCreatedEvent>,
    mut refresh_events: EventWriter<RefreshCharacterListEvent>,
    mut state: ResMut<CharacterSelectionState>,
) {
    for event in events.read() {
        info!(
            "Character '{}' created in slot {}",
            event.character.name, event.slot
        );

        // Exit creation mode
        state.is_creating_character = false;
        state.creation_slot = None;

        // Refresh character list
        refresh_events.write(RefreshCharacterListEvent);
    }
}

pub fn handle_character_deleted(
    mut events: EventReader<CharacterDeletedEvent>,
    mut refresh_events: EventWriter<RefreshCharacterListEvent>,
) {
    for _event in events.read() {
        info!("Character deleted successfully");

        // Refresh character list
        refresh_events.write(RefreshCharacterListEvent);
    }
}

pub fn update_char_client(char_client: Option<ResMut<CharServerClient>>) {
    if let Some(mut client) = char_client {
        // Client update is handled by char_client_update_system in char_client.rs
        // This is just a placeholder for any additional update logic
        let _ = client.send_keepalive();
    }
}
