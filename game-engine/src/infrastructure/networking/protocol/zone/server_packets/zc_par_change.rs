use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_PAR_CHANGE
pub const ZC_PAR_CHANGE: u16 = 0x00B0;

/// ZC_PAR_CHANGE (0x00B0) - Zone Server → Client
///
/// Notifies client of a character parameter change.
/// This is the primary packet for updating character status values like HP, SP,
/// experience, weight, stats, etc. Used for most numeric status updates.
///
/// # Packet Structure
/// ```text
/// Size: 8 bytes
/// +--------+-------------+----------+----------+---------------------------+
/// | Offset | Field       | Type     | Size     | Description               |
/// +--------+-------------+----------+----------+---------------------------+
/// | 0      | packet_id   | u16      | 2        | 0x00B0                    |
/// | 2      | var_id      | u16      | 2        | Status parameter ID       |
/// | 4      | value       | u32      | 4        | New value for parameter   |
/// +--------+-------------+----------+----------+---------------------------+
/// ```
///
/// # Status Parameter IDs
/// Common var_id values include:
/// - 0x0005: Max HP
/// - 0x0006: Max SP
/// - 0x0007: Current HP
/// - 0x0008: Current SP
/// - 0x0009: Status points
/// - 0x000B: Base level
/// - 0x0018: Weight
/// - 0x0019: Max weight
#[derive(Debug, Clone)]
pub struct ZcParChangePacket {
    /// Status parameter ID
    pub var_id: u16,
    /// New value for the parameter
    pub value: u32,
}

impl ServerPacket for ZcParChangePacket {
    const PACKET_ID: u16 = ZC_PAR_CHANGE;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_PAR_CHANGE packet too short: expected 8 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let var_id = buf.get_u16_le();
        let value = buf.get_u32_le();

        Ok(Self { var_id, value })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_par_change_parse() {
        let mut data = vec![0u8; 8];

        data[0..2].copy_from_slice(&ZC_PAR_CHANGE.to_le_bytes());

        data[2..4].copy_from_slice(&0x0007u16.to_le_bytes());

        data[4..8].copy_from_slice(&1500u32.to_le_bytes());

        let packet = ZcParChangePacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.var_id, 0x0007);
        assert_eq!(packet.value, 1500);
    }

    #[test]
    fn test_zc_par_change_parse_invalid_size() {
        let data = vec![0u8; 5];
        let result = ZcParChangePacket::parse(&data);
        assert!(result.is_err());
    }
}
