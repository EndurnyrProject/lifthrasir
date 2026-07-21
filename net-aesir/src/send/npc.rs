use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{RespondToNpc, TalkToNpc};
use net_contract::dto::NpcResponse;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{NpcInteract, NpcTalk, npc_interact};
use crate::zone::{QuicZoneState, ZonePhase};

fn talk_body(t: &TalkToNpc) -> Body {
    Body::NpcTalk(NpcTalk { npc_id: t.npc_id })
}

fn interact_body(r: &RespondToNpc) -> Body {
    let response = match &r.response {
        NpcResponse::Continue => npc_interact::Response::Continue(true),
        NpcResponse::Choice(n) => npc_interact::Response::Choice(*n),
        NpcResponse::Number(v) => npc_interact::Response::Number(*v),
        NpcResponse::Input(s) => npc_interact::Response::Input(s.clone()),
        NpcResponse::Cancel => npc_interact::Response::Cancel(true),
    };
    Body::NpcInteract(NpcInteract {
        npc_id: r.npc_id,
        response: Some(response),
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_npc_talk(
    mut events: MessageReader<TalkToNpc>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, talk_body(ev)) {
            error!("failed to send NpcTalk: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_npc_interact(
    mut events: MessageReader<RespondToNpc>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, interact_body(ev)) {
            error!("failed to send NpcInteract: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talk_body_carries_npc_id() {
        let body = talk_body(&TalkToNpc { npc_id: 2_000_042 });
        match body {
            Body::NpcTalk(NpcTalk { npc_id }) => assert_eq!(npc_id, 2_000_042u32),
            other => panic!("expected Body::NpcTalk, got {other:?}"),
        }
    }

    #[test]
    fn interact_body_maps_continue() {
        let body = interact_body(&RespondToNpc {
            npc_id: 2_000_042,
            response: NpcResponse::Continue,
        });
        match body {
            Body::NpcInteract(NpcInteract { npc_id, response }) => {
                assert_eq!(npc_id, 2_000_042u32);
                assert_eq!(response, Some(npc_interact::Response::Continue(true)));
            }
            other => panic!("expected Body::NpcInteract, got {other:?}"),
        }
    }

    #[test]
    fn interact_body_maps_choice_passthrough_1_based() {
        let body = interact_body(&RespondToNpc {
            npc_id: 7,
            response: NpcResponse::Choice(3),
        });
        match body {
            Body::NpcInteract(NpcInteract { response, .. }) => {
                assert_eq!(response, Some(npc_interact::Response::Choice(3)));
            }
            other => panic!("expected Body::NpcInteract, got {other:?}"),
        }
    }

    #[test]
    fn interact_body_maps_number() {
        let body = interact_body(&RespondToNpc {
            npc_id: 7,
            response: NpcResponse::Number(-42),
        });
        match body {
            Body::NpcInteract(NpcInteract { response, .. }) => {
                assert_eq!(response, Some(npc_interact::Response::Number(-42)));
            }
            other => panic!("expected Body::NpcInteract, got {other:?}"),
        }
    }

    #[test]
    fn interact_body_maps_input() {
        let body = interact_body(&RespondToNpc {
            npc_id: 7,
            response: NpcResponse::Input("hello".to_string()),
        });
        match body {
            Body::NpcInteract(NpcInteract { response, .. }) => {
                assert_eq!(
                    response,
                    Some(npc_interact::Response::Input("hello".to_string()))
                );
            }
            other => panic!("expected Body::NpcInteract, got {other:?}"),
        }
    }

    #[test]
    fn interact_body_maps_cancel() {
        let body = interact_body(&RespondToNpc {
            npc_id: 7,
            response: NpcResponse::Cancel,
        });
        match body {
            Body::NpcInteract(NpcInteract { npc_id, response }) => {
                assert_eq!(npc_id, 7u32);
                assert_eq!(response, Some(npc_interact::Response::Cancel(true)));
            }
            other => panic!("expected Body::NpcInteract, got {other:?}"),
        }
    }
}
