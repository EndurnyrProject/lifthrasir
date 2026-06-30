use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::commands::{EquipRequested, UnequipRequested};

use crate::core::state::GameState;
use crate::domain::inventory::Inventory;

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

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_equip_item_send(
    mut events: MessageReader<EquipItemRequested>,
    inventory: Res<Inventory>,
    mut equip_requests: MessageWriter<EquipRequested>,
) {
    for event in events.read() {
        let Some(item) = inventory.get(event.index) else {
            warn!(
                "Equip requested for unknown inventory index {}",
                event.index
            );
            continue;
        };

        equip_requests.write(EquipRequested {
            index: item.index,
            location: item.location,
        });
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_unequip_item_send(
    mut events: MessageReader<UnequipItemRequested>,
    mut unequip_requests: MessageWriter<UnequipRequested>,
) {
    for event in events.read() {
        unequip_requests.write(UnequipRequested { index: event.index });
    }
}
