use super::components::CharacterSelectionState;
use super::events::*;
use super::models::CharacterData;
use crate::core::state::GameState;
use crate::infrastructure::networking::protocols::ro_char::ChMakeCharPacket;
use crate::infrastructure::networking::{CharServerClient, CharServerEvent};
use bevy::prelude::*;

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
                let char_data = CharacterData::from(net_char.clone());
                let slot = net_char.char_num as usize;
                if slot < char_list.len() {
                    char_list[slot] = Some(char_data);
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

pub fn handle_char_server_events(
    mut char_events: EventReader<CharServerEvent>,
    mut list_events: EventWriter<CharacterListReceivedEvent>,
    mut zone_events: EventWriter<ZoneServerInfoReceivedEvent>,
    mut created_events: EventWriter<CharacterCreatedEvent>,
    mut deleted_events: EventWriter<CharacterDeletedEvent>,
    mut creation_failed_events: EventWriter<CharacterCreationFailedEvent>,
    mut deletion_failed_events: EventWriter<CharacterDeletionFailedEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in char_events.read() {
        match event {
            CharServerEvent::CharacterListReceived(characters) => {
                let mut char_list = vec![None; 15];

                for net_char in characters {
                    let char_data = CharacterData::from(net_char.clone());
                    let slot = net_char.char_num as usize;
                    if slot < char_list.len() {
                        char_list[slot] = Some(char_data);
                    }
                }

                list_events.write(CharacterListReceivedEvent {
                    characters: char_list,
                    max_slots: 9,
                    available_slots: 9,
                });

                next_state.set(GameState::CharacterSelection);
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
    mut char_client: Option<ResMut<CharServerClient>>,
    mut state: ResMut<CharacterSelectionState>,
) {
    for event in events.read() {
        state.selected_slot = Some(event.slot);

        if let Some(client) = char_client.as_deref_mut() {
            if let Err(e) = client.select_character(event.slot) {
                error!("Failed to select character: {:?}", e);
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
        // Validate form before sending
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
