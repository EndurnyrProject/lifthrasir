use crate::bridge::correlation::{LoginCorrelation, ServerCorrelation};
use crate::bridge::pending_senders::PendingSenders;
use crate::bridge::SessionData;
use bevy::prelude::*;
use game_engine::domain::authentication::events::{LoginFailureEvent, LoginSuccessEvent};
use game_engine::presentation::ui::events::ServerSelectedEvent;

/// System to capture LoginSuccessEvent and send response through oneshot channel
/// Uses correlation map to find the RequestId from username
pub fn write_login_success_response(
    mut success_events: MessageReader<LoginSuccessEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<LoginCorrelation>,
) {
    for event in success_events.read() {
        let username = &event.session.username;

        if let Some(request_id) = correlation.remove(username) {
            if let Some(sender) = pending.logins.senders.remove(&request_id) {
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
                    "No pending sender found for request_id: {} (username: '{}')",
                    request_id, username
                );
            }
        } else {
            error!(
                "No correlation found for username: '{}' - this should not happen",
                username
            );
        }
    }
}

/// System to capture LoginFailureEvent and send response through oneshot channel
pub fn write_login_failure_response(
    mut failure_events: MessageReader<LoginFailureEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<LoginCorrelation>,
) {
    for event in failure_events.read() {
        let request_id = if event.username.is_empty() {
            warn!("Received LoginFailureEvent with empty username - cannot correlate to request");
            None
        } else {
            // Normal path: use correlation
            correlation.remove(&event.username)
        };

        if let Some(request_id) = request_id {
            if let Some(sender) = pending.logins.senders.remove(&request_id) {
                let error_msg = format!("{:?}", event.error);
                debug!("Login failed: {}", error_msg);

                if sender.send(Err(error_msg.clone())).is_err() {
                    debug!(
                        "Failed to send login failure response - receiver was dropped: {}",
                        error_msg
                    );
                }
            } else {
                error!(
                    "No pending sender found for request_id: {} (username: '{}')",
                    request_id, event.username
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

/// System to capture ServerSelectedEvent and send success response through oneshot channel
/// Uses correlation map to find the RequestId from server_index
pub fn write_server_selection_response(
    mut server_events: MessageReader<ServerSelectedEvent>,
    mut pending: ResMut<PendingSenders>,
    mut correlation: ResMut<ServerCorrelation>,
) {
    for event in server_events.read() {
        if let Some(server_index) = event.server_index {
            // Use correlation to find request_id from server_index
            if let Some(request_id) = correlation.remove(&server_index) {
                if let Some(sender) = pending.servers.senders.remove(&request_id) {
                    debug!(
                        "Server selection succeeded for index {}, sending response to UI",
                        server_index
                    );
                    if sender.send(Ok(())).is_err() {
                        debug!("Failed to send server selection response - receiver was dropped");
                    }
                } else {
                    error!(
                        "No pending sender found for request_id: {} (server_index: {})",
                        request_id, server_index
                    );
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
