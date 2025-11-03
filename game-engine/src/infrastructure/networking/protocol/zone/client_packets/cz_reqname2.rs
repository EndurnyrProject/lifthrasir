use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CZ_REQNAME2: u16 = 0x0368;

/// CZ_REQNAME2 (0x0368) - Client → Zone Server
///
/// Requests the name and details of an entity (player, NPC, or mob).
/// The server will respond with either ZC_ACK_REQNAME (0x0095) for basic name
/// or ZC_ACK_REQNAMEALL (0x0195) for full details including party/guild info.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x0368                           |
/// | 2      | entity_id   | u32      | 4    | Account ID of target entity      |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct CzReqname2Packet {
    pub entity_id: u32,
}

impl CzReqname2Packet {
    pub fn new(entity_id: u32) -> Self {
        Self { entity_id }
    }
}

impl ClientPacket for CzReqname2Packet {
    const PACKET_ID: u16 = CZ_REQNAME2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(6);
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.entity_id);
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
    fn test_cz_reqname2_serialization() {
        let packet = CzReqname2Packet::new(123456);

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 6, "Packet size should be 6 bytes");

        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_REQNAME2);

        let entity_id = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(entity_id, 123456);
    }

    #[test]
    fn test_cz_reqname2_packet_id() {
        let packet = CzReqname2Packet::new(0);
        assert_eq!(packet.packet_id(), CZ_REQNAME2);
    }
}
