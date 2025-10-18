use bevy::prelude::*;
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::{LoginAttemptEvent, ServerSelectedEvent};

use crate::bridge::events::{LoginRequestedEvent, ServerSelectionRequestedEvent};
use crate::bridge::pending_senders::PendingSenders;

/// System that handles LoginRequestedEvent and translates to game engine LoginAttemptEvent
/// Also stores the oneshot sender for response correlation
pub fn handle_login_request(
    mut events: EventReader<LoginRequestedEvent>,
    mut login_events: EventWriter<LoginAttemptEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing LoginRequestedEvent for username: '{}', request_id: {}",
            event.username, event.request_id
        );

        login_events.write(LoginAttemptEvent {
            username: event.username.clone(),
            password: event.password.clone(),
        });
    }
}

/// System that handles ServerSelectionRequestedEvent and translates to game engine ServerSelectedEvent
pub fn handle_server_selection_request(
    mut events: EventReader<ServerSelectionRequestedEvent>,
    mut pending: ResMut<PendingSenders>,
    session: Option<Res<UserSession>>,
    mut server_events: EventWriter<ServerSelectedEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing ServerSelectionRequestedEvent for index: {}, request_id: {}",
            event.server_index, event.request_id
        );

        if let Some(session) = session.as_ref() {
            if let Some(server) = session.server_list.get(event.server_index) {
                server_events.write(ServerSelectedEvent {
                    server: server.clone(),
                    server_index: Some(event.server_index),
                });
            } else {
                let error_msg = format!(
                    "Invalid server index: {}. Available servers: {}",
                    event.server_index,
                    session.server_list.len()
                );
                warn!("{}", error_msg);

                if let Some(sender) = pending.servers.senders.remove(&event.request_id) {
                    if sender.send(Err(error_msg.clone())).is_err() {
                        debug!(
                            "Failed to send invalid server index error - receiver was dropped: {}",
                            error_msg
                        );
                    }
                }
            }
        } else {
            let error_msg = "Server selection requested but no session available".to_string();
            warn!("{}", error_msg);

            if let Some(sender) = pending.servers.senders.remove(&event.request_id) {
                if sender.send(Err(error_msg.clone())).is_err() {
                    debug!(
                        "Failed to send no session error - receiver was dropped: {}",
                        error_msg
                    );
                }
            }
        }
    }
}
