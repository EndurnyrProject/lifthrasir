use serde::Serialize;
use std::io;
use thiserror::Error;

#[derive(Debug, Error, Clone, Serialize)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Server refused login with code: {code}")]
    LoginRefused { code: u8 },

    #[error("Invalid packet received")]
    InvalidPacket,

    #[error("Packet parsing failed: {0}")]
    PacketParsingFailed(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Server disconnected unexpectedly")]
    UnexpectedDisconnect,

    #[error("Encryption/decryption failed")]
    EncryptionFailed,
}

impl From<io::Error> for NetworkError {
    fn from(error: io::Error) -> Self {
        NetworkError::ConnectionFailed(error.to_string())
    }
}

pub type NetworkResult<T> = Result<T, NetworkError>;
