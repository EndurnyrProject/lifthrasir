use std::time::Duration;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use bevy_quinnet::client::connection::{
    ConnectionEvent, ConnectionFailedEvent, ConnectionLostEvent,
};
use bevy_quinnet::client::QuinnetClient;

use super::super::mapping::handshake::enter_ack;
use super::super::{QuicZoneState, ZonePhase, ZoneSpawn};
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::{Hello, SessionAuth, TimeSync};
use crate::infrastructure::networking::zone_messages::{ZoneDisconnected, ZoneEntered};

/// Periodic time-sync cadence, preserving the legacy TCP zone path's 30s interval.
const TIME_SYNC_INTERVAL: Duration = Duration::from_secs(30);

/// Pure outcome of receiving a `HelloAck`: the next phase, or `None` when out of phase.
fn hello_ack_next(phase: ZonePhase, accepted: bool) -> Option<ZonePhase> {
    if phase != ZonePhase::HelloSent {
        return None;
    }
    Some(if accepted {
        ZonePhase::AuthSent
    } else {
        ZonePhase::Failed
    })
}

/// Pure outcome of receiving an `EnterAck`: the next phase, or `None` when out of phase.
fn enter_ack_next(phase: ZonePhase) -> Option<ZonePhase> {
    (phase == ZonePhase::AuthSent).then_some(ZonePhase::Entering)
}

/// On a fresh quinnet connection, send the `Hello` handshake on the control channel.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn zone_send_hello(
    mut events: MessageReader<ConnectionEvent>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
) {
    for _ in events.read() {
        if state.phase != ZonePhase::Connecting {
            continue;
        }
        let hello = Body::Hello(Hello {
            protocol_version: 1,
            build: "lifthrasir".into(),
        });
        if let Err(e) = state.send(&mut client, CONTROL, hello) {
            error!("failed to send zone Hello: {e}");
            state.phase = ZonePhase::Failed;
            continue;
        }
        state.phase = ZonePhase::HelloSent;
    }
}

/// Drains the control channel and advances the zone-server session.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_control(
    mut incoming: MessageReader<IncomingMessage>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
    mut entered: MessageWriter<ZoneEntered>,
) {
    for msg in incoming.read() {
        if msg.channel != CONTROL {
            continue;
        }
        match msg.body.clone() {
            Body::HelloAck(ack) => {
                let Some(next) = hello_ack_next(state.phase, ack.accepted) else {
                    continue;
                };
                if next == ZonePhase::Failed {
                    warn!("zone server rejected Hello handshake");
                    state.phase = ZonePhase::Failed;
                    continue;
                }
                let auth = Body::SessionAuth(SessionAuth {
                    account_id: state.auth.account_id,
                    login_id1: state.auth.login_id1,
                    login_id2: state.auth.login_id2,
                    sex: state.auth.sex,
                    char_id: state.auth.char_id,
                    zone_auth_token: state.auth.zone_auth_token.clone(),
                });
                if let Err(e) = state.send(&mut client, CONTROL, auth) {
                    error!("failed to send zone SessionAuth: {e}");
                    state.phase = ZonePhase::Failed;
                    continue;
                }
                state.phase = next;
            }
            Body::EnterAck(ack) => {
                let Some(next) = enter_ack_next(state.phase) else {
                    warn!("unexpected EnterAck in phase {:?}", state.phase);
                    continue;
                };
                state.spawn = Some(ZoneSpawn::from_enter_ack(&ack));
                entered.write(enter_ack(ack));
                state.phase = next;
            }
            Body::TimeSyncAck(reply) => {
                state.clock_offset = reply.server_tick as i64;
            }
            _ => warn!("unexpected control body on zone channel"),
        }
    }
}

/// Periodically sends `TimeSync { client_tick }` on the control channel.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_time_sync(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
) {
    let timer = timer.get_or_insert_with(|| Timer::new(TIME_SYNC_INTERVAL, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }
    let client_tick = (time.elapsed_secs() * 1000.0) as u32;
    let body = Body::TimeSync(TimeSync { client_tick });
    if let Err(e) = state.send(&mut client, CONTROL, body) {
        error!("failed to send TimeSync: {e}");
    }
}

/// Maps quinnet connection failure / loss onto a failed zone session.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn zone_handle_connection_lost(
    mut failed_events: MessageReader<ConnectionFailedEvent>,
    mut lost_events: MessageReader<ConnectionLostEvent>,
    mut state: ResMut<QuicZoneState>,
    mut disconnected: MessageWriter<ZoneDisconnected>,
) {
    let mut fail = |state: &mut QuicZoneState, message: String| {
        if state.phase == ZonePhase::Disconnected {
            return;
        }
        error!("zone connection lost: {message}");
        state.phase = ZonePhase::Failed;
        disconnected.write(ZoneDisconnected { reason: message });
    };

    for event in failed_events.read() {
        fail(&mut state, format!("connection failed: {}", event.err));
    }
    for _ in lost_events.read() {
        fail(&mut state, "connection lost".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_ack_accepted_in_hello_sent_advances_to_auth_sent() {
        assert_eq!(
            hello_ack_next(ZonePhase::HelloSent, true),
            Some(ZonePhase::AuthSent)
        );
    }

    #[test]
    fn hello_ack_rejected_in_hello_sent_fails() {
        assert_eq!(
            hello_ack_next(ZonePhase::HelloSent, false),
            Some(ZonePhase::Failed)
        );
    }

    #[test]
    fn hello_ack_out_of_phase_is_ignored() {
        assert_eq!(hello_ack_next(ZonePhase::Connecting, true), None);
        assert_eq!(hello_ack_next(ZonePhase::AuthSent, true), None);
    }

    #[test]
    fn enter_ack_in_auth_sent_advances_to_entering() {
        assert_eq!(
            enter_ack_next(ZonePhase::AuthSent),
            Some(ZonePhase::Entering)
        );
    }

    #[test]
    fn enter_ack_out_of_phase_is_ignored() {
        assert_eq!(enter_ack_next(ZonePhase::HelloSent), None);
        assert_eq!(enter_ack_next(ZonePhase::Entering), None);
    }
}
