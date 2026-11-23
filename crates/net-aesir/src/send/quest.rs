use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::QuestCommand;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net;
use crate::zone::{QuicZoneState, ZonePhase};

fn quest_body(command: &QuestCommand) -> Body {
    match *command {
        QuestCommand::SetActive { quest_id, active } => {
            Body::QuestActiveRequest(net::QuestActiveRequest { quest_id, active })
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_quest_commands(
    mut commands: MessageReader<QuestCommand>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }
    for command in commands.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, quest_body(command)) {
            error!("failed to send {command:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shelving_a_quest_asks_the_server_to_deactivate_it() {
        assert_eq!(
            quest_body(&QuestCommand::SetActive {
                quest_id: 7,
                active: false,
            }),
            Body::QuestActiveRequest(net::QuestActiveRequest {
                quest_id: 7,
                active: false,
            })
        );
    }
}
