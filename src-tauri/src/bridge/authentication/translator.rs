use bevy::prelude::*;
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::{LoginAttemptEvent, ServerSelectedEvent};

use crate::bridge::correlation::ServerCorrelation;
use crate::bridge::events::{LoginRequestedEvent, ServerSelectionRequestedEvent};

pub fn handle_login_request(
    mut events: MessageReader<LoginRequestedEvent>,
    mut login_events: MessageWriter<LoginAttemptEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing LoginRequestedEvent for username: '{}'",
            event.username
        );

        login_events.write(LoginAttemptEvent {
            username: event.username.clone(),
            password: event.password.clone(),
        });
    }
}

pub fn handle_server_selection_request(
    mut events: MessageReader<ServerSelectionRequestedEvent>,
    mut correlation: ResMut<ServerCorrelation>,
    session: Option<Res<UserSession>>,
    mut server_events: MessageWriter<ServerSelectedEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing ServerSelectionRequestedEvent for index: {}",
            event.server_index
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

                if let Some(sender) = correlation.remove(&event.server_index) {
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

            if let Some(sender) = correlation.remove(&event.server_index) {
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
