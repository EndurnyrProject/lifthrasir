use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{ComposeCardRequested, UseRequested};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{CardComposeRequest, UseItem};
use crate::zone::{QuicZoneState, ZonePhase};

fn use_item_body(c: &UseRequested) -> Body {
    Body::UseItem(UseItem { index: c.index })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_use_item_requests(
    mut events: MessageReader<UseRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, use_item_body(ev)) {
            error!("failed to send UseItem: {e}");
        }
    }
}

fn card_compose_body(c: &ComposeCardRequested) -> Body {
    Body::CardComposeRequest(CardComposeRequest {
        card_index: c.card_index,
        equipment_index: c.equipment_index,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_card_compose_requests(
    mut events: MessageReader<ComposeCardRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, card_compose_body(ev)) {
            error!("failed to send CardComposeRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_compose_body_preserves_both_indices() {
        let body = card_compose_body(&net_contract::commands::ComposeCardRequested {
            card_index: 9,
            equipment_index: 23,
        });
        match body {
            Body::CardComposeRequest(request) => {
                assert_eq!((request.card_index, request.equipment_index), (9, 23));
            }
            other => panic!("expected CardComposeRequest, got {other:?}"),
        }
    }

    #[test]
    fn use_item_body_carries_index() {
        let body = use_item_body(&UseRequested { index: 42 });
        match body {
            Body::UseItem(UseItem { index }) => assert_eq!(index, 42u32),
            other => panic!("expected Body::UseItem, got {other:?}"),
        }
    }
}
