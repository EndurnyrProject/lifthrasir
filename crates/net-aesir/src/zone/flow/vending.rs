//! Ordered vending ingress: one stream over the world channel plus the control
//! channel's map change and the world channel's vendor despawn.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::VendingIngress;
use net_contract::state::ZoneSessionGeneration;

use crate::channels::{CONTROL, WORLD};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use crate::zone::mapping::vending::vending_payload;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(
        after = crate::zone::session::publish_zone_session,
        run_if = client_connected
    )
)]
pub fn zone_drain_vending(
    generation: Res<ZoneSessionGeneration>,
    mut observed_generation: Local<Option<ZoneSessionGeneration>>,
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<VendingIngress>,
) {
    if observed_generation.is_some_and(|previous| previous != *generation) {
        incoming.clear();
        *observed_generation = Some(*generation);
        return;
    }
    *observed_generation = Some(*generation);

    for message in incoming.read() {
        let expected_channel = match message.body {
            Body::MapMove(_) => CONTROL,
            _ => WORLD,
        };
        if message.channel != expected_channel {
            continue;
        }
        let Some(payload) = vending_payload(message.body.clone()) else {
            continue;
        };
        out.write(VendingIngress {
            generation: *generation,
            payload,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{GAMEPLAY, SNAPSHOTS};
    use crate::proto::aesir::net;
    use net_contract::dto::VendingSnapshot;
    use net_contract::events::VendingIngressPayload;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<VendingIngress>()
            .insert_resource(ZoneSessionGeneration(4))
            .add_systems(Update, zone_drain_vending);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    fn events(app: &App) -> Vec<VendingIngress> {
        app.world()
            .resource::<Messages<VendingIngress>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn board(unit_id: u64) -> Body {
        Body::VendingBoardShown(net::VendingBoardShown {
            unit_id,
            title: "Potions".into(),
            shop_id: "shop-1".into(),
        })
    }

    #[test]
    fn world_traffic_keeps_receive_order_and_generation() {
        let app = drain(vec![
            (WORLD, board(5)),
            (
                WORLD,
                Body::UnitDespawn(net::UnitDespawn { gid: 5, reason: 0 }),
            ),
            (
                CONTROL,
                Body::MapMove(net::MapMove {
                    map_name: "prontera".into(),
                    x: 1,
                    y: 2,
                }),
            ),
        ]);

        let events = events(&app);
        assert!(
            events
                .iter()
                .all(|e| e.generation == ZoneSessionGeneration(4))
        );
        let payloads: Vec<_> = events.into_iter().map(|e| e.payload).collect();
        assert!(matches!(payloads[0], VendingIngressPayload::BoardShown(_)));
        assert_eq!(
            payloads[1],
            VendingIngressPayload::VendorDespawned { unit_id: 5 }
        );
        assert_eq!(payloads[2], VendingIngressPayload::MapChanging);
    }

    #[test]
    fn other_channels_never_enter_the_vending_stream() {
        let app = drain(vec![
            (GAMEPLAY, board(5)),
            (SNAPSHOTS, board(6)),
            (
                WORLD,
                Body::MapMove(net::MapMove {
                    map_name: "prontera".into(),
                    x: 1,
                    y: 2,
                }),
            ),
            (CONTROL, board(7)),
        ]);
        assert!(events(&app).is_empty());
    }

    #[test]
    fn non_vending_world_bodies_are_ignored() {
        let app = drain(vec![
            (
                WORLD,
                Body::ItemVanished(net::ItemVanished {
                    ground_id: 1,
                    reason: 0,
                }),
            ),
            (WORLD, board(5)),
        ]);
        let events = events(&app);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload,
            VendingIngressPayload::BoardShown(net_contract::dto::VendingBoard {
                vendor_unit_id: 5,
                shop_id: "shop-1".into(),
                title: "Potions".into(),
            })
        );
    }

    #[test]
    fn generation_change_drops_stale_packets() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<VendingIngress>()
            .insert_resource(ZoneSessionGeneration(4))
            .add_systems(Update, zone_drain_vending);
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: board(5),
            });
        app.update();
        assert_eq!(events(&app).len(), 1);

        app.world_mut().resource_mut::<ZoneSessionGeneration>().0 = 5;
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: board(6),
            });
        app.update();
        assert!(events(&app).is_empty());

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: board(7),
            });
        app.update();
        let events = events(&app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].generation, ZoneSessionGeneration(5));
    }

    #[test]
    fn successful_open_reaches_the_stream_with_its_snapshot() {
        let app = drain(vec![(
            WORLD,
            Body::VendingOpenResult(net::VendingOpenResult {
                result: net::VendingOpenResultCode::VendOk as i32,
                request_id: 2,
                setup_id: "setup-1".into(),
                setup_active: false,
                shop_id: "shop-1".into(),
                title: "Potions".into(),
                items: vec![],
                vendor_unit_id: 5,
            }),
        )]);
        let events = events(&app);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload,
            VendingIngressPayload::OpenResult {
                request_id: 2,
                setup_id: "setup-1".into(),
                setup_active: false,
                outcome: net_contract::dto::VendingOpenOutcome::Success,
                snapshot: Some(VendingSnapshot {
                    vendor_unit_id: 5,
                    shop_id: "shop-1".into(),
                    title: "Potions".into(),
                    items: vec![],
                    total_earned: 0,
                }),
            }
        );
    }
}
