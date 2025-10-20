use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_LONGPAR_CHANGE
pub const ZC_LONGPAR_CHANGE: u16 = 0x00B1;

/// ZC_LONGPAR_CHANGE (0x00B1) - Zone Server → Client
///
/// Notifies client of a character parameter change (long value).
/// Similar to ZC_PAR_CHANGE but semantically used for larger values that may
/// require more precision, such as large experience values or zeny amounts.
///
/// # Packet Structure
/// ```text
/// Size: 8 bytes
/// +--------+-------------+----------+----------+---------------------------+
/// | Offset | Field       | Type     | Size     | Description               |
/// +--------+-------------+----------+----------+---------------------------+
/// | 0      | packet_id   | u16      | 2        | 0x00B1                    |
/// | 2      | var_id      | u16      | 2        | Status parameter ID       |
/// | 4      | value       | u32      | 4        | New value for parameter   |
/// +--------+-------------+----------+----------+---------------------------+
/// ```
///
/// # Status Parameter IDs
/// Common var_id values include:
/// - 0x0001: Base experience
/// - 0x0002: Job experience
/// - 0x0014: Zeny (currency)
/// - 0x0016: Base experience needed for next level
/// - 0x0017: Job experience needed for next level
///
/// # Note
/// Despite the name "long", this packet still uses 32-bit values in the current
/// packet version. The distinction is primarily for semantic clarity and future
/// compatibility with values that may exceed smaller integer limits.
#[derive(Debug, Clone)]
pub struct ZcLongparChangePacket {
    /// Status parameter ID
    pub var_id: u16,
    /// New value for the parameter
    pub value: u32,
}

impl ServerPacket for ZcLongparChangePacket {
    const PACKET_ID: u16 = ZC_LONGPAR_CHANGE;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_LONGPAR_CHANGE packet too short: expected 8 bytes, got {}",
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
    fn test_zc_longpar_change_parse() {
        let mut data = vec![0u8; 8];

        data[0..2].copy_from_slice(&ZC_LONGPAR_CHANGE.to_le_bytes());

        data[2..4].copy_from_slice(&0x0001u16.to_le_bytes());

        data[4..8].copy_from_slice(&100000u32.to_le_bytes());

        let packet = ZcLongparChangePacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.var_id, 0x0001);
        assert_eq!(packet.value, 100000);
    }

    #[test]
    fn test_zc_longpar_change_parse_invalid_size() {
        let data = vec![0u8; 5];
        let result = ZcLongparChangePacket::parse(&data);
        assert!(result.is_err());
    }
}
