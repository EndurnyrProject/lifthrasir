use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::MoveRequested;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::MoveRequest;
use crate::zone::{QuicZoneState, ZonePhase};

/// Pure command -> wire body: the outbound analogue of a mapping fn.
fn move_body(m: &MoveRequested) -> Body {
    Body::MoveRequest(MoveRequest {
        dest_x: m.dest_x as u32,
        dest_y: m.dest_y as u32,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_move_requests(
    mut events: MessageReader<MoveRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, move_body(ev)) {
            error!("failed to send MoveRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_body_maps_dest_coords_to_proto() {
        let body = move_body(&MoveRequested {
            dest_x: 150,
            dest_y: 99,
        });
        match body {
            Body::MoveRequest(m) => {
                assert_eq!(m.dest_x, 150u32);
                assert_eq!(m.dest_y, 99u32);
            }
            other => panic!("expected MoveRequest, got {other:?}"),
        }
    }
}
