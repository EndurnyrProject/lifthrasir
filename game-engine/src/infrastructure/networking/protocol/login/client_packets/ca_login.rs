use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CA_LOGIN: u16 = 0x0064;
const USERNAME_MAX_BYTES: usize = 24;
const PASSWORD_MAX_BYTES: usize = 24;
const PACKET_SIZE: usize = 55;

/// CA_LOGIN (0x0064) - Client login request
///
/// Sends username, password, and client version to the login server.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Version: u32 (4 bytes)
/// - Username: [u8; 24] (null-padded)
/// - Password: [u8; 24] (null-padded)
/// - Client Type: u8 (1 byte)
///
/// Total: 55 bytes
///
/// # Direction
/// Client → Login Server
#[derive(Debug, Clone)]
pub struct CaLoginPacket {
    pub version: u32,
    pub username: String,
    pub password: String,
    pub client_type: u8,
}

impl CaLoginPacket {
    /// Create a new login packet
    ///
    /// # Arguments
    ///
    /// * `username` - Account username
    /// * `password` - Account password
    /// * `version` - Client version number
    ///
    /// # Example
    ///
    /// ```ignore
    /// let packet = CaLoginPacket::new("testuser", "testpass", 55);
    /// ```
    pub fn new(username: &str, password: &str, version: u32) -> Self {
        Self {
            version,
            username: username.to_string(),
            password: password.to_string(),
            client_type: 0, // Default client type
        }
    }
}

impl ClientPacket for CaLoginPacket {
    const PACKET_ID: u16 = CA_LOGIN;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        // Packet ID (2 bytes, little-endian)
        buf.put_u16_le(Self::PACKET_ID);

        // Version (4 bytes, little-endian)
        buf.put_u32_le(self.version);

        // Username (24 bytes, null-padded)
        let mut username_bytes = [0u8; USERNAME_MAX_BYTES];
        let username_data = self.username.as_bytes();
        let copy_len = username_data.len().min(USERNAME_MAX_BYTES - 1); // Leave space for null terminator
        username_bytes[..copy_len].copy_from_slice(&username_data[..copy_len]);
        buf.put_slice(&username_bytes);

        // Password (24 bytes, null-padded)
        let mut password_bytes = [0u8; PASSWORD_MAX_BYTES];
        let password_data = self.password.as_bytes();
        let copy_len = password_data.len().min(PASSWORD_MAX_BYTES - 1);
        password_bytes[..copy_len].copy_from_slice(&password_data[..copy_len]);
        buf.put_slice(&password_bytes);

        // Client type (1 byte)
        buf.put_u8(self.client_type);

        debug_assert_eq!(buf.len(), PACKET_SIZE, "CA_LOGIN packet size mismatch");

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_login_serialization() {
        let packet = CaLoginPacket::new("testuser", "testpass", 55);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 55);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CA_LOGIN);
        assert_eq!(
            u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            55
        );
    }

    #[test]
    fn test_ca_login_username_truncation() {
        let long_username = "a".repeat(50);
        let packet = CaLoginPacket::new(&long_username, "pass", 55);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 55);
    }

    #[test]
    fn test_ca_login_packet_id() {
        let packet = CaLoginPacket::new("user", "pass", 55);
        assert_eq!(packet.packet_id(), CA_LOGIN);
    }
}
