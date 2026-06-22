use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::snapshots::snapshot;
use crate::infrastructure::networking::quic::channels::SNAPSHOTS;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::SnapshotReceived;

/// Drains the unreliable snapshots channel for periodic full-state position snapshots.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
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
