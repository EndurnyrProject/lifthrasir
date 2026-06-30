use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::snapshots::snapshot;
use crate::channels::SNAPSHOTS;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::SnapshotReceived;

/// Drains the unreliable snapshots channel for periodic full-state position snapshots.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_snapshots(
    mut incoming: MessageReader<IncomingMessage>,
    mut received: MessageWriter<SnapshotReceived>,
) {
    for msg in incoming.read() {
        if msg.channel != SNAPSHOTS {
            continue;
        }
        if let Body::Snapshot(s) = msg.body.clone() {
            debug!(
                "[snapshot] received from server: tick={} entities={}",
                s.server_tick,
                s.entities.len()
            );
            received.write(snapshot(s));
        }
    }
}
