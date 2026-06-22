use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::warp::map_move;
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::MapChangeRequested;

/// Drains the control channel for server-commanded map changes (warps).
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_warp(
    mut incoming: MessageReader<IncomingMessage>,
    mut writer: MessageWriter<MapChangeRequested>,
) {
    for msg in incoming.read() {
        if msg.channel != CONTROL {
            continue;
        }
        if let Body::MapMove(m) = msg.body.clone() {
            writer.write(map_move(m));
        }
    }
}
