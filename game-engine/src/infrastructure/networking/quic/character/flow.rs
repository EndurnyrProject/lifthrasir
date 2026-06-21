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
use super::{CharPhase, CharacterRoster, QuicCharState};
use crate::domain::character::events::{
    CreateCharacterRequestEvent, DeleteCharacterRequestEvent, RefreshCharacterListEvent,
    SelectCharacterEvent,
};
use crate::domain::character::forms::CharacterCreationForm;
use crate::infrastructure::networking::char_messages::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected, CharacterSlotInfoReceived, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::{
    CharListRefresh, CreateChar, DeleteCharRequest, Hello, SelectChar, SessionAuth,
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

/// Pure outcome of receiving a `CharList`: the next phase, and whether this is the
/// initial list (emit the UI events) or a `Ready` refresh (roster update only).
fn char_list_outcome(phase: CharPhase) -> CharPhase {
    match phase {
        CharPhase::AuthSent => CharPhase::Ready,
        other => other,
    }
}

/// On a fresh quinnet connection, send the `Hello` handshake on the control channel.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
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
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
#[allow(clippy::too_many_arguments)]
pub fn char_drain_control(
    mut incoming: MessageReader<IncomingMessage>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
    mut roster: ResMut<CharacterRoster>,
    mut connected: MessageWriter<CharacterServerConnected>,
    mut slot_info: MessageWriter<CharacterSlotInfoReceived>,
    mut zone_info: MessageWriter<ZoneServerInfoReceived>,
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
                });
                if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, auth) {
                    error!("failed to send SessionAuth: {e}");
                    state.phase = CharPhase::Failed;
                    continue;
                }
                state.phase = next;
            }
            Body::CharList(list) => {
                let emit = state.phase == CharPhase::AuthSent;
                roster.update_from_char_list(&list);
                if emit {
                    connected.write(char_list_to_connected(&list));
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
                zone_info.write(zone_server_info_to_event(z));
                state.phase = CharPhase::Done;
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
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
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

/// Maps a validated creation form onto the proto `CreateChar` request.
fn form_to_create_char(form: &CharacterCreationForm) -> CreateChar {
    CreateChar {
        name: form.name.clone(),
        slot: form.slot as u32,
        hair_color: form.hair_color as u32,
        hair_style: form.hair_style as u32,
        starting_job: form.starting_job as u32,
        sex: form.sex as u32,
    }
}

/// Sends `SelectChar` for a UI-selected slot while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_select(
    mut events: MessageReader<SelectCharacterEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::SelectChar(SelectChar {
            slot: ev.slot as u32,
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send SelectChar: {e}");
            continue;
        }
        state.phase = CharPhase::Selecting;
    }
}

/// Sends `CreateChar` for a UI creation request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_create(
    mut events: MessageReader<CreateCharacterRequestEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        if let Err(e) = ev.form.validate() {
            warn!("rejecting invalid character creation form: {e}");
            continue;
        }
        let body = Body::CreateChar(form_to_create_char(&ev.form));
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send CreateChar: {e}");
        }
    }
}

/// Sends `DeleteCharRequest` for a UI deletion request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_delete(
    mut events: MessageReader<DeleteCharacterRequestEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for ev in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::DeleteCharRequest(DeleteCharRequest {
            char_id: ev.character_id,
        });
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send DeleteCharRequest: {e}");
        }
    }
}

/// Sends `CharListRefresh` for a UI refresh request while the session is `Ready`.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update
)]
pub fn char_send_refresh(
    mut events: MessageReader<RefreshCharacterListEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicCharState>,
) {
    for _ in events.read() {
        if state.phase != CharPhase::Ready {
            continue;
        }
        let body = Body::CharListRefresh(CharListRefresh {});
        if let Err(e) = state.conn.send(client.connection_mut(), CONTROL, body) {
            error!("failed to send CharListRefresh: {e}");
        }
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

    #[test]
    fn form_to_create_char_maps_field_for_field() {
        use crate::domain::entities::character::components::Gender;

        let form = CharacterCreationForm {
            name: "Hero".into(),
            slot: 2,
            hair_style: 7,
            hair_color: 3,
            starting_job: 0,
            sex: Gender::Male,
            ..Default::default()
        };

        let req = form_to_create_char(&form);
        assert_eq!(req.name, "Hero");
        assert_eq!(req.slot, 2);
        assert_eq!(req.hair_style, 7);
        assert_eq!(req.hair_color, 3);
        assert_eq!(req.starting_job, 0);
        assert_eq!(req.sex, Gender::Male as u32);

        let female_form = CharacterCreationForm {
            sex: Gender::Female,
            ..Default::default()
        };
        assert_eq!(form_to_create_char(&female_form).sex, 0u32);
    }
}
