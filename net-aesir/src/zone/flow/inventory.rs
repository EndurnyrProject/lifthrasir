use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::inventory::{
    equip_result, inventory_list, item_added, item_removed, item_use_result, unequip_result,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    InventoryReceived, ItemAdded, ItemEquipped, ItemRemoved, ItemUnequipped, ItemUseFailed,
};

/// Drains inventory bodies. The dump rides the bulk channel and the deltas ride
/// gameplay, so the match is on the `Body` variant directly, not the channel.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
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
    mut use_failed: MessageWriter<ItemUseFailed>,
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
            Body::ItemUseResult(r) if !r.ok => {
                use_failed.write(item_use_result(r));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{BULK, GAMEPLAY};
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<InventoryReceived>()
            .add_message::<ItemAdded>()
            .add_message::<ItemRemoved>()
            .add_message::<ItemEquipped>()
            .add_message::<ItemUnequipped>()
            .add_message::<ItemUseFailed>()
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

    #[test]
    fn item_use_result_failure_produces_one_item_use_failed() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemUseResult(net::ItemUseResult {
                index: 3,
                ok: false,
                reason: 2,
            }),
        )]);

        let failed = app.world().resource::<Messages<ItemUseFailed>>();
        let events: Vec<_> = failed.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].index, 3);
        assert_eq!(events[0].reason, 2);
    }

    #[test]
    fn item_use_result_success_produces_no_item_use_failed() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemUseResult(net::ItemUseResult {
                index: 3,
                ok: true,
                reason: 0,
            }),
        )]);

        let failed = app.world().resource::<Messages<ItemUseFailed>>();
        assert_eq!(failed.iter_current_update_messages().count(), 0);
    }
}
