use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::warp::map_move;
use crate::channels::CONTROL;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::MapChangeRequested;

/// Drains the control channel for server-commanded map changes (warps).
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
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
