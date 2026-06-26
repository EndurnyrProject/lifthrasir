use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::status::status_change;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::StatusEffectChanged;

/// Drains EFST status-bar bodies into domain events. Aesir feigns death purely
/// through this channel: SC_TRICKDEAD carries no opt field, so the dead pose is
/// driven by the trickdead EFST toggling on and off.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_status(
    mut incoming: MessageReader<IncomingMessage>,
    mut changed: MessageWriter<StatusEffectChanged>,
) {
    for msg in incoming.read() {
        if let Body::StatusChange(s) = msg.body.clone() {
            changed.write(status_change(s));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::channels::GAMEPLAY;
    use crate::infrastructure::networking::quic::proto::aesir::net;

    #[test]
    fn status_change_drains_to_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<StatusEffectChanged>()
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
}
