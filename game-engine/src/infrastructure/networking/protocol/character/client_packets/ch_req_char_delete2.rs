use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_REQ_CHAR_DELETE2: u16 = 0x0827;
const PACKET_SIZE: usize = 6;

/// CH_REQ_CHAR_DELETE2 (0x0827) - Request character deletion with timer
///
/// Modern character deletion request. The server marks the character for
/// deletion after a delay and replies with HC_CHAR_DELETE2_ACK (0x0828).
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character ID: u32 (4 bytes)
///
/// Total: 6 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChReqCharDelete2Packet {
    pub char_id: u32,
}

impl ChReqCharDelete2Packet {
    /// Create a new CH_REQ_CHAR_DELETE2 packet
    ///
    /// # Arguments
    ///
    /// * `char_id` - ID of character to delete
    pub fn new(char_id: u32) -> Self {
        Self { char_id }
    }
}

impl ClientPacket for ChReqCharDelete2Packet {
    const PACKET_ID: u16 = CH_REQ_CHAR_DELETE2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.char_id);

        debug_assert_eq!(
            buf.len(),
            PACKET_SIZE,
            "CH_REQ_CHAR_DELETE2 packet size mismatch"
        );

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_req_char_delete2_serialization() {
        let packet = ChReqCharDelete2Packet::new(150000);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            CH_REQ_CHAR_DELETE2
        );
        assert_eq!(
            u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            150000
        );
    }

    #[test]
    fn test_ch_req_char_delete2_packet_id() {
        let packet = ChReqCharDelete2Packet::new(1);
        assert_eq!(packet.packet_id(), CH_REQ_CHAR_DELETE2);
    }
}
