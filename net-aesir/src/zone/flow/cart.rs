use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::cart::{
    cart_info, cart_item_added, cart_item_removed, cart_mount_result,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{CartItemAdded, CartItemRemoved, CartLoaded, CartMountResult};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_cart(
    mut incoming: MessageReader<IncomingMessage>,
    mut loaded: MessageWriter<CartLoaded>,
    mut added: MessageWriter<CartItemAdded>,
    mut removed: MessageWriter<CartItemRemoved>,
    mut mount_result: MessageWriter<CartMountResult>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::CartInfo(i) => {
                loaded.write(cart_info(i));
            }
            Body::CartItemAdded(a) => {
                added.write(cart_item_added(a));
            }
            Body::CartItemRemoved(r) => {
                removed.write(cart_item_removed(r));
            }
            Body::CartMountResult(r) => {
                mount_result.write(cart_mount_result(r));
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
    use net_contract::events::CartMountRejection;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<CartLoaded>()
            .add_message::<CartItemAdded>()
            .add_message::<CartItemRemoved>()
            .add_message::<CartMountResult>()
            .add_systems(Update, zone_drain_cart);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn cart_info_produces_one_cart_loaded_with_weights() {
        let app = drain(vec![(
            BULK,
            Body::CartInfo(net::CartInfo {
                items: vec![net::InventoryItem {
                    index: 0,
                    nameid: 501,
                    weight: 10,
                    ..Default::default()
                }],
                current_weight: 10,
                max_weight: 8000,
            }),
        )]);

        let loaded = app.world().resource::<Messages<CartLoaded>>();
        let events: Vec<_> = loaded.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].items.len(), 1);
        assert_eq!(events[0].current_weight, 10);
        assert_eq!(events[0].max_weight, 8000);
    }

    #[test]
    fn cart_item_added_on_gameplay_produces_one_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::CartItemAdded(net::CartItemAdded {
                index: 3,
                amount: 5,
                weight: 12,
                ..Default::default()
            }),
        )]);

        let added = app.world().resource::<Messages<CartItemAdded>>();
        let events: Vec<_> = added.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].item.index, 3);
        assert_eq!(events[0].item.amount, 5);
        assert_eq!(events[0].item.weight, 12);
    }

    #[test]
    fn cart_item_removed_on_gameplay_produces_one_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::CartItemRemoved(net::CartItemRemoved {
                index: 3,
                amount: 2,
                reason: 1,
            }),
        )]);

        let removed = app.world().resource::<Messages<CartItemRemoved>>();
        let events: Vec<_> = removed.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].index, 3u16);
        assert_eq!(events[0].amount, 2u16);
        assert_eq!(events[0].reason, 1);
    }

    #[test]
    fn cart_mount_result_produces_one_rejection_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::CartMountResult(net::CartMountResult {
                result: net::CartMountResultCode::CartSkillNotLearned as i32,
            }),
        )]);

        let results = app.world().resource::<Messages<CartMountResult>>();
        let events: Vec<_> = results.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].outcome, Err(CartMountRejection::SkillNotLearned));
    }
}
