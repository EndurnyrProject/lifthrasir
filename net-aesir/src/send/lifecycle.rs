use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::QuinnetClient;
use net_contract::commands::{ConnectCharServer, ConnectLogin, ConnectZone};
use net_contract::events::{LoginRefused, ZoneDisconnected};

use crate::character::{self, PendingAuth, QuicCharState};
use crate::login::{self, Pending, QuicLoginState};
use crate::zone::{self, QuicZoneState, ZoneAuth};

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
