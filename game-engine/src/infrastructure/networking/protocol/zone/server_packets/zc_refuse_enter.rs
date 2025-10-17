use crate::infrastructure::networking::protocol::{
    traits::ServerPacket, zone::types::ZoneEntryError,
};
use bytes::Buf;
use std::io;

/// Packet ID for ZC_REFUSE_ENTER
pub const ZC_REFUSE_ENTER: u16 = 0x0074;

/// ZC_REFUSE_ENTER (0x0074) - Zone Server → Client
///
/// Sent when the zone server refuses entry to the client.
/// Contains an error code indicating the reason for refusal.
///
/// # Packet Structure
/// ```text
/// Size: 3 bytes
/// +--------+-------------+----------+----------+------------------------+
/// | Offset | Field       | Type     | Size     | Description            |
/// +--------+-------------+----------+----------+------------------------+
/// | 0      | packet_id   | u16      | 2        | 0x0074                 |
/// | 2      | error_code  | u8       | 1        | Error code (see below) |
/// +--------+-------------+----------+----------+------------------------+
/// ```
///
/// # Error Codes
/// - 0: Normal (no error - shouldn't be in refuse packet)
/// - 1: Server closed
/// - 2: Someone has already logged in with this ID
/// - 3: Already logged in
/// - 4: Environment error
/// - 8: Server still recognizes last connection
#[derive(Debug, Clone)]
pub struct ZcRefuseEnterPacket {
    pub error: ZoneEntryError,
}

impl ZcRefuseEnterPacket {
    /// Get human-readable error description
    pub fn error_description(&self) -> &'static str {
        self.error.description()
    }
}

impl ServerPacket for ZcRefuseEnterPacket {
    const PACKET_ID: u16 = ZC_REFUSE_ENTER;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_REFUSE_ENTER packet too short: expected 3 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read error code
        let error_code = buf.get_u8();
        let error = ZoneEntryError::from(error_code);

        Ok(Self { error })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_refuse_enter_parse() {
        let mut data = vec![0u8; 3];

        // Packet ID: 0x0074
        data[0..2].copy_from_slice(&ZC_REFUSE_ENTER.to_le_bytes());

        // Error code: 1 (Server closed)
        data[2] = 1;

        let packet = ZcRefuseEnterPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.error, ZoneEntryError::ServerClosed);
        assert_eq!(packet.error_description(), "Server closed");
    }

    #[test]
    fn test_zc_refuse_enter_parse_all_error_codes() {
        let test_cases = vec![
            (0, ZoneEntryError::Normal),
            (1, ZoneEntryError::ServerClosed),
            (2, ZoneEntryError::AlreadyLoggedIn),
            (3, ZoneEntryError::AlreadyLoggedInAlt),
            (4, ZoneEntryError::EnvironmentError),
            (8, ZoneEntryError::PreviousConnectionActive),
        ];

        for (error_code, expected_error) in test_cases {
            let mut data = vec![0u8; 3];
            data[0..2].copy_from_slice(&ZC_REFUSE_ENTER.to_le_bytes());
            data[2] = error_code;

            let packet = ZcRefuseEnterPacket::parse(&data).expect("Failed to parse packet");
            assert_eq!(packet.error, expected_error);
        }
    }

    #[test]
    fn test_zc_refuse_enter_parse_invalid_size() {
        let data = vec![0u8; 1]; // Too short
        let result = ZcRefuseEnterPacket::parse(&data);
        assert!(result.is_err());
    }
}
