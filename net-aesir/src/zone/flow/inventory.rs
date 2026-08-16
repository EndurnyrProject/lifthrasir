use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::inventory::{
    equip_result, inventory_list, item_added, item_bound, item_removed, item_use_result,
    unequip_result,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    InventoryReceived, ItemAdded, ItemBound, ItemEquipped, ItemRemoved, ItemUnequipped,
    ItemUseFailed,
};

#[derive(SystemParam)]
pub struct InventoryEventWriters<'w> {
    received: MessageWriter<'w, InventoryReceived>,
    added: MessageWriter<'w, ItemAdded>,
    removed: MessageWriter<'w, ItemRemoved>,
    bound: MessageWriter<'w, ItemBound>,
    equipped: MessageWriter<'w, ItemEquipped>,
    unequipped: MessageWriter<'w, ItemUnequipped>,
    use_failed: MessageWriter<'w, ItemUseFailed>,
}

/// Drains inventory bodies. The dump rides the bulk channel and the deltas ride
/// gameplay, so the match is on the `Body` variant directly, not the channel.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_inventory(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: InventoryEventWriters,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::InventoryList(l) => {
                out.received.write(inventory_list(l));
            }
            Body::ItemAdded(a) => {
                out.added.write(item_added(a));
            }
            Body::ItemRemoved(r) => {
                out.removed.write(item_removed(r));
            }
            Body::ItemBound(b) => {
                out.bound.write(item_bound(b));
            }
            Body::EquipResult(e) => {
                out.equipped.write(equip_result(e));
            }
            Body::UnequipResult(u) => {
                out.unequipped.write(unequip_result(u));
            }
            Body::ItemUseResult(r) if !r.ok => {
                out.use_failed.write(item_use_result(r));
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
            .add_message::<ItemBound>()
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
    fn item_bound_on_gameplay_produces_one_item_bound() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemBound(net::ItemBound { index: 7, bound: 4 }),
        )]);

        let bound = app.world().resource::<Messages<ItemBound>>();
        let events: Vec<_> = bound.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].index, 7);
        assert_eq!(events[0].bound, 4);
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
