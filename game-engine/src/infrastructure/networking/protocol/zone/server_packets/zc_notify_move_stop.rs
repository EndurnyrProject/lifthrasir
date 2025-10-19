use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_NOTIFY_MOVE_STOP
pub const ZC_NOTIFY_MOVE_STOP: u16 = 0x0088;

/// ZC_NOTIFY_MOVE_STOP (0x0088) - Zone Server → Client
///
/// **MULTI-CHARACTER READY** - This packet includes an Account ID field,
/// making it suitable for stopping movement of any character (local player,
/// other players, or NPCs).
///
/// Forces the client to stop character movement immediately.
/// This can be sent when:
/// - Movement is interrupted by combat
/// - Server detects invalid movement
/// - Character is stunned or frozen
/// - Any other server-side movement cancellation
///
/// # Multi-Character Support
///
/// Unlike `ZC_NOTIFY_PLAYERMOVE`, this packet identifies which character stopped
/// via the `account_id` field. When CharacterRegistry is implemented, the handler
/// can look up the correct entity and stop its movement. See movement module docs.
///
/// # Packet Structure
/// ```text
/// Size: 10 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x0088                           |
/// | 2      | account_id  | u32      | 4    | Account ID                       |
/// | 6      | x           | u16      | 2    | Final X position                 |
/// | 8      | y           | u16      | 2    | Final Y position                 |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcNotifyMoveStopPacket {
    pub account_id: u32,
    pub x: u16,
    pub y: u16,
}

impl ZcNotifyMoveStopPacket {
    /// Create a new ZC_NOTIFY_MOVE_STOP packet
    pub fn new(account_id: u32, x: u16, y: u16) -> Self {
        Self { account_id, x, y }
    }
}

impl ServerPacket for ZcNotifyMoveStopPacket {
    const PACKET_ID: u16 = ZC_NOTIFY_MOVE_STOP;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 10 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_MOVE_STOP packet too short: expected 10 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read account ID
        let account_id = buf.get_u32_le();

        // Read position
        let x = buf.get_u16_le();
        let y = buf.get_u16_le();

        Ok(Self { account_id, x, y })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_notify_move_stop_parse() {
        // Create test data
        let mut data = vec![0u8; 10];

        // Packet ID: 0x0088
        data[0..2].copy_from_slice(&ZC_NOTIFY_MOVE_STOP.to_le_bytes());

        // Account ID: 999888
        data[2..6].copy_from_slice(&999888u32.to_le_bytes());

        // Position: (123, 456)
        data[6..8].copy_from_slice(&123u16.to_le_bytes());
        data[8..10].copy_from_slice(&456u16.to_le_bytes());

        let packet = ZcNotifyMoveStopPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.account_id, 999888);
        assert_eq!(packet.x, 123);
        assert_eq!(packet.y, 456);
    }

    #[test]
    fn test_zc_notify_move_stop_parse_invalid_size() {
        let data = vec![0u8; 6]; // Too short
        let result = ZcNotifyMoveStopPacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_zc_notify_move_stop_packet_id() {
        let packet = ZcNotifyMoveStopPacket::new(12345, 100, 200);
        assert_eq!(packet.packet_id(), ZC_NOTIFY_MOVE_STOP);
    }

    #[test]
    fn test_boundary_values() {
        let packet = ZcNotifyMoveStopPacket::new(0, 0, 0);
        assert_eq!(packet.account_id, 0);
        assert_eq!(packet.x, 0);
        assert_eq!(packet.y, 0);

        let packet = ZcNotifyMoveStopPacket::new(u32::MAX, 1023, 1023);
        assert_eq!(packet.account_id, u32::MAX);
        assert_eq!(packet.x, 1023);
        assert_eq!(packet.y, 1023);
    }
}
