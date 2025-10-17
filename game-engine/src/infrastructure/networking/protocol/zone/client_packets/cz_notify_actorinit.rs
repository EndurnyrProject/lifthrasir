use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_NOTIFY_ACTORINIT
pub const CZ_NOTIFY_ACTORINIT: u16 = 0x007D;

/// CZ_NOTIFY_ACTORINIT (0x007D) - Client → Zone Server
///
/// Notifies the zone server that the client is ready to receive actor information.
/// This is sent after successfully entering the zone to signal readiness for
/// receiving NPC, monster, and other player data.
///
/// # Packet Structure
/// ```text
/// Size: 2 bytes (just packet ID, no additional data)
/// +--------+-------------+----------+----------+-------------+
/// | Offset | Field       | Type     | Size     | Description |
/// +--------+-------------+----------+----------+-------------+
/// | 0      | packet_id   | u16      | 2        | 0x007D      |
/// +--------+-------------+----------+----------+-------------+
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CzNotifyActorinitPacket;

impl CzNotifyActorinitPacket {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CzNotifyActorinitPacket {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientPacket for CzNotifyActorinitPacket {
    const PACKET_ID: u16 = CZ_NOTIFY_ACTORINIT;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(2);
        buf.put_u16_le(Self::PACKET_ID);
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
    fn test_cz_notify_actorinit_serialization() {
        let packet = CzNotifyActorinitPacket::new();
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 2, "Packet size should be 2 bytes");

        // Verify packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_NOTIFY_ACTORINIT);
    }

    #[test]
    fn test_cz_notify_actorinit_packet_id() {
        let packet = CzNotifyActorinitPacket::new();
        assert_eq!(packet.packet_id(), CZ_NOTIFY_ACTORINIT);
    }
}
