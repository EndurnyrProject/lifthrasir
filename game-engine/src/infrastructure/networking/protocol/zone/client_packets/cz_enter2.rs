use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_ENTER2
pub const CZ_ENTER2: u16 = 0x0436;

/// CZ_ENTER2 (0x0436) - Client → Zone Server
///
/// Initial packet sent when entering the zone server.
/// This packet authenticates the player and requests entry to the game world.
///
/// # Packet Structure
/// ```text
/// Size: 23 bytes
/// +--------+-------------+----------+----------+-------------+----------+-----+
/// | Offset | Field       | Type     | Size     | Description              |
/// +--------+-------------+----------+----------+-------------+----------+-----+
/// | 0      | packet_id   | u16      | 2        | 0x0436                   |
/// | 2      | account_id  | u32      | 4        | Account ID               |
/// | 6      | char_id     | u32      | 4        | Character ID             |
/// | 10     | auth_code   | u32      | 4        | Auth code (login_id1)    |
/// | 14     | client_time | u32      | 4        | Client timestamp         |
/// | 18     | unknown     | u32      | 4        | Unknown (usually 0)      |
/// | 22     | sex         | u8       | 1        | Character sex (0=F, 1=M) |
/// +--------+-------------+----------+----------+-------------+----------+-----+
/// ```
#[derive(Debug, Clone)]
pub struct CzEnter2Packet {
    pub account_id: u32,
    pub char_id: u32,
    pub auth_code: u32,
    pub client_time: u32,
    pub unknown: u32,
    pub sex: u8,
}

impl CzEnter2Packet {
    /// Create a new CZ_ENTER2 packet
    pub fn new(account_id: u32, char_id: u32, auth_code: u32, client_time: u32, sex: u8) -> Self {
        Self {
            account_id,
            char_id,
            auth_code,
            client_time,
            unknown: 0,
            sex,
        }
    }
}

impl ClientPacket for CzEnter2Packet {
    const PACKET_ID: u16 = CZ_ENTER2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(23);
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.account_id);
        buf.put_u32_le(self.char_id);
        buf.put_u32_le(self.auth_code);
        buf.put_u32_le(self.client_time);
        buf.put_u32_le(self.unknown);
        buf.put_u8(self.sex);
        buf.freeze()
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cz_enter2_serialization() {
        let packet = CzEnter2Packet::new(12345, 67890, 11111, 22222, 1);

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 23, "Packet size should be 23 bytes");

        // Verify packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_ENTER2);

        // Verify account ID
        let account_id = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(account_id, 12345);

        // Verify sex
        assert_eq!(bytes[22], 1);
    }

    #[test]
    fn test_cz_enter2_packet_id() {
        let packet = CzEnter2Packet::new(1, 2, 3, 4, 0);
        assert_eq!(packet.packet_id(), CZ_ENTER2);
    }
}
