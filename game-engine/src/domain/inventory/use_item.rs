use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::infrastructure::networking::quic::channels::GAMEPLAY;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::UseItem;
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct UseItemRequested {
    pub index: u32,
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_use_item_send(
    mut events: MessageReader<UseItemRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }

    for event in events.read() {
        let body = Body::UseItem(UseItem { index: event.index });
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("Failed to send use-item request: {e}");
        }
    }
}
