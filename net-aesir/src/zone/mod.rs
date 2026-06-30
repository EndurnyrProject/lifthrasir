pub mod flow;
pub mod mapping;
pub mod session;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use bevy_quinnet::client::certificate::CertificateVerificationMode;
use bevy_quinnet::client::connection::{ClientAddrConfiguration, ConnectionLocalId};
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, ClientSendError,
    QuinnetClient,
};
use bevy_quinnet::shared::error::AsyncChannelError;

use crate::channels;
use crate::connection::QuicConnection;
use crate::envelope::Body;
use crate::proto::aesir::net;

/// Phase of the long-lived QUIC zone-server session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ZonePhase {
    #[default]
    Disconnected,
    Connecting,
    HelloSent,
    AuthSent,
    Entering,
    MapReady,
    Playing,
    Failed,
}

/// Session credentials carried from login/char, sent in zone `SessionAuth`.
#[derive(Debug, Clone, Default)]
pub struct ZoneAuth {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u32,
    pub char_id: u32,
    /// Single-use handoff token from `ZoneServerInfo`, echoed in `SessionAuth.zone_auth_token`.
    pub zone_auth_token: Vec<u8>,
}

/// The spawn cell carried by `EnterAck`, stored for the local-player spawn.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZoneSpawn {
    pub account_id: u32,
    pub x: u32,
    pub y: u32,
    pub dir: u32,
    pub start_time: u64,
}

/// Drives the QUIC zone-server flow: tracks the session phase, owns the
/// seq-counting `QuicConnection`, holds session credentials and the target map,
/// and stashes the spawn cell from `EnterAck`. Also tracks the latest server
/// clock offset from `TimeSyncAck`.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::AesirNetPlugin)]
pub struct QuicZoneState {
    pub phase: ZonePhase,
    pub conn: QuicConnection,
    pub auth: ZoneAuth,
    pub map_name: String,
    pub spawn: Option<ZoneSpawn>,
    pub clock_offset: i64,
}

impl QuicZoneState {
    /// Begin a fresh zone-server session: reset the seq counter, stash credentials
    /// and the target map, clear any prior spawn, and arm the `Connecting` phase so
    /// `zone_send_hello` fires once the connection opens.
    pub fn start_connecting(&mut self, auth: ZoneAuth, map_name: String) {
        self.conn = QuicConnection::default();
        self.auth = auth;
        self.map_name = map_name;
        self.spawn = None;
        self.phase = ZonePhase::Connecting;
    }

    /// Encode and send a body on the given channel via the seq-counting connection.
    pub fn send(
        &mut self,
        client: &mut QuinnetClient,
        channel: u8,
        body: Body,
    ) -> Result<(), ClientSendError> {
        self.conn.send(client.connection_mut(), channel, body)
    }
}

/// Opens the QUIC connection to the aesir zone server.
///
/// Closes any existing connection first so the new zone connection becomes the
/// unambiguous default for `client.connection_mut()` (one-active-connection
/// invariant). Dev cert handling: `SkipVerification` (self-signed).
pub fn connect(
    client: &mut QuinnetClient,
    addr: &str,
) -> Result<ConnectionLocalId, AsyncChannelError> {
    client.close_all_connections();
    let addr_config = ClientAddrConfiguration::from_strings(addr, "0.0.0.0:0")
        .expect("valid zone server address");
    client.open_connection(ClientConnectionConfiguration {
        addr_config,
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: channels::send_channels_config(),
            ..Default::default()
        },
    })
}

impl ZoneSpawn {
    pub fn from_enter_ack(e: &net::EnterAck) -> Self {
        Self {
            account_id: e.account_id,
            x: e.x,
            y: e.y,
            dir: e.dir,
            start_time: e.start_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_connecting_resets_and_arms() {
        let mut state = QuicZoneState {
            phase: ZonePhase::Failed,
            spawn: Some(ZoneSpawn::default()),
            ..Default::default()
        };
        state.start_connecting(
            ZoneAuth {
                account_id: 1,
                login_id1: 2,
                login_id2: 3,
                sex: 1,
                char_id: 4,
                zone_auth_token: vec![9, 9, 9],
            },
            "prontera".into(),
        );
        assert_eq!(state.phase, ZonePhase::Connecting);
        assert_eq!(state.auth.account_id, 1);
        assert_eq!(state.auth.char_id, 4);
        assert_eq!(state.map_name, "prontera");
        assert!(state.spawn.is_none());
    }

    #[test]
    fn spawn_from_enter_ack_copies_cell() {
        let spawn = ZoneSpawn::from_enter_ack(&net::EnterAck {
            account_id: 2000001,
            x: 150,
            y: 99,
            dir: 4,
            start_time: 123456789,
            font: 0,
        });
        assert_eq!(spawn.account_id, 2000001);
        assert_eq!(spawn.x, 150);
        assert_eq!(spawn.y, 99);
        assert_eq!(spawn.dir, 4);
        assert_eq!(spawn.start_time, 123456789);
    }
}
