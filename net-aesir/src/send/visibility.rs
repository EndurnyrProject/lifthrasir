use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{client_connected, QuinnetClient};
use net_contract::commands::NameRequested;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::NameRequest;
use crate::zone::{QuicZoneState, ZonePhase};

/// Pure command -> wire body: the outbound analogue of a mapping fn.
fn name_request_body(n: &NameRequested) -> Body {
    Body::NameRequest(NameRequest { entity_id: n.gid })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_name_requests(
    mut events: MessageReader<NameRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, name_request_body(ev)) {
            error!("failed to send NameRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_request_body_maps_gid_to_entity_id() {
        let body = name_request_body(&NameRequested { gid: 2_000_042 });
        match body {
            Body::NameRequest(n) => assert_eq!(n.entity_id, 2_000_042u32),
            other => panic!("expected Body::NameRequest, got {other:?}"),
        }
    }
}
