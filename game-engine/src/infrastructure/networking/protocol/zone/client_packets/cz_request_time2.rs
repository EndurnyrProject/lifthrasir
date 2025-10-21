use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_REQUEST_TIME2
pub const CZ_REQUEST_TIME2: u16 = 0x0360;

/// CZ_REQUEST_TIME2 (0x0360) - Client → Zone Server
///
/// Requests the current server time for client-server time synchronization.
/// The client sends its local time, and the server responds with ZC_NOTIFY_TIME2
/// containing the server's current time. The client can then calculate the time
/// offset to synchronize animations, movements, and other time-based events.
///
/// # Packet Structure
/// ```text
/// Size: 6 bytes
/// +--------+-------------+----------+----------+------------------+
/// | Offset | Field       | Type     | Size     | Description      |
/// +--------+-------------+----------+----------+------------------+
/// | 0      | packet_id   | u16      | 2        | 0x0360           |
/// | 2      | client_time | u32      | 4        | Client time (ms) |
/// +--------+-------------+----------+----------+------------------+
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CzRequestTime2Packet {
    pub client_time: u32,
}

impl CzRequestTime2Packet {
    pub fn new(client_time: u32) -> Self {
        Self { client_time }
    }
}

impl ClientPacket for CzRequestTime2Packet {
    const PACKET_ID: u16 = CZ_REQUEST_TIME2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(6);
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.client_time);
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
    fn test_cz_request_time2_serialization() {
        let client_time = 123456789u32;
        let packet = CzRequestTime2Packet::new(client_time);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 6, "Packet size should be 6 bytes");

        // Verify packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_REQUEST_TIME2);

        // Verify client_time
        let parsed_time = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(parsed_time, client_time);
    }

    #[test]
    fn test_cz_request_time2_packet_id() {
        let packet = CzRequestTime2Packet::new(0);
        assert_eq!(packet.packet_id(), CZ_REQUEST_TIME2);
    }
}
