use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{client_connected, QuinnetClient};
use net_contract::commands::{
    ConnectCharServer, ConnectLogin, ConnectZone, LeaveZone, LocalMapLoaded, LocalPlayerReady,
    RespawnRequested,
};
use net_contract::events::{LoginRefused, MapChangeRequested, ZoneDisconnected};

use crate::channels::{CONTROL, GAMEPLAY};
use crate::character::{self, PendingAuth, QuicCharState};
use crate::envelope::Body;
use crate::login::{self, Pending, QuicLoginState};
use crate::proto::aesir::net::{MapLoaded, Respawn};
use crate::zone::{self, QuicZoneState, ZoneAuth, ZonePhase};

/// Pure outcome of the map asset becoming ready: the next phase, or `None` when out of phase.
fn map_loaded_next(phase: ZonePhase) -> Option<ZonePhase> {
    (phase == ZonePhase::Entering).then_some(ZonePhase::MapReady)
}

/// Pure outcome of the local player entity becoming ready: the next phase, or
/// `None` when out of phase.
fn player_ready_next(phase: ZonePhase) -> Option<ZonePhase> {
    (phase == ZonePhase::MapReady).then_some(ZonePhase::Playing)
}

/// Open the login-server connection and arm the login handshake.
///
/// On an immediate connect error, surface the existing `LoginRefused` contract
/// event (mirroring `login::flow::quic_handle_connection_lost`) so the domain's
/// failure path stays identical whether the failure is at connect or in-flight.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn handle_connect_login(
    mut events: MessageReader<ConnectLogin>,
    mut client: ResMut<QuinnetClient>,
    mut login_state: ResMut<QuicLoginState>,
    mut refused: MessageWriter<LoginRefused>,
) {
    for cmd in events.read() {
        if let Err(e) = login::connect(&mut client, &cmd.address) {
            error!("failed to connect to login server {}: {e}", cmd.address);
            refused.write(LoginRefused {
                username: cmd.username.clone(),
                error_code: 3,
                error_message: format!("connection failed: {e}"),
                block_date: None,
            });
            continue;
        }
        login_state.start_connecting(Pending {
            username: cmd.username.clone(),
            password: cmd.password.clone(),
            client_version: cmd.client_version,
            build: cmd.build.clone(),
        });
    }
}

/// Open the char-server connection and arm the char-session handshake.
///
/// There is no contract char-failure event (the adapter's
/// `char_handle_connection_lost` only logs and sets `CharPhase::Failed`), so an
/// immediate connect error is logged here and not surfaced to the domain.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn handle_connect_char_server(
    mut events: MessageReader<ConnectCharServer>,
    mut client: ResMut<QuinnetClient>,
    mut char_state: ResMut<QuicCharState>,
) {
    for cmd in events.read() {
        if let Err(e) = character::connect(&mut client, &cmd.address) {
            error!("failed to connect to char server {}: {e}", cmd.address);
            continue;
        }
        char_state.start_connecting(PendingAuth {
            account_id: cmd.account_id,
            login_id1: cmd.login_id1,
            login_id2: cmd.login_id2,
            sex: cmd.sex,
        });
    }
}

/// Open the zone-server connection and arm the zone handshake.
///
/// `zone::connect` closes any existing connection first (the char hop), so the
/// handoff-close the domain used to log is preserved inside the connect call. On
/// an immediate connect error, surface the existing `ZoneDisconnected` event.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn handle_connect_zone(
    mut events: MessageReader<ConnectZone>,
    mut client: ResMut<QuinnetClient>,
    mut zone_state: ResMut<QuicZoneState>,
    mut disconnected: MessageWriter<ZoneDisconnected>,
) {
    for cmd in events.read() {
        if let Err(e) = zone::connect(&mut client, &cmd.address) {
            error!("failed to connect to zone server {}: {e}", cmd.address);
            disconnected.write(ZoneDisconnected {
                reason: format!("connection failed: {e}"),
            });
            continue;
        }
        zone_state.start_connecting(
            ZoneAuth {
                account_id: cmd.account_id,
                login_id1: cmd.login_id1,
                login_id2: cmd.login_id2,
                sex: cmd.sex,
                char_id: cmd.char_id,
                zone_auth_token: cmd.zone_auth_token.clone(),
            },
            cmd.map_name.clone(),
        );
    }
}

