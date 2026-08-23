use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::production::production_result;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::ProductionResult;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_production(
    mut incoming: MessageReader<IncomingMessage>,
    mut results: MessageWriter<ProductionResult>,
) {
    for msg in incoming.read() {
        if let Body::ProductionResult(r) = msg.body.clone() {
            results.write(production_result(r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::proto::aesir::net;

    #[test]
    fn production_result_produces_one_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ProductionResult>()
            .add_systems(Update, zone_drain_production);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: Body::ProductionResult(net::ProductionResult {
                    success: true,
                    item_id: 1201,
                }),
            });
        app.update();

        let results = app.world().resource::<Messages<ProductionResult>>();
        let events: Vec<_> = results.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert!(events[0].success);
        assert_eq!(events[0].item_id, 1201);
    }

    #[test]
    fn unrelated_body_produces_nothing() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ProductionResult>()
            .add_systems(Update, zone_drain_production);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: Body::MountResult(net::MountResult { result: 0 }),
            });
        app.update();

        assert!(
            app.world()
                .resource::<Messages<ProductionResult>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
    }
}
