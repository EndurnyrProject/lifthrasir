use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{client_connected, QuinnetClient};
use net_contract::commands::ChatSent;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::ChatRequest;
use crate::zone::{QuicZoneState, ZonePhase};

fn chat_body(c: &ChatSent) -> Body {
    Body::ChatRequest(ChatRequest {
        message: c.message.clone(),
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_chat_requests(
    mut events: MessageReader<ChatSent>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, chat_body(ev)) {
            error!("failed to send ChatRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_body_carries_message() {
        let body = chat_body(&ChatSent {
            message: "Hero : hello".to_string(),
        });
        match body {
            Body::ChatRequest(ChatRequest { message }) => assert_eq!(message, "Hero : hello"),
            other => panic!("expected Body::ChatRequest, got {other:?}"),
        }
    }
}
