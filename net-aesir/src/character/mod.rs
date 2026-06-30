pub mod flow;
pub mod mapping;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use bevy_quinnet::client::certificate::CertificateVerificationMode;
use bevy_quinnet::client::connection::{ClientAddrConfiguration, ConnectionLocalId};
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
};
use bevy_quinnet::shared::error::AsyncChannelError;

use crate::channels;
use crate::connection::QuicConnection;

/// Phase of the long-lived QUIC char-server session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CharPhase {
    #[default]
    Disconnected,
    Connecting,
    HelloSent,
    AuthSent,
    Ready,
    Selecting,
    Done,
    Failed,
}

/// Session credentials carried from login, sent in `SessionAuth`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingAuth {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u32,
}

/// Drives the QUIC char-server flow: tracks the session phase, owns the
/// seq-counting `QuicConnection`, and holds the session credentials.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::AesirNetPlugin)]
pub struct QuicCharState {
    pub phase: CharPhase,
    pub conn: QuicConnection,
    pub auth: PendingAuth,
}

impl QuicCharState {
    /// Begin a fresh char-server session: reset the seq counter, stash credentials,
    /// and arm the `Connecting` phase so `char_send_hello` fires once the connection opens.
    pub fn start_connecting(&mut self, auth: PendingAuth) {
        self.conn = QuicConnection::default();
        self.auth = auth;
        self.phase = CharPhase::Connecting;
    }
}

/// Opens the QUIC connection to the aesir char server.
///
/// Closes any existing connection first so the new char connection becomes the
/// unambiguous default for `client.connection_mut()` (one-active-connection invariant;
/// login leaves its connection open). Dev cert handling: `SkipVerification` (self-signed).
pub fn connect(
    client: &mut QuinnetClient,
    addr: &str,
) -> Result<ConnectionLocalId, AsyncChannelError> {
    client.close_all_connections();
    let addr_config = ClientAddrConfiguration::from_strings(addr, "0.0.0.0:0")
        .expect("valid char server address");
    client.open_connection(ClientConnectionConfiguration {
        addr_config,
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: channels::send_channels_config(),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_connecting_resets_and_arms() {
        let mut state = QuicCharState {
            phase: CharPhase::Failed,
            ..Default::default()
        };
        state.start_connecting(PendingAuth {
            account_id: 1,
            login_id1: 2,
            login_id2: 3,
            sex: 1,
        });
        assert_eq!(state.phase, CharPhase::Connecting);
        assert_eq!(state.auth.account_id, 1);
        assert_eq!(state.auth.sex, 1);
    }
}
