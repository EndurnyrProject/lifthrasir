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

use crate::infrastructure::networking::quic::channels;
use crate::infrastructure::networking::quic::connection::QuicConnection;

/// Phase of the QUIC login handshake.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LoginPhase {
    #[default]
    Disconnected,
    Connecting,
    HelloSent,
    Ready,
    LoginSent,
    Done,
    Failed,
}

/// In-flight credentials needed to build the `Hello` + `LoginRequest`.
#[derive(Debug, Clone, Default)]
pub struct Pending {
    pub username: String,
    pub password: String,
    pub client_version: u32,
    pub build: String,
}

/// Drives the QUIC login flow: tracks the handshake phase, owns the seq-counting
/// `QuicConnection`, and holds the credentials in flight.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct QuicLoginState {
    pub phase: LoginPhase,
    pub conn: QuicConnection,
    pub pending: Pending,
}

impl QuicLoginState {
    /// Begin a fresh login attempt: reset the seq counter, stash credentials, and
    /// arm the `Connecting` phase so `quic_send_hello` fires once the connection opens.
    pub fn start_connecting(&mut self, pending: Pending) {
        self.conn = QuicConnection::default();
        self.pending = pending;
        self.phase = LoginPhase::Connecting;
    }
}

/// Opens the QUIC connection to the aesir account server.
///
/// Dev cert handling: `SkipVerification` (self-signed). The send channels use
/// aesir's fixed order so channel ids line up; recv channels keep their defaults.
pub fn connect(
    client: &mut QuinnetClient,
    addr: &str,
) -> Result<ConnectionLocalId, AsyncChannelError> {
    let addr_config = ClientAddrConfiguration::from_strings(addr, "0.0.0.0:0")
        .expect("valid login server address");
    client.open_connection(ClientConnectionConfiguration {
        addr_config,
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: channels::send_channels_config(),
            ..Default::default()
        },
    })
}
