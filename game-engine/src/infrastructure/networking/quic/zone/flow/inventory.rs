use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::inventory::{
    equip_result, inventory_list, item_added, item_removed, unequip_result,
};
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::{
    InventoryReceived, ItemAdded, ItemEquipped, ItemRemoved, ItemUnequipped,
};

/// Drains inventory bodies. The dump rides the bulk channel and the deltas ride
/// gameplay, so the match is on the `Body` variant directly, not the channel.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_inventory(
    mut incoming: MessageReader<IncomingMessage>,
    mut received: MessageWriter<InventoryReceived>,
    mut added: MessageWriter<ItemAdded>,
    mut removed: MessageWriter<ItemRemoved>,
    mut equipped: MessageWriter<ItemEquipped>,
    mut unequipped: MessageWriter<ItemUnequipped>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::InventoryList(l) => {
                received.write(inventory_list(l));
            }
            Body::ItemAdded(a) => {
                added.write(item_added(a));
            }
            Body::ItemRemoved(r) => {
                removed.write(item_removed(r));
            }
            Body::EquipResult(e) => {
                equipped.write(equip_result(e));
            }
            Body::UnequipResult(u) => {
                unequipped.write(unequip_result(u));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::channels::{BULK, GAMEPLAY};
    use crate::infrastructure::networking::quic::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<InventoryReceived>()
            .add_message::<ItemAdded>()
            .add_message::<ItemRemoved>()
            .add_message::<ItemEquipped>()
            .add_message::<ItemUnequipped>()
            .add_systems(Update, zone_drain_inventory);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn inventory_list_on_bulk_produces_one_inventory_received() {
        let app = drain(vec![(
            BULK,
            Body::InventoryList(net::InventoryList::default()),
        )]);

        let received = app.world().resource::<Messages<InventoryReceived>>();
        assert_eq!(received.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn item_added_on_gameplay_produces_one_item_added() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemAdded(net::ItemAdded {
                index: 3,
                amount: 5,
                ..Default::default()
            }),
        )]);

        let added = app.world().resource::<Messages<ItemAdded>>();
        let events: Vec<_> = added.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].index, 3);
        assert_eq!(events[0].amount, 5);
    }
}
