use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::status::{status_change, unit_state_change};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{StatusEffectChanged, UnitStateChanged};

/// Drains EFST status-bar bodies into domain events. Aesir feigns death purely
/// through this channel: SC_TRICKDEAD carries no opt field, so the dead pose is
/// driven by the trickdead EFST toggling on and off.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_status(
    mut incoming: MessageReader<IncomingMessage>,
    mut changed: MessageWriter<StatusEffectChanged>,
    mut unit_state: MessageWriter<UnitStateChanged>,
) {
    for msg in incoming.read() {
        if let Body::StatusChange(s) = msg.body.clone() {
            changed.write(status_change(s));
        }
        if let Body::UnitStateChange(s) = msg.body.clone() {
            unit_state.write(unit_state_change(s));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    #[test]
    fn status_change_drains_to_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<StatusEffectChanged>()
            .add_message::<UnitStateChanged>()
            .add_systems(Update, zone_drain_status);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::StatusChange(net::StatusChange {
                    unit_id: 1,
                    efst: 29,
                    on: true,
                    ..Default::default()
                }),
            });
        app.update();

        let events = app.world().resource::<Messages<StatusEffectChanged>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].efst, 29);
        assert!(drained[0].on);
    }

    #[test]
    fn unit_state_change_drains_to_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<StatusEffectChanged>()
            .add_message::<UnitStateChanged>()
            .add_systems(Update, zone_drain_status);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::UnitStateChange(net::UnitStateChange {
                    unit_id: 150001,
                    body_state: 1,
                    health_state: 2,
                    effect_state: 4,
                    virtue: 8,
                }),
            });
        app.update();

        let events = app.world().resource::<Messages<UnitStateChanged>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].unit_id, 150001);
        assert_eq!(drained[0].body_state, 1);
        assert_eq!(drained[0].health_state, 2);
        assert_eq!(drained[0].effect_state, 4);
        assert_eq!(drained[0].virtue, 8);
    }
}
