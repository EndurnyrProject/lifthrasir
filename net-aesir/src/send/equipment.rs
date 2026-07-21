use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{EquipRequested, UnequipRequested};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{EquipItem, UnequipItem};
use crate::zone::{QuicZoneState, ZonePhase};

fn equip_body(c: &EquipRequested) -> Body {
    Body::EquipItem(EquipItem {
        index: c.index as u32,
        position: c.location,
    })
}

fn unequip_body(c: &UnequipRequested) -> Body {
    Body::UnequipItem(UnequipItem {
        index: c.index as u32,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_equip_requests(
    mut events: MessageReader<EquipRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, equip_body(ev)) {
            error!("failed to send EquipItem: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_unequip_requests(
    mut events: MessageReader<UnequipRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, unequip_body(ev)) {
            error!("failed to send UnequipItem: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equip_body_carries_index_and_location() {
        let body = equip_body(&EquipRequested {
            index: 7,
            location: 0x0100,
        });
        match body {
            Body::EquipItem(EquipItem { index, position }) => {
                assert_eq!(index, 7u32);
                assert_eq!(position, 0x0100u32);
            }
            other => panic!("expected Body::EquipItem, got {other:?}"),
        }
    }
}