/// Drive the zone map-load handshake from the domain readiness signals.
///
/// Latches `LocalMapLoaded`/`LocalPlayerReady` onto `QuicZoneState` so signal
/// order versus phase never matters, then advances the phase machine: once
/// **both** the map and the local player are ready while `Entering`, send
/// `MapLoaded` (legacy CZ_NOTIFY_ACTORINIT) and move to `MapReady`, then
/// immediately to `Playing` (the gate the gameplay senders wait on).
///
/// `MapLoaded` must wait for the player, not just the map: the server answers
/// it with the login self-sync (own `UnitStateChange`, status params, cart
/// dump), and its consumers resolve `unit_id` through the entity registry. If
/// the ack went out before the local player is spawned and registered, that
/// sync would arrive early and be dropped on the registry miss (e.g. a
/// relogged merchant's cart never rendering). The player spawn is purely
/// client-driven (spawn cell comes from `EnterAck`), so nothing here waits on
/// the server.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn advance_zone_handshake(
    mut map_loaded: MessageReader<LocalMapLoaded>,
    mut player_ready: MessageReader<LocalPlayerReady>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
) {
    if !map_loaded.is_empty() {
        map_loaded.clear();
        state.map_loaded_signal = true;
    }
    if !player_ready.is_empty() {
        player_ready.clear();
        state.player_ready_signal = true;
    }

    if state.map_loaded_signal && state.player_ready_signal {
        if let Some(next) = map_loaded_next(state.phase) {
            if let Err(e) = state.send(&mut client, CONTROL, Body::MapLoaded(MapLoaded {})) {
                error!("failed to send MapLoaded: {e}");
                state.phase = ZonePhase::Failed;
                return;
            }
            debug!("zone handshake: MapLoaded sent (map + player ready)");
            state.phase = next;
        }
    }

    if state.player_ready_signal {
        if let Some(next) = player_ready_next(state.phase) {
            state.phase = next;
        }
    }
}

/// Tear down the zone session when the domain leaves the zone (return to login).
///
/// Resets the phase to `Disconnected` and clears the handshake latches so a later
/// re-entry starts from a clean state machine rather than a stale `Playing`/latched one.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn handle_leave_zone(mut events: MessageReader<LeaveZone>, mut state: ResMut<QuicZoneState>) {
    for _ in events.read() {
        state.phase = ZonePhase::Disconnected;
        state.map_loaded_signal = false;
        state.player_ready_signal = false;
    }
}

/// Re-arm the map-load handshake on a server warp.
///
/// `MapChangeRequested` means the client is unloading and reloading a map, so the
/// handshake must replay: reset to `Entering` and clear the latched map signal so
/// the next `LocalMapLoaded` re-sends `MapLoaded`. The player entity survives a
/// warp, so `player_ready_signal` is intentionally kept.
#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub fn reset_handshake_on_warp(
    mut events: MessageReader<MapChangeRequested>,
    mut state: ResMut<QuicZoneState>,
) {
    for _ in events.read() {
        state.phase = ZonePhase::Entering;
        state.map_loaded_signal = false;
    }
}

fn respawn_body(r: &RespawnRequested) -> Body {
    Body::Respawn(Respawn { r#type: r.type_ })
}

/// Send the death-screen respawn request (save point or char select).
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_respawn_requests(
    mut events: MessageReader<RespawnRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, respawn_body(ev)) {
            error!("failed to send Respawn: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_loaded_in_entering_advances_to_map_ready() {
        assert_eq!(
            map_loaded_next(ZonePhase::Entering),
            Some(ZonePhase::MapReady)
        );
    }

    #[test]
    fn map_loaded_out_of_phase_is_ignored() {
        assert_eq!(map_loaded_next(ZonePhase::AuthSent), None);
        assert_eq!(map_loaded_next(ZonePhase::MapReady), None);
    }

    #[test]
    fn player_ready_in_map_ready_advances_to_playing() {
        assert_eq!(
            player_ready_next(ZonePhase::MapReady),
            Some(ZonePhase::Playing)
        );
    }

    #[test]
    fn player_ready_out_of_phase_is_ignored() {
        assert_eq!(player_ready_next(ZonePhase::Entering), None);
        assert_eq!(player_ready_next(ZonePhase::Playing), None);
    }

    #[test]
    fn respawn_body_carries_type_save_point() {
        match respawn_body(&RespawnRequested { type_: 0 }) {
            Body::Respawn(Respawn { r#type }) => assert_eq!(r#type, 0u32),
            other => panic!("expected Body::Respawn, got {other:?}"),
        }
    }

    #[test]
    fn respawn_body_carries_type_char_select() {
        match respawn_body(&RespawnRequested { type_: 1 }) {
            Body::Respawn(Respawn { r#type }) => assert_eq!(r#type, 1u32),
            other => panic!("expected Body::Respawn, got {other:?}"),
        }
    }
}
