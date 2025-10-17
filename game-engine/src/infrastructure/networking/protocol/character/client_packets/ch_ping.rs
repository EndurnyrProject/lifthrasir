use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_PING: u16 = 0x0187;
const PACKET_SIZE: usize = 6;

/// CH_PING (0x0187) - Keep-alive ping
///
/// Sends a ping to keep the connection alive.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Account ID: u32 (4 bytes, typically 0 as placeholder)
///
/// Total: 6 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChPingPacket {
    pub account_id: u32,
}

impl ChPingPacket {
    /// Create a new CH_PING packet
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID (typically 0)
    pub fn new(account_id: u32) -> Self {
        Self { account_id }
    }

    /// Create a ping packet with default account ID (0)
    pub fn default() -> Self {
        Self { account_id: 0 }
    }
}

impl ClientPacket for ChPingPacket {
    const PACKET_ID: u16 = CH_PING;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.account_id);

        debug_assert_eq!(buf.len(), PACKET_SIZE, "CH_PING packet size mismatch");

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_ping_serialization() {
        let packet = ChPingPacket::new(0);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_PING);
        assert_eq!(u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]), 0);
    }

    #[test]
    fn test_ch_ping_default() {
        let packet = ChPingPacket::default();
        assert_eq!(packet.account_id, 0);
    }

    #[test]
    fn test_ch_ping_packet_id() {
        let packet = ChPingPacket::new(123);
        assert_eq!(packet.packet_id(), CH_PING);
    }
}
