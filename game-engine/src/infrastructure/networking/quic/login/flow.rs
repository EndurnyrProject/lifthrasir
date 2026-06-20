use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use bevy_quinnet::client::connection::{
    ConnectionEvent, ConnectionFailedEvent, ConnectionLostEvent,
};
use bevy_quinnet::client::QuinnetClient;

use super::mapping::{login_failed_to_refused, login_response_to_accepted};
use super::{LoginPhase, QuicLoginState};
use crate::infrastructure::networking::messages::{LoginAccepted, LoginRefused};
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::connection::QuicConnection;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::{Hello, LoginRequest};

/// On a fresh quinnet connection, send the `Hello` handshake on the control channel.
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn quic_send_hello(
    mut events: MessageReader<ConnectionEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicLoginState>,
) {
    for _ in events.read() {
        if state.phase != LoginPhase::Connecting {
            continue;
        }
        let hello = Body::Hello(Hello {
            protocol_version: 1,
            build: state.pending.build.clone(),
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, hello) {
            error!("failed to send Hello: {e}");
            state.phase = LoginPhase::Failed;
            continue;
        }
        state.phase = LoginPhase::HelloSent;
    }
}

/// Drains the control channel and advances the login handshake.
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn quic_drain_control(
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicLoginState>,
    mut accepted: MessageWriter<LoginAccepted>,
    mut refused: MessageWriter<LoginRefused>,
) {
    for (channel, body) in QuicConnection::drain(client.connection_mut()) {
        if channel != CONTROL {
            continue;
        }
        match body {
            Body::HelloAck(ack) => {
                if state.phase != LoginPhase::HelloSent {
                    continue;
                }
                if !ack.accepted {
                    warn!("server rejected Hello handshake");
                    refused.write(LoginRefused {
                        username: state.pending.username.clone(),
                        error_code: 3,
                        error_message: "server rejected handshake".to_string(),
                        block_date: None,
                    });
                    state.phase = LoginPhase::Failed;
                    continue;
                }
                state.phase = LoginPhase::Ready;
                let request = Body::LoginRequest(LoginRequest {
                    username: state.pending.username.clone(),
                    password: state.pending.password.clone(),
                    client_version: state.pending.client_version,
                });
                if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, request) {
                    error!("failed to send LoginRequest: {e}");
                    state.phase = LoginPhase::Failed;
                    continue;
                }
                state.phase = LoginPhase::LoginSent;
            }
            Body::LoginResponse(resp) => {
                if state.phase != LoginPhase::LoginSent {
                    continue;
                }
                accepted.write(login_response_to_accepted(
                    resp,
                    state.pending.username.clone(),
                ));
                state.phase = LoginPhase::Done;
            }
            Body::LoginFailed(failed) => {
                if state.phase != LoginPhase::LoginSent {
                    continue;
                }
                refused.write(login_failed_to_refused(
                    failed,
                    state.pending.username.clone(),
                ));
                state.phase = LoginPhase::Failed;
            }
            _ => warn!("unexpected control body on login channel"),
        }
    }
}

/// Maps quinnet connection failure / loss onto a `LoginRefused`.
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn quic_handle_connection_lost(
    mut failed_events: MessageReader<ConnectionFailedEvent>,
    mut lost_events: MessageReader<ConnectionLostEvent>,
    mut state: ResMut<QuicLoginState>,
    mut refused: MessageWriter<LoginRefused>,
) {
    let mut fail = |state: &mut QuicLoginState, message: String| {
        if state.phase == LoginPhase::Done || state.phase == LoginPhase::Disconnected {
            return;
        }
        refused.write(LoginRefused {
            username: state.pending.username.clone(),
            error_code: 3,
            error_message: message,
            block_date: None,
        });
        state.phase = LoginPhase::Failed;
    };

    for event in failed_events.read() {
        fail(&mut state, format!("connection failed: {}", event.err));
    }
    for _ in lost_events.read() {
        fail(&mut state, "connection lost".to_string());
    }
}
