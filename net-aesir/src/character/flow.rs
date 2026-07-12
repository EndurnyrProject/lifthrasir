use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use bevy_quinnet::client::connection::{
    ConnectionEvent, ConnectionFailedEvent, ConnectionLostEvent,
};
use bevy_quinnet::client::QuinnetClient;

use super::mapping::{
    char_create_failed, char_created, char_list_to_connected, char_list_to_slot_info, delete_ack,
    zone_server_info_to_event,
};
use super::{CharPhase, QuicCharState};
use crate::channels::CONTROL;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use crate::proto::aesir::net::{Hello, SessionAuth};
use net_contract::events::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected, CharacterSlotInfoReceived, ZoneDisconnected, ZoneServerInfoReceived,
};

/// Pure outcome of receiving a `HelloAck`: whether to send `SessionAuth` and the next phase.
fn hello_ack_outcome(phase: CharPhase, accepted: bool) -> Option<CharPhase> {
    if phase != CharPhase::HelloSent {
        return None;
    }
    Some(if accepted {
        CharPhase::AuthSent
    } else {
        CharPhase::Failed
    })
}

/// Pure outcome of receiving a `CharList`: the next phase. The first list (in
/// `AuthSent`) advances to `Ready`; later refresh lists stay in their phase.
fn char_list_outcome(phase: CharPhase) -> CharPhase {
    match phase {
        CharPhase::AuthSent => CharPhase::Ready,
        other => other,
    }
}

/// On a fresh quinnet connection, send the `Hello` handshake on the control channel.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update
)]
pub fn char_send_hello(
    mut events: MessageReader<ConnectionEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for _ in events.read() {
        if state.phase != CharPhase::Connecting {
            continue;
        }
        let hello = Body::Hello(Hello {
            protocol_version: 1,
            build: "lifthrasir".into(),
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, hello) {
            error!("failed to send char Hello: {e}");
            state.phase = CharPhase::Failed;
            continue;
        }
        state.phase = CharPhase::HelloSent;
    }
}

/// Drains the control channel and advances the char-server session.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
#[allow(clippy::too_many_arguments)]
pub fn char_drain_control(
    mut incoming: MessageReader<IncomingMessage>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
    mut connected: MessageWriter<CharacterServerConnected>,
    mut slot_info: MessageWriter<CharacterSlotInfoReceived>,
    mut zone_info: MessageWriter<ZoneServerInfoReceived>,
    mut zone_disconnected: MessageWriter<ZoneDisconnected>,
    mut created: MessageWriter<CharacterCreated>,
    mut create_failed: MessageWriter<CharacterCreationFailed>,
    mut deleted: MessageWriter<CharacterDeleted>,
    mut deletion_failed: MessageWriter<CharacterDeletionFailed>,
) {
    // The connection is reused for the zone hop, so this drainer keeps seeing
    // control traffic it doesn't own once char selection is done. Bail to avoid
    // warning on the zone server's control messages.
    if matches!(state.phase, CharPhase::Done | CharPhase::Failed) {
        return;
    }
    for msg in incoming.read() {
        if msg.channel != CONTROL {
            continue;
        }
        match msg.body.clone() {
            Body::HelloAck(ack) => {
                let Some(next) = hello_ack_outcome(state.phase, ack.accepted) else {
                    continue;
                };
                if next == CharPhase::Failed {
                    warn!("char server rejected Hello handshake");
                    state.phase = CharPhase::Failed;
                    continue;
                }
                let auth = Body::SessionAuth(SessionAuth {
                    account_id: state.auth.account_id,
                    login_id1: state.auth.login_id1,
                    login_id2: state.auth.login_id2,
                    sex: state.auth.sex,
                    char_id: 0,
                    // Unused on the char-server entry path (see proto comment).
                    zone_auth_token: Vec::new(),
                });
                if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, auth) {
                    error!("failed to send SessionAuth: {e}");
                    state.phase = CharPhase::Failed;
                    continue;
                }
                state.phase = next;
            }
            Body::CharList(list) => {
                // Emit the roster on every list (initial + create/delete refreshes) so
                // the domain rebuilds its char-select view; slot info only changes on the
                // initial list, so keep that initial-only.
                let initial = state.phase == CharPhase::AuthSent;
                connected.write(char_list_to_connected(&list));
                if initial {
                    slot_info.write(char_list_to_slot_info(&list));
                }
                state.phase = char_list_outcome(state.phase);
            }
            Body::CharAuthFailed(_) => {
                error!("char session auth failed: stale or expired session");
                state.phase = CharPhase::Failed;
            }
            Body::ZoneServerInfo(z) => {
                if !matches!(state.phase, CharPhase::Ready | CharPhase::Selecting) {
                    warn!("unexpected ZoneServerInfo in phase {:?}", state.phase);
                    continue;
                }
                match zone_server_info_to_event(z) {
                    Ok(event) => {
                        zone_info.write(event);
                        state.phase = CharPhase::Done;
                    }
                    Err(reason) => {
                        error!("invalid zone server address: {reason}");
                        zone_disconnected.write(ZoneDisconnected { reason });
                        state.phase = CharPhase::Failed;
                    }
                }
            }
            Body::CharCreated(c) => match char_created(c) {
                Some(ev) => {
                    created.write(ev);
                }
                None => error!("aesir sent CharCreated with no character"),
            },
            Body::CharCreateFailed(f) => {
                create_failed.write(char_create_failed(f));
            }
            Body::DeleteCharAck(a) => match delete_ack(a) {
                Ok(ev) => {
                    deleted.write(ev);
                }
                Err(ev) => {
                    deletion_failed.write(ev);
                }
            },
            _ => warn!("unexpected control body on char channel"),
        }
    }
}

/// Maps quinnet connection failure / loss onto a failed char session.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update
)]
pub fn char_handle_connection_lost(
    mut failed_events: MessageReader<ConnectionFailedEvent>,
    mut lost_events: MessageReader<ConnectionLostEvent>,
    mut state: ResMut<QuicCharState>,
) {
    let fail = |state: &mut QuicCharState, message: &str| {
        if state.phase == CharPhase::Done || state.phase == CharPhase::Disconnected {
            return;
        }
        error!("char connection lost: {message}");
        state.phase = CharPhase::Failed;
    };

    for event in failed_events.read() {
        fail(&mut state, &format!("connection failed: {}", event.err));
    }
    for _ in lost_events.read() {
        fail(&mut state, "connection lost");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_ack_accepted_in_hello_sent_advances_to_auth_sent() {
        assert_eq!(
            hello_ack_outcome(CharPhase::HelloSent, true),
            Some(CharPhase::AuthSent)
        );
    }

    #[test]
    fn hello_ack_rejected_in_hello_sent_fails() {
        assert_eq!(
            hello_ack_outcome(CharPhase::HelloSent, false),
            Some(CharPhase::Failed)
        );
    }

    #[test]
    fn hello_ack_out_of_phase_is_ignored() {
        assert_eq!(hello_ack_outcome(CharPhase::Ready, true), None);
        assert_eq!(hello_ack_outcome(CharPhase::AuthSent, true), None);
    }

    #[test]
    fn char_list_in_auth_sent_advances_to_ready() {
        assert_eq!(char_list_outcome(CharPhase::AuthSent), CharPhase::Ready);
    }

    #[test]
    fn char_list_in_ready_stays_ready() {
        assert_eq!(char_list_outcome(CharPhase::Ready), CharPhase::Ready);
    }
}
