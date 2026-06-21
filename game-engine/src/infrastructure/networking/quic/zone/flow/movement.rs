use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::movement::{move_stop, self_move};
use crate::infrastructure::networking::quic::channels::GAMEPLAY;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::{SelfMoved, UnitMoveStopped};

/// Drains the gameplay channel for own-character movement and move-stop bodies.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
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
