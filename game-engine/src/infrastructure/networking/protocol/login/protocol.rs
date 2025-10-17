use super::{
    client_packets::{CaLoginPacket, CA_LOGIN},
    server_packets::{AcAcceptLoginPacket, AcRefuseLoginPacket, AC_ACCEPT_LOGIN, AC_REFUSE_LOGIN},
};
use crate::infrastructure::networking::protocol::traits::{
    ClientPacket, PacketSize, Protocol, ServerPacket,
};
use bytes::Bytes;
use std::io;

/// Login protocol definition
///
/// The login protocol handles authentication with the login server.
/// It's a simple request-response protocol:
/// 1. Client sends CA_LOGIN with credentials
/// 2. Server responds with either AC_ACCEPT_LOGIN or AC_REFUSE_LOGIN
pub struct LoginProtocol;

impl Protocol for LoginProtocol {
    const NAME: &'static str = "Login";

    type ClientPacket = LoginClientPacket;
    type ServerPacket = LoginServerPacket;
    type Context = LoginContext;

    fn parse_server_packet(packet_id: u16, data: &[u8]) -> io::Result<Self::ServerPacket> {
        match packet_id {
            AC_ACCEPT_LOGIN => {
                let packet = AcAcceptLoginPacket::parse(data)?;
                Ok(LoginServerPacket::AcAcceptLogin(packet))
            }
            AC_REFUSE_LOGIN => {
                let packet = AcRefuseLoginPacket::parse(data)?;
                Ok(LoginServerPacket::AcRefuseLogin(packet))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown login packet ID: 0x{:04X}", packet_id),
            )),
        }
    }

    fn packet_size(packet_id: u16) -> PacketSize {
        match packet_id {
            AC_ACCEPT_LOGIN => PacketSize::Variable {
                length_offset: 2,
                length_bytes: 2,
            },
            AC_REFUSE_LOGIN => PacketSize::Fixed(23),
            _ => PacketSize::Fixed(0), // Unknown
        }
    }
}

/// Context maintained during login protocol processing
///
/// Tracks login state including attempts, errors, and the username
/// for the current login session.
#[derive(Debug, Default)]
pub struct LoginContext {
    /// Number of login attempts made
    pub attempt_count: u32,

    /// Last error code received (if any)
    pub last_error: Option<u8>,

    /// Username for the current login session
    pub username: Option<String>,
}

/// Enum of all client packets for login protocol
#[derive(Debug, Clone)]
pub enum LoginClientPacket {
    CaLogin(CaLoginPacket),
    // Future: CtAuth for token-based authentication
}

impl ClientPacket for LoginClientPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn serialize(&self) -> Bytes {
        match self {
            Self::CaLogin(p) => p.serialize(),
        }
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::CaLogin(_) => CA_LOGIN,
        }
    }
}

/// Enum of all server packets for login protocol
#[derive(Debug, Clone)]
pub enum LoginServerPacket {
    AcAcceptLogin(AcAcceptLoginPacket),
    AcRefuseLogin(AcRefuseLoginPacket),
}

impl ServerPacket for LoginServerPacket {
    const PACKET_ID: u16 = 0; // Not used for enums

    fn parse(_data: &[u8]) -> io::Result<Self> {
        unreachable!("Use Protocol::parse_server_packet instead")
    }

    fn packet_id(&self) -> u16 {
        match self {
            Self::AcAcceptLogin(_) => AC_ACCEPT_LOGIN,
            Self::AcRefuseLogin(_) => AC_REFUSE_LOGIN,
        }
    }
}

/// Convenience methods for LoginContext
impl LoginContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a login attempt with the username
    pub fn record_attempt(&mut self, username: String) {
        self.attempt_count += 1;
        self.username = Some(username);
    }

    pub fn record_error(&mut self, error_code: u8) {
        self.last_error = Some(error_code);
    }

    pub fn reset(&mut self) {
        self.attempt_count = 0;
        self.last_error = None;
        self.username = None;
    }
}
