use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_NOTIFY_TIME2
pub const ZC_NOTIFY_TIME2: u16 = 0x02C2;

/// ZC_NOTIFY_TIME2 (0x02C2) - Zone Server → Client
///
/// Sends the server's current time in response to CZ_REQUEST_TIME2.
/// The client uses this to calculate the time offset between client and server
/// for accurate synchronization of movements, animations, and other time-based events.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+----------+------------------+
/// | Offset | Field       | Type     | Size     | Description      |
/// +--------+-------------+----------+----------+------------------+
/// | 0      | packet_id   | u16      | 2        | 0x02C2           |
/// | 2      | server_time | u32      | 4        | Server time (ms) |
/// +--------+-------------+----------+----------+------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcNotifyTime2Packet {
    pub server_time: u32,
}

impl ServerPacket for ZcNotifyTime2Packet {
    const PACKET_ID: u16 = ZC_NOTIFY_TIME2;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_TIME2 packet too short: expected 6 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read server time
        let server_time = buf.get_u32_le();

        Ok(Self { server_time })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_notify_time2_parse() {
        let mut data = vec![0u8; 6];

        // Packet ID: 0x02C2
        data[0..2].copy_from_slice(&ZC_NOTIFY_TIME2.to_le_bytes());

        // Server time: 987654321
        data[2..6].copy_from_slice(&987654321u32.to_le_bytes());

        let packet = ZcNotifyTime2Packet::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.server_time, 987654321);
    }

    #[test]
    fn test_zc_notify_time2_parse_invalid_size() {
        let data = vec![0u8; 3]; // Too short
        let result = ZcNotifyTime2Packet::parse(&data);
        assert!(result.is_err());
    }
}
