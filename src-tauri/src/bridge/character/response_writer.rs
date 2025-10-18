use crate::bridge::correlation::CharacterCorrelation;
use crate::bridge::pending_senders::PendingSenders;
use bevy::prelude::*;
use game_engine::domain::character::events::{
    CharacterCreatedEvent, CharacterCreationFailedEvent, CharacterDeletedEvent,
    CharacterDeletionFailedEvent, CharacterListReceivedEvent, CharacterSelectedEvent,
};

/// System to capture CharacterListReceivedEvent and send response through oneshot channel
/// Character list has no correlation - we just send to the first pending request (FIFO)
pub fn write_character_list_response(
    mut list_events: EventReader<CharacterListReceivedEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    for event in list_events.read() {
        // Take the first pending sender (FIFO) - character list requests are typically sequential
        if let Some((request_id, _)) = pending.char_lists.senders.iter().next() {
            let request_id = *request_id;
            if let Some(sender) = pending.char_lists.senders.remove(&request_id) {
                // Filter out None values from the character list
                let characters = event
                    .characters
                    .iter()
                    .filter_map(|opt| opt.clone())
                    .collect();

                debug!("Sending character list response to UI");
                if sender.send(Ok(characters)).is_err() {
                    debug!("Failed to send character list response - receiver was dropped");
                }
            }
        }
    }
}

/// System to capture CharacterSelectedEvent and send response through oneshot channel
/// Uses correlation map to find the RequestId from slot
pub fn write_character_selection_response(
    mut select_events: EventReader<CharacterSelectedEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    for event in select_events.read() {
        // Use correlation to find request_id from slot
        if let Some(request_id) = correlation.remove_slot(&event.slot) {
            if let Some(sender) = pending.char_selections.senders.remove(&request_id) {
                debug!("Character selected from slot {}, sending response to UI", event.slot);
                if sender.send(Ok(())).is_err() {
                    debug!("Failed to send character selection response - receiver was dropped");
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (slot: {})",
                    request_id, event.slot
                );
            }
        } else {
            error!(
                "No correlation found for character selection slot: {}",
                event.slot
            );
        }
    }
}

/// System to capture character creation events and send response through oneshot channel
/// Uses correlation map to find the RequestId from slot
pub fn write_character_creation_response(
    mut create_events: EventReader<CharacterCreatedEvent>,
    mut create_fail_events: EventReader<CharacterCreationFailedEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    // Handle successful creations
    for event in create_events.read() {
        // Use correlation to find request_id from slot
        if let Some(request_id) = correlation.remove_slot(&event.slot) {
            if let Some(sender) = pending.char_creations.senders.remove(&request_id) {
                debug!("Character created in slot {}, sending response to UI", event.slot);
                if sender.send(Ok(event.character.clone())).is_err() {
                    debug!("Failed to send character creation response - receiver was dropped");
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (slot: {})",
                    request_id, event.slot
                );
            }
        } else {
            error!(
                "No correlation found for character creation slot: {}",
                event.slot
            );
        }
    }

    // Handle failed creations
    for event in create_fail_events.read() {
        // Use correlation to find request_id from slot
        if let Some(request_id) = correlation.remove_slot(&event.slot) {
            if let Some(sender) = pending.char_creations.senders.remove(&request_id) {
                debug!("Character creation failed in slot {}: {}", event.slot, event.error);
                if sender.send(Err(event.error.clone())).is_err() {
                    debug!("Failed to send character creation failure response - receiver was dropped");
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (slot: {})",
                    request_id, event.slot
                );
            }
        } else {
            error!(
                "No correlation found for character creation failure slot: {}",
                event.slot
            );
        }
    }
}

/// System to capture character deletion events and send response through oneshot channel
/// Uses correlation map to find the RequestId from char_id
pub fn write_character_deletion_response(
    mut delete_events: EventReader<CharacterDeletedEvent>,
    mut delete_fail_events: EventReader<CharacterDeletionFailedEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    // Handle successful deletions
    for event in delete_events.read() {
        // Use correlation to find request_id from char_id
        if let Some(request_id) = correlation.remove_char_id(&event.character_id) {
            if let Some(sender) = pending.char_deletions.senders.remove(&request_id) {
                debug!("Character {} deleted, sending response to UI", event.character_id);
                if sender.send(Ok(())).is_err() {
                    debug!("Failed to send character deletion response - receiver was dropped");
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (char_id: {})",
                    request_id, event.character_id
                );
            }
        } else {
            error!(
                "No correlation found for character deletion char_id: {}",
                event.character_id
            );
        }
    }

    // Handle failed deletions
    for event in delete_fail_events.read() {
        // Use correlation to find request_id from char_id
        if let Some(request_id) = correlation.remove_char_id(&event.character_id) {
            if let Some(sender) = pending.char_deletions.senders.remove(&request_id) {
                debug!("Character deletion failed for {}: {}", event.character_id, event.error);
                if sender.send(Err(event.error.clone())).is_err() {
                    debug!("Failed to send character deletion failure response - receiver was dropped");
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (char_id: {})",
                    request_id, event.character_id
                );
            }
        } else {
            error!(
                "No correlation found for character deletion failure char_id: {}",
                event.character_id
            );
        }
    }
}
