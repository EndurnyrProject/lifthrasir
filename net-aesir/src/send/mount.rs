use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::MountPeco;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::MountRequest;
use crate::zone::{QuicZoneState, ZonePhase};

fn mount_body(c: &MountPeco) -> Body {
    Body::MountRequest(MountRequest { mount: c.mount })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_mount_peco(
    mut events: MessageReader<MountPeco>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, mount_body(ev)) {
            error!("failed to send MountRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_body_carries_mount_flag() {
        let body = mount_body(&MountPeco { mount: true });
        match body {
            Body::MountRequest(MountRequest { mount }) => assert!(mount),
            other => panic!("expected Body::MountRequest, got {other:?}"),
        }
    }
}
