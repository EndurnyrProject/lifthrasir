use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::ground::{item_on_ground, item_vanished, pickup_result};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{ItemOnGround, ItemVanished, PickupResult};

/// Drains ground-item bodies (drop spawn, vanish, pickup result) on the gameplay channel.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_ground(
    mut incoming: MessageReader<IncomingMessage>,
    mut on_ground: MessageWriter<ItemOnGround>,
    mut vanished: MessageWriter<ItemVanished>,
    mut pickup: MessageWriter<PickupResult>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::ItemOnGround(i) => {
                on_ground.write(item_on_ground(i));
            }
            Body::ItemVanished(v) => {
                vanished.write(item_vanished(v));
            }
            Body::PickupResult(r) => {
                pickup.write(pickup_result(r));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ItemOnGround>()
            .add_message::<ItemVanished>()
            .add_message::<PickupResult>()
            .add_systems(Update, zone_drain_ground);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn item_on_ground_produces_one_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemOnGround(net::ItemOnGround {
                ground_id: 7,
                nameid: 501,
                ..Default::default()
            }),
        )]);

        let events = app.world().resource::<Messages<ItemOnGround>>();
        let collected: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].ground_id, 7);
        assert_eq!(collected[0].nameid, 501);
    }

    #[test]
    fn item_vanished_produces_one_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ItemVanished(net::ItemVanished {
                ground_id: 7,
                reason: net::ItemVanishReason::Expired as i32,
            }),
        )]);

        let events = app.world().resource::<Messages<ItemVanished>>();
        assert_eq!(events.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn pickup_result_produces_one_event() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::PickupResult(net::PickupResult {
                ground_id: 7,
                result: net::PickupResultCode::Ok as i32,
            }),
        )]);

        let events = app.world().resource::<Messages<PickupResult>>();
        assert_eq!(events.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn unrelated_body_produces_no_events() {
        let app = drain(vec![(GAMEPLAY, Body::ItemAdded(net::ItemAdded::default()))]);

        assert_eq!(
            app.world()
                .resource::<Messages<ItemOnGround>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Messages<ItemVanished>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Messages<PickupResult>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
}
