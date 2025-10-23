use crate::bridge::correlation::{LoginCorrelation, ServerCorrelation};
use crate::bridge::SessionData;
use bevy::prelude::*;
use game_engine::domain::authentication::events::{LoginFailureEvent, LoginSuccessEvent};
use game_engine::presentation::ui::events::ServerSelectedEvent;

pub fn write_login_success_response(
    mut success_events: MessageReader<LoginSuccessEvent>,
    mut correlation: ResMut<LoginCorrelation>,
) {
    for event in success_events.read() {
        let username = &event.session.username;

        if let Some(sender) = correlation.remove(username) {
            let session_data = SessionData {
                username: event.session.username.clone(),
                login_id1: event.session.tokens.login_id1,
                account_id: event.session.tokens.account_id,
                login_id2: event.session.tokens.login_id2,
                sex: event.session.sex,
                servers: event.session.server_list.clone(),
            };

            debug!(
                "Login success for account_id: {}, sending response to UI",
                event.session.tokens.account_id
            );

            match sender.send(Ok(session_data)) {
                Ok(_) => debug!("Successfully sent session data to UI"),
                Err(_) => warn!("Failed to send session data - receiver dropped"),
            }
        } else {
            error!(
                "No correlation found for username: '{}' - this should not happen",
                username
            );
        }
    }
}

pub fn write_login_failure_response(
    mut failure_events: MessageReader<LoginFailureEvent>,
    mut correlation: ResMut<LoginCorrelation>,
) {
    for event in failure_events.read() {
        let sender = if event.username.is_empty() {
            warn!("Received LoginFailureEvent with empty username - cannot correlate to request");
            None
        } else {
            correlation.remove(&event.username)
        };

        if let Some(sender) = sender {
            let error_msg = format!("{:?}", event.error);
            debug!("Login failed: {}", error_msg);

            if sender.send(Err(error_msg.clone())).is_err() {
                debug!(
                    "Failed to send login failure response - receiver was dropped: {}",
                    error_msg
                );
            }
        } else if !event.username.is_empty() {
            error!(
                "No correlation found for login failure (username: '{}')",
                event.username
            );
        }
    }
}

pub fn write_server_selection_response(
    mut server_events: MessageReader<ServerSelectedEvent>,
    mut correlation: ResMut<ServerCorrelation>,
) {
    for event in server_events.read() {
        if let Some(server_index) = event.server_index {
            if let Some(sender) = correlation.remove(&server_index) {
                debug!(
                    "Server selection succeeded for index {}, sending response to UI",
                    server_index
                );
                if sender.send(Ok(())).is_err() {
                    debug!("Failed to send server selection response - receiver was dropped");
                }
            } else {
                error!(
                    "No correlation found for server_index: {} - this should not happen",
                    server_index
                );
            }
        } else {
            warn!(
                "ServerSelectedEvent received without server_index - cannot correlate to request"
            );
        }
    }
}
