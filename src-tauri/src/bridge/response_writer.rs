use crate::bridge::{pending_senders::PendingSenders, SessionData};
use bevy::prelude::*;
use game_engine::{
    domain::authentication::events::{LoginFailureEvent, LoginSuccessEvent},
    domain::character::{
        CharacterCreatedEvent, CharacterCreationFailedEvent, CharacterDeletedEvent,
        CharacterDeletionFailedEvent, CharacterListReceivedEvent, CharacterSelectedEvent,
    },
    infrastructure::networking::session::UserSession,
    presentation::ui::events::ServerSelectedEvent,
};

/// System to capture LoginSuccessEvent and send response through oneshot channel.
pub fn write_login_success_response(
    mut success_events: EventReader<LoginSuccessEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    for event in success_events.read() {
        // Find the sender for this username
        if let Some(sender) = pending.logins.senders.remove(&event.session.username) {
            let session_data = SessionData {
                username: event.session.username.clone(),
                login_id1: event.session.tokens.login_id1,
                account_id: event.session.tokens.account_id,
                login_id2: event.session.tokens.login_id2,
                sex: event.session.sex,
                servers: event.session.server_list.clone(),
            };

            // Send response through oneshot channel
            let _ = sender.send(Ok(session_data));
        }
    }
}

/// System to capture LoginFailureEvent and send response through oneshot channel.
pub fn write_login_failure_response(
    mut failure_events: EventReader<LoginFailureEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    for event in failure_events.read() {
        // Find the sender for this username
        if let Some(sender) = pending.logins.senders.remove(&event.username) {
            let error_msg = format!("{:?}", event.error);
            let _ = sender.send(Err(error_msg));
        }
    }
}

/// System to capture ServerSelectedEvent and send success response through oneshot channel.
pub fn write_server_selection_response(
    mut server_events: EventReader<ServerSelectedEvent>,
    mut pending: ResMut<PendingSenders>,
    session: Option<Res<UserSession>>,
) {
    for event in server_events.read() {
        if let Some(session) = session.as_ref() {
            if let Some(index) = session
                .server_list
                .iter()
                .position(|s| s.name == event.server.name)
            {
                if let Some(sender) = pending.servers.senders.remove(&index) {
                    let _ = sender.send(Ok(()));
                }
            }
        }
    }
}

/// System to capture CharacterListReceivedEvent and send response through oneshot channel.
pub fn write_character_list_response(
    mut list_events: EventReader<CharacterListReceivedEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    for event in list_events.read() {
        // Pop the first pending sender (FIFO)
        if let Some(sender) = pending.char_lists.senders.pop() {
            // Filter out None values from the character list
            let characters = event
                .characters
                .iter()
                .filter_map(|opt| opt.clone())
                .collect();

            let _ = sender.send(Ok(characters));
        }
    }
}

/// System to capture CharacterSelectedEvent and send response through oneshot channel.
pub fn write_character_selection_response(
    mut select_events: EventReader<CharacterSelectedEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    for event in select_events.read() {
        if let Some(sender) = pending.char_selections.senders.remove(&event.slot) {
            let _ = sender.send(Ok(()));
        }
    }
}

/// System to capture character creation events and send response through oneshot channel.
pub fn write_character_creation_response(
    mut create_events: EventReader<CharacterCreatedEvent>,
    mut create_fail_events: EventReader<CharacterCreationFailedEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    // Handle successful creations
    for event in create_events.read() {
        if let Some(sender) = pending.char_creations.senders.remove(&event.slot) {
            let _ = sender.send(Ok(event.character.clone()));
        }
    }

    // Handle failed creations
    for event in create_fail_events.read() {
        if let Some(sender) = pending.char_creations.senders.remove(&event.slot) {
            let _ = sender.send(Err(event.error.clone()));
        }
    }
}

/// System to capture character deletion events and send response through oneshot channel.
pub fn write_character_deletion_response(
    mut delete_events: EventReader<CharacterDeletedEvent>,
    mut delete_fail_events: EventReader<CharacterDeletionFailedEvent>,
    mut pending: ResMut<PendingSenders>,
) {
    // Handle successful deletions
    for event in delete_events.read() {
        if let Some(sender) = pending.char_deletions.senders.remove(&event.character_id) {
            let _ = sender.send(Ok(()));
        }
    }

    // Handle failed deletions
    for event in delete_fail_events.read() {
        if let Some(sender) = pending.char_deletions.senders.remove(&event.character_id) {
            let _ = sender.send(Err(event.error.clone()));
        }
    }
}
