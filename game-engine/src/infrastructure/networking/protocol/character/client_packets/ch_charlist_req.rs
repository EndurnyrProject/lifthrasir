use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_CHARLIST_REQ: u16 = 0x09A1;
const PACKET_SIZE: usize = 2;

/// CH_CHARLIST_REQ (0x09A1) - Request character list
///
/// Requests the character list from the server.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
///
/// Total: 2 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone, Copy)]
pub struct ChCharlistReqPacket;

impl ChCharlistReqPacket {
    /// Create a new CH_CHARLIST_REQ packet
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChCharlistReqPacket {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientPacket for ChCharlistReqPacket {
    const PACKET_ID: u16 = CH_CHARLIST_REQ;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);

        debug_assert_eq!(
            buf.len(),
            PACKET_SIZE,
            "CH_CHARLIST_REQ packet size mismatch"
        );

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_charlist_req_serialization() {
        let packet = ChCharlistReqPacket::new();
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_CHARLIST_REQ);
    }

    #[test]
    fn test_ch_charlist_req_packet_id() {
        let packet = ChCharlistReqPacket::new();
        assert_eq!(packet.packet_id(), CH_CHARLIST_REQ);
    }

    #[test]
    fn test_ch_charlist_req_default() {
        let packet = ChCharlistReqPacket::default();
        let bytes = packet.serialize();
        assert_eq!(bytes.len(), PACKET_SIZE);
    }
}
