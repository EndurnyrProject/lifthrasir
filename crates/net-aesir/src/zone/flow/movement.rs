use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::movement::{move_stop, self_move};
use crate::channels::GAMEPLAY;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{SelfMoved, UnitMoveStopped};

/// Drains the gameplay channel for own-character movement and move-stop bodies.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_movement(
    mut incoming: MessageReader<IncomingMessage>,
    mut moved: MessageWriter<SelfMoved>,
    mut stopped: MessageWriter<UnitMoveStopped>,
) {
    for msg in incoming.read() {
        if msg.channel != GAMEPLAY {
            continue;
        }
        match msg.body.clone() {
            Body::SelfMove(m) => {
                moved.write(self_move(m));
            }
            Body::MoveStop(m) => {
                stopped.write(move_stop(m));
            }
            _ => {}
        }
    }
}
