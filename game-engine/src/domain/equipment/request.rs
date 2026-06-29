use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::domain::inventory::{Inventory, Item};
use crate::infrastructure::networking::quic::channels::GAMEPLAY;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::{EquipItem, UnequipItem};
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct EquipItemRequested {
    pub index: u16,
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct UnequipItemRequested {
    pub index: u16,
}

fn equip_body(item: &Item) -> Body {
    Body::EquipItem(EquipItem {
        index: item.index as u32,
        position: item.location,
    })
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_equip_item_send(
    mut events: MessageReader<EquipItemRequested>,
    inventory: Res<Inventory>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }

    for event in events.read() {
        let Some(item) = inventory.get(event.index) else {
            warn!(
                "Equip requested for unknown inventory index {}",
                event.index
            );
            continue;
        };

        if let Err(e) = zone.send(&mut client, GAMEPLAY, equip_body(item)) {
            error!("Failed to send equip-item request: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_unequip_item_send(
    mut events: MessageReader<UnequipItemRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }

    for event in events.read() {
        let body = Body::UnequipItem(UnequipItem {
            index: event.index as u32,
        });
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("Failed to send unequip-item request: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equip_body_carries_index_and_location() {
        let item = Item {
            index: 7,
            location: 0x0100,
            ..Default::default()
        };

        match equip_body(&item) {
            Body::EquipItem(EquipItem { index, position }) => {
                assert_eq!(index, 7);
                assert_eq!(position, 0x0100);
            }
            other => panic!("expected Body::EquipItem, got {other:?}"),
        }
    }

    #[test]
    fn non_playing_phase_does_not_send() {
        let mut app = App::new();
        app.init_resource::<Inventory>()
            .init_resource::<QuicZoneState>()
            .init_resource::<QuinnetClient>()
            .add_message::<EquipItemRequested>()
            .add_systems(Update, handle_equip_item_send);

        assert_ne!(
            app.world().resource::<QuicZoneState>().phase,
            ZonePhase::Playing
        );

        app.world_mut()
            .resource_mut::<Messages<EquipItemRequested>>()
            .write(EquipItemRequested { index: 1 });

        app.update();
    }
}
