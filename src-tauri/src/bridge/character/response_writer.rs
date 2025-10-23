use crate::bridge::correlation::{CharacterCorrelation, PendingCharacterListSenders};
use bevy::prelude::*;
use game_engine::domain::character::events::{
    CharacterCreatedEvent, CharacterCreationFailedEvent, CharacterDeletedEvent,
    CharacterDeletionFailedEvent, CharacterListReceivedEvent, CharacterSelectedEvent,
};

pub fn write_character_list_response(
    mut list_events: MessageReader<CharacterListReceivedEvent>,
    mut char_list_senders: ResMut<PendingCharacterListSenders>,
) {
    for event in list_events.read() {
        if let Some(sender) = char_list_senders.pop_oldest() {
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

pub fn write_character_selection_response(
    mut select_events: MessageReader<CharacterSelectedEvent>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    for event in select_events.read() {
        if let Some(sender) = correlation.remove_selection(&event.slot) {
            debug!(
                "Character selected from slot {}, sending response to UI",
                event.slot
            );
            if sender.send(Ok(())).is_err() {
                debug!("Failed to send character selection response - receiver was dropped");
            }
        } else {
            error!(
                "No correlation found for character selection slot: {}",
                event.slot
            );
        }
    }
}

pub fn write_character_creation_response(
    mut create_events: MessageReader<CharacterCreatedEvent>,
    mut create_fail_events: MessageReader<CharacterCreationFailedEvent>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    for event in create_events.read() {
        if let Some(sender) = correlation.remove_creation(&event.slot) {
            debug!(
                "Character created in slot {}, sending response to UI",
                event.slot
            );
            if sender.send(Ok(event.character.clone())).is_err() {
                debug!("Failed to send character creation response - receiver was dropped");
            }
        } else {
            error!(
                "No correlation found for character creation slot: {}",
                event.slot
            );
        }
    }

    for event in create_fail_events.read() {
        if let Some(sender) = correlation.remove_creation(&event.slot) {
            debug!(
                "Character creation failed in slot {}: {}",
                event.slot, event.error
            );
            if sender.send(Err(event.error.clone())).is_err() {
                debug!("Failed to send character creation failure response - receiver was dropped");
            }
        } else {
            error!(
                "No correlation found for character creation failure slot: {}",
                event.slot
            );
        }
    }
}

pub fn write_character_deletion_response(
    mut delete_events: MessageReader<CharacterDeletedEvent>,
    mut delete_fail_events: MessageReader<CharacterDeletionFailedEvent>,
    mut correlation: ResMut<CharacterCorrelation>,
) {
    for event in delete_events.read() {
        if let Some(sender) = correlation.remove_deletion(&event.character_id) {
            debug!(
                "Character {} deleted, sending response to UI",
                event.character_id
            );
            if sender.send(Ok(())).is_err() {
                debug!("Failed to send character deletion response - receiver was dropped");
            }
        } else {
            error!(
                "No correlation found for character deletion char_id: {}",
                event.character_id
            );
        }
    }

    for event in delete_fail_events.read() {
        if let Some(sender) = correlation.remove_deletion(&event.character_id) {
            debug!(
                "Character deletion failed for {}: {}",
                event.character_id, event.error
            );
            if sender.send(Err(event.error.clone())).is_err() {
                debug!("Failed to send character deletion failure response - receiver was dropped");
            }
        } else {
            error!(
                "No correlation found for character deletion failure char_id: {}",
                event.character_id
            );
        }
    }
}
