use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_NOTIFY_TIME
pub const ZC_NOTIFY_TIME: u16 = 0x007F;

/// ZC_NOTIFY_TIME (0x007F) - Zone Server → Client
///
/// Sends the server's current time in response to CZ_REQUEST_TIME (0x0360).
/// The client uses this to calculate the time offset between client and server
/// for accurate synchronization of movements, animations, and other time-based events.
///
/// This is the legacy version of ZC_NOTIFY_TIME2 (0x02C2) but has the same structure.
/// Different RO server implementations may use either one.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+----------+------------------+
/// | Offset | Field       | Type     | Size     | Description      |
/// +--------+-------------+----------+----------+------------------+
/// | 0      | packet_id   | u16      | 2        | 0x007F           |
/// | 2      | server_tick | u32      | 4        | Server time (ms) |
/// +--------+-------------+----------+----------+------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcNotifyTimePacket {
    pub server_tick: u32,
}

impl ServerPacket for ZcNotifyTimePacket {
    const PACKET_ID: u16 = ZC_NOTIFY_TIME;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_TIME packet too short: expected 6 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read server tick
        let server_tick = buf.get_u32_le();

        Ok(Self { server_tick })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_notify_time_parse() {
        let mut data = vec![0u8; 6];

        // Packet ID: 0x007F
        data[0..2].copy_from_slice(&ZC_NOTIFY_TIME.to_le_bytes());

        // Server tick: 987654321
        data[2..6].copy_from_slice(&987654321u32.to_le_bytes());

        let packet = ZcNotifyTimePacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.server_tick, 987654321);
    }

    #[test]
    fn test_zc_notify_time_parse_invalid_size() {
        let data = vec![0u8; 3]; // Too short
        let result = ZcNotifyTimePacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_zc_notify_time_packet_id() {
        let packet = ZcNotifyTimePacket {
            server_tick: 123456,
        };
        assert_eq!(packet.packet_id(), 0x007F);
    }
}
