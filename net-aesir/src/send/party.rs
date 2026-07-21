use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{
    PartyCreateRequested, PartyInviteRequested, PartyInviteResponded, PartyLeaveRequested,
};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    PartyCreateRequest, PartyInviteRequest, PartyInviteResponse, PartyLeaveRequest,
};
use crate::zone::{QuicZoneState, ZonePhase};

fn party_create_body(c: &PartyCreateRequested) -> Body {
    Body::PartyCreateRequest(PartyCreateRequest {
        name: c.name.clone(),
    })
}

fn party_invite_body(i: &PartyInviteRequested) -> Body {
    Body::PartyInviteRequest(PartyInviteRequest {
        target_char_id: i.target_char_id,
        target_name: i.target_name.clone(),
    })
}

fn party_invite_response_body(r: &PartyInviteResponded) -> Body {
    Body::PartyInviteResponse(PartyInviteResponse {
        party_id: r.party_id,
        accept: r.accept,
    })
}

fn party_leave_body(_: &PartyLeaveRequested) -> Body {
    Body::PartyLeaveRequest(PartyLeaveRequest {})
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_party_create(
    mut events: MessageReader<PartyCreateRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, party_create_body(ev)) {
            error!("failed to send PartyCreateRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_party_invite(
    mut events: MessageReader<PartyInviteRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, party_invite_body(ev)) {
            error!("failed to send PartyInviteRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_party_invite_response(
    mut events: MessageReader<PartyInviteResponded>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, party_invite_response_body(ev)) {
            error!("failed to send PartyInviteResponse: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_party_leave(
    mut events: MessageReader<PartyLeaveRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, party_leave_body(ev)) {
            error!("failed to send PartyLeaveRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_create_body_carries_name() {
        let body = party_create_body(&PartyCreateRequested {
            name: "Heroes".to_string(),
        });
        match body {
            Body::PartyCreateRequest(PartyCreateRequest { name }) => {
                assert_eq!(name, "Heroes")
            }
            other => panic!("expected Body::PartyCreateRequest, got {other:?}"),
        }
    }

    #[test]
    fn party_invite_body_maps_target_fields() {
        let body = party_invite_body(&PartyInviteRequested {
            target_char_id: 42,
            target_name: "Ally".to_string(),
        });
        match body {
            Body::PartyInviteRequest(PartyInviteRequest {
                target_char_id,
                target_name,
            }) => {
                assert_eq!(target_char_id, 42u32);
                assert_eq!(target_name, "Ally");
            }
            other => panic!("expected Body::PartyInviteRequest, got {other:?}"),
        }
    }

    #[test]
    fn party_invite_response_body_carries_party_id_and_accept() {
        let body = party_invite_response_body(&PartyInviteResponded {
            party_id: 7,
            accept: true,
        });
        match body {
            Body::PartyInviteResponse(PartyInviteResponse { party_id, accept }) => {
                assert_eq!(party_id, 7u32);
                assert!(accept);
            }
            other => panic!("expected Body::PartyInviteResponse, got {other:?}"),
        }
    }

    #[test]
    fn party_leave_body_is_empty() {
        let body = party_leave_body(&PartyLeaveRequested);
        match body {
            Body::PartyLeaveRequest(PartyLeaveRequest {}) => {}
            other => panic!("expected Body::PartyLeaveRequest, got {other:?}"),
        }
    }
}
