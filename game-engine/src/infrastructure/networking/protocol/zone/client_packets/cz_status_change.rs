use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_STATUS_CHANGE
pub const CZ_STATUS_CHANGE: u16 = 0x00BB;

/// CZ_STATUS_CHANGE (0x00BB) - Client → Zone Server
///
/// Requests raising a primary status (STR–LUK) by a given amount.
/// The server validates, applies the max affordable increase, persists, and
/// replies with ZC_STATUS_CHANGE_ACK plus authoritative ZC_PAR_CHANGE updates.
///
/// # Packet Structure
/// ```text
/// Size: 5 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x00BB                           |
/// | 2      | status_id   | u16      | 2    | Status parameter id (Str=13...)  |
/// | 4      | amount      | u8       | 1    | Number of points to raise        |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct CzStatusChangePacket {
    pub status_id: u16,
    pub amount: u8,
}

impl ClientPacket for CzStatusChangePacket {
    const PACKET_ID: u16 = CZ_STATUS_CHANGE;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u16_le(self.status_id);
        buf.put_u8(self.amount);
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
    fn test_status_change_serialization() {
        let packet = CzStatusChangePacket {
            status_id: 13,
            amount: 5,
        };

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 5, "Packet size should be 5 bytes");

        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_STATUS_CHANGE);

        let status_id = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(status_id, 13);

        assert_eq!(bytes[4], 5);
    }

    #[test]
    fn test_packet_id() {
        let packet = CzStatusChangePacket {
            status_id: 18,
            amount: 1,
        };
        assert_eq!(packet.packet_id(), CZ_STATUS_CHANGE);
    }
}
