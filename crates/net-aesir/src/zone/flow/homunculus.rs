use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::{events::HomunculusIngress, state::ZoneSessionGeneration};

use crate::{
    dispatch::IncomingMessage,
    zone::{QuicZoneState, ZonePhase, mapping::homunculus::ingress},
};

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update, config(run_if = client_connected, after = crate::zone::session::publish_zone_session, before = crate::send::lifecycle::advance_zone_handshake))]
pub fn zone_drain_homunculus(
    zone: Res<QuicZoneState>,
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<HomunculusIngress>,
) {
    if !matches!(
        zone.phase,
        ZonePhase::Entering | ZonePhase::MapReady | ZonePhase::Playing
    ) {
        incoming.clear();
        return;
    }
    for message in incoming.read() {
        if let Some(payload) = ingress(message) {
            out.write(HomunculusIngress {
                generation: ZoneSessionGeneration(zone.connection_epoch),
                sequence: message.sequence,
                payload,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::{BULK, CONTROL, GAMEPLAY, WORLD},
        envelope::Body,
        proto::aesir::net,
    };
    use net_contract::{
        dto::{HomunculusError, HomunculusLifecycle},
        events::HomunculusIngressPayload as Ingress,
    };

    #[test]
    fn homunculus_ingress_keeps_channels_generation_and_sequence() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<HomunculusIngress>()
            .insert_resource(QuicZoneState {
                phase: ZonePhase::Entering,
                connection_epoch: 4,
                ..default()
            })
            .add_systems(Update, zone_drain_homunculus);
        let state = Body::HomunculusPrivateState(net::HomunculusPrivateState {
            durable_id: 8,
            hp: u64::MAX,
            lifecycle: 777,
            ..default()
        });
        let result = Body::HomunculusResult(net::HomunculusResult {
            request_id: 42,
            error: 555,
            ..default()
        });
        for (sequence, channel, body) in [
            (22, GAMEPLAY, result.clone()),
            (20, BULK, state.clone()),
            (23, CONTROL, Body::MapMove(net::MapMove::default())),
            (
                24,
                BULK,
                Body::HomunculusPrivateState(net::HomunculusPrivateState {
                    durable_id: 8,
                    removed: true,
                    ..default()
                }),
            ),
            (25, GAMEPLAY, state),
            (26, WORLD, result),
            (
                27,
                BULK,
                Body::HomunculusPrivateState(net::HomunculusPrivateState::default()),
            ),
        ] {
            app.world_mut().write_message(IncomingMessage {
                channel,
                sequence,
                body,
            });
        }
        app.update();
        let events = app
            .world()
            .resource::<Messages<HomunculusIngress>>()
            .iter_current_update_messages()
            .collect::<Vec<_>>();
        assert_eq!(
            events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [22, 20, 23, 24]
        );
        assert!(
            events
                .iter()
                .all(|e| e.generation == ZoneSessionGeneration(4))
        );
        assert!(matches!(
            events[0].payload,
            Ingress::Result {
                request_id: 42,
                error: HomunculusError::Unknown(555),
                state: None,
                ..
            }
        ));
        assert!(
            matches!(&events[1].payload, Ingress::State(s) if s.hp == u64::MAX && s.lifecycle == HomunculusLifecycle::Unknown(777))
        );
        assert_eq!(events[2].payload, Ingress::MapBarrier);
        assert_eq!(events[3].payload, Ingress::Removed { durable_id: 8 });
    }
}
