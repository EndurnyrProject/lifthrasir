use std::time::Duration;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::QuinnetClient;
use bevy_quinnet::client::client_connected;
use bevy_quinnet::client::connection::{
    ConnectionEvent, ConnectionFailedEvent, ConnectionLostEvent,
};

use super::super::mapping::handshake::enter_ack;
use super::super::{QuicZoneState, ZonePhase, ZoneSpawn};
use crate::channels::CONTROL;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use crate::proto::aesir::net::{Hello, SessionAuth, TimeSync};
use net_contract::events::{ServerClockSynced, ZoneDisconnected, ZoneEntered};

/// Time-sync doubles as a liveness probe so an abruptly stopped UDP server is noticed promptly.
const TIME_SYNC_INTERVAL: Duration = Duration::from_secs(5);
const TIME_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

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
    plugin = crate::AesirNetPlugin,
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
            capabilities: Vec::new(),
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
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_control(
    mut incoming: MessageReader<IncomingMessage>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
    mut entered: MessageWriter<ZoneEntered>,
    mut clock_synced: MessageWriter<ServerClockSynced>,
    time: Res<Time<Real>>,
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
                let Some(sent_ms) = state.time_sync_sent_ms.take() else {
                    warn!("TimeSyncAck without an in-flight TimeSync; ignoring");
                    continue;
                };
                let recv_ms = time.elapsed().as_millis() as i64;
                clock_synced.write(time_sync_result(sent_ms, recv_ms, reply.server_tick));
            }
            _ => warn!("unexpected control body on zone channel"),
        }
    }
}

fn fail_zone_session(state: &mut QuicZoneState, reason: String) -> Option<ZoneDisconnected> {
    if matches!(state.phase, ZonePhase::Disconnected | ZonePhase::Failed) {
        return None;
    }
    state.phase = ZonePhase::Failed;
    Some(ZoneDisconnected { reason })
}

fn time_sync_timed_out(sent_ms: Option<i64>, now_ms: i64) -> bool {
    sent_ms.is_some_and(|sent_ms| {
        now_ms.saturating_sub(sent_ms) >= TIME_SYNC_TIMEOUT.as_millis() as i64
    })
}

/// Sends `TimeSync` on the control channel once the session is entering the map.
/// One probe remains in flight until its acknowledgement arrives; a missing reply
/// for `TIME_SYNC_TIMEOUT` fails the zone session and surfaces `ZoneDisconnected`.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected, after = zone_drain_control)
)]
pub fn zone_time_sync(
    time: Res<Time<Real>>,
    mut timer: Local<Option<Timer>>,
    mut synced_epoch: Local<u64>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
    mut disconnected: MessageWriter<ZoneDisconnected>,
) {
    if !matches!(
        state.phase,
        ZonePhase::Entering | ZonePhase::MapReady | ZonePhase::Playing
    ) {
        return;
    }
    let now_ms = time.elapsed().as_millis() as i64;
    if time_sync_timed_out(state.time_sync_sent_ms, now_ms) {
        let reason = "zone server did not answer the liveness probe".to_string();
        if let Some(event) = fail_zone_session(&mut state, reason) {
            error!("zone connection lost: {}", event.reason);
            disconnected.write(event);
        }
        return;
    }
    if state.time_sync_sent_ms.is_some() {
        return;
    }

    let timer = timer.get_or_insert_with(|| Timer::new(TIME_SYNC_INTERVAL, TimerMode::Repeating));
    let new_connection = *synced_epoch != state.connection_epoch;
    if !new_connection && !timer.tick(time.delta()).just_finished() {
        return;
    }
    let body = Body::TimeSync(TimeSync {
        client_tick: now_ms as u32,
    });
    if let Err(error) = state.send(&mut client, CONTROL, body) {
        let reason = format!("failed to send zone liveness probe: {error}");
        if let Some(event) = fail_zone_session(&mut state, reason) {
            error!("zone connection lost: {}", event.reason);
            disconnected.write(event);
        }
        return;
    }
    state.time_sync_sent_ms = Some(now_ms);
    *synced_epoch = state.connection_epoch;
    timer.reset();
}

/// Cristian's-algorithm clock estimate from a `TimeSync` round-trip: the server
/// samples its tick roughly at the round-trip midpoint, so the offset against the
/// client's `Time<Real>` clock is `server_tick + rtt/2 - recv`.
fn time_sync_result(sent_ms: i64, recv_ms: i64, server_tick: u32) -> ServerClockSynced {
    let rtt_ms = (recv_ms - sent_ms).max(0);
    let offset_ms = server_tick as i64 + rtt_ms / 2 - recv_ms;
    ServerClockSynced {
        offset_ms,
        rtt_ms: rtt_ms as u32,
    }
}

/// Maps quinnet connection failure / loss onto a failed zone session.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update
)]
pub fn zone_handle_connection_lost(
    mut failed_events: MessageReader<ConnectionFailedEvent>,
    mut lost_events: MessageReader<ConnectionLostEvent>,
    mut state: ResMut<QuicZoneState>,
    mut disconnected: MessageWriter<ZoneDisconnected>,
) {
    let mut fail = |state: &mut QuicZoneState, reason: String| {
        if let Some(event) = fail_zone_session(state, reason) {
            error!("zone connection lost: {}", event.reason);
            disconnected.write(event);
        }
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

    #[test]
    fn time_sync_result_applies_half_rtt_correction() {
        // sent at t=1000, ack recv at t=1200 (rtt 200), server sampled 5000ms.
        // offset = 5000 + 200/2 - 1200 = 3900.
        let result = time_sync_result(1_000, 1_200, 5_000);
        assert_eq!(result.rtt_ms, 200);
        assert_eq!(result.offset_ms, 3_900);
    }

    #[test]
    fn time_sync_result_clamps_negative_rtt() {
        // A non-monotonic recv (< sent) must not produce a negative rtt.
        let result = time_sync_result(1_200, 1_000, 5_000);
        assert_eq!(result.rtt_ms, 0);
        assert_eq!(result.offset_ms, 5_000 - 1_000);
    }

    #[test]
    fn missing_time_sync_ack_disconnects_a_playing_zone() {
        let mut time = Time::<Real>::default();
        time.update_with_duration(Duration::ZERO);
        time.update_with_duration(TIME_SYNC_TIMEOUT);

        let mut app = App::new();
        app.add_plugins(bevy_quinnet::client::QuinnetClientPlugin::default())
            .insert_resource(time)
            .insert_resource(QuicZoneState {
                phase: ZonePhase::Playing,
                time_sync_sent_ms: Some(0),
                ..Default::default()
            })
            .add_message::<ZoneDisconnected>()
            .add_systems(Update, zone_time_sync);

        app.update();

        let messages = app.world().resource::<Messages<ZoneDisconnected>>();
        let mut cursor = messages.get_cursor();
        let disconnected = cursor
            .read(messages)
            .next()
            .expect("missing liveness reply should disconnect");
        assert_eq!(
            disconnected.reason,
            "zone server did not answer the liveness probe"
        );
        assert_eq!(
            app.world().resource::<QuicZoneState>().phase,
            ZonePhase::Failed
        );
    }
}
