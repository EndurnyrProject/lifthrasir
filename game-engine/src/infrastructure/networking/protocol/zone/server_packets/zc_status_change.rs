use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_STATUS_CHANGE_ACK
pub const ZC_STATUS_CHANGE_ACK: u16 = 0x00BC;

/// ZC_STATUS_CHANGE_ACK (0x00BC) - Zone Server → Client
///
/// Result of a stat-raise request (the reply to CZ_STATUS_CHANGE). The
/// authoritative new stat value also arrives via ZC_PAR_CHANGE, so this packet
/// is primarily the success/failure acknowledgement.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x00BC                           |
/// | 2      | sp          | u16      | 2    | Status parameter id (Str=13...)  |
/// | 4      | ok          | u8       | 1    | 0 = failure, 1 = success         |
/// | 5      | value       | u8       | 1    | New stat value (capped to 255)   |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcStatusChangeAckPacket {
    /// Status parameter id that was changed
    pub sp: u16,
    /// Whether the raise succeeded
    pub ok: u8,
    /// New stat value (capped to 255 by the server)
    pub value: u8,
}

impl ServerPacket for ZcStatusChangeAckPacket {
    const PACKET_ID: u16 = ZC_STATUS_CHANGE_ACK;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_STATUS_CHANGE_ACK packet too short: expected 6 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let sp = buf.get_u16_le();
        let ok = buf.get_u8();
        let value = buf.get_u8();

        Ok(Self { sp, ok, value })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_status_change_ack_parse() {
        let mut data = vec![0u8; 6];

        data[0..2].copy_from_slice(&ZC_STATUS_CHANGE_ACK.to_le_bytes());

        data[2..4].copy_from_slice(&13u16.to_le_bytes());

        data[4] = 1;
        data[5] = 42;

        let packet = ZcStatusChangeAckPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.sp, 13);
        assert_eq!(packet.ok, 1);
        assert_eq!(packet.value, 42);
    }

    #[test]
    fn test_zc_status_change_ack_parse_invalid_size() {
        let data = vec![0u8; 4];
        let result = ZcStatusChangeAckPacket::parse(&data);
        assert!(result.is_err());
    }
}
