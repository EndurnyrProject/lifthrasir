use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::PickupRequested;

use crate::channels::GAMEPLAY;
use crate::zone::mapping::ground::pickup_body;
use crate::zone::{QuicZoneState, ZonePhase};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_pickup_requests(
    mut events: MessageReader<PickupRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, pickup_body(ev)) {
            error!("failed to send PickupItemRequest: {e}");
        }
    }
}
