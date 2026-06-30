use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{client_connected, QuinnetClient};
use net_contract::commands::UseRequested;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::UseItem;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_item_body_carries_index() {
        let body = use_item_body(&UseRequested { index: 42 });
        match body {
            Body::UseItem(UseItem { index }) => assert_eq!(index, 42u32),
            other => panic!("expected Body::UseItem, got {other:?}"),
        }
    }
}
