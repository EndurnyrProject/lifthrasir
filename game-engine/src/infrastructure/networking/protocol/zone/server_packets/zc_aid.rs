use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_AID
pub const ZC_AID: u16 = 0x0283;

/// ZC_AID (0x0283) - Zone Server → Client
///
/// Sends the account ID to the client after accepting entry.
/// This is a simple confirmation packet.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+----------+-------------+
/// | Offset | Field       | Type     | Size     | Description |
/// +--------+-------------+----------+----------+-------------+
/// | 0      | packet_id   | u16      | 2        | 0x0283      |
/// | 2      | account_id  | u32      | 4        | Account ID  |
/// +--------+-------------+----------+----------+-------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcAidPacket {
    pub account_id: u32,
}

impl ServerPacket for ZcAidPacket {
    const PACKET_ID: u16 = ZC_AID;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_AID packet too short: expected 6 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read account ID
        let account_id = buf.get_u32_le();

        Ok(Self { account_id })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_aid_parse() {
        let mut data = vec![0u8; 6];

        // Packet ID: 0x0283
        data[0..2].copy_from_slice(&ZC_AID.to_le_bytes());

        // Account ID: 123456
        data[2..6].copy_from_slice(&123456u32.to_le_bytes());

        let packet = ZcAidPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.account_id, 123456);
    }

    #[test]
    fn test_zc_aid_parse_invalid_size() {
        let data = vec![0u8; 3]; // Too short
        let result = ZcAidPacket::parse(&data);
        assert!(result.is_err());
    }
}
