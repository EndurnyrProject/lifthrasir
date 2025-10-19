use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_REQUEST_MOVE2
pub const CZ_REQUEST_MOVE2: u16 = 0x035F;

/// CZ_REQUEST_MOVE2 (0x035F) - Client → Zone Server
///
/// Requests character movement to a specific position.
/// The server will validate the movement and respond with either
/// ZC_NOTIFY_PLAYERMOVE or ZC_NOTIFY_MOVE_STOP.
///
/// # Packet Structure
/// ```text
/// Size: 5 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x035F                           |
/// | 2      | position    | u8[3]    | 3    | Encoded position (x, y, dir)     |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
///
/// Position encoding (3 bytes):
/// ```text
/// Byte 0: X[9:2]
/// Byte 1: X[1:0] Y[9:4]
/// Byte 2: Y[3:0] Dir[3:0]
/// ```
#[derive(Debug, Clone)]
pub struct CzRequestMove2Packet {
    pub x: u16,
    pub y: u16,
    pub dir: u8,
}

impl CzRequestMove2Packet {
    /// Create a new CZ_REQUEST_MOVE2 packet
    pub fn new(x: u16, y: u16, dir: u8) -> Self {
        Self { x, y, dir }
    }

    /// Encode position and direction into 3 bytes
    ///
    /// Format: X and Y are 10-bit values, direction is 4-bit
    /// ```text
    /// Byte 0: X[9:2]
    /// Byte 1: X[1:0] Y[9:4]
    /// Byte 2: Y[3:0] Dir[3:0]
    /// ```
    fn encode_position(&self) -> [u8; 3] {
        let byte0 = (self.x >> 2) as u8;
        let byte1 = (((self.x << 6) | ((self.y >> 4) & 0x3F)) & 0xFF) as u8;
        let byte2 = (((self.y << 4) | (self.dir as u16 & 0x0F)) & 0xFF) as u8;
        [byte0, byte1, byte2]
    }
}

impl ClientPacket for CzRequestMove2Packet {
    const PACKET_ID: u16 = CZ_REQUEST_MOVE2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u16_le(Self::PACKET_ID);

        let encoded_pos = self.encode_position();
        buf.put_slice(&encoded_pos);

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
    fn test_cz_request_move2_serialization() {
        let packet = CzRequestMove2Packet::new(100, 200, 3);

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 5, "Packet size should be 5 bytes");

        // Verify packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_REQUEST_MOVE2);
    }

    #[test]
    fn test_position_encoding() {
        let test_cases = vec![
            (100, 200, 3),
            (0, 0, 0),
            (1023, 1023, 15), // Max values (10-bit for x/y, 4-bit for dir)
            (512, 512, 7),
            (1, 1, 1),
        ];

        for (x, y, dir) in test_cases {
            let packet = CzRequestMove2Packet::new(x, y, dir);
            let encoded = packet.encode_position();

            // Decode to verify
            let decoded_x = ((encoded[0] as u16) << 2) | ((encoded[1] as u16) >> 6);
            let decoded_y = (((encoded[1] as u16) & 0x3F) << 4) | ((encoded[2] as u16) >> 4);
            let decoded_dir = encoded[2] & 0x0F;

            assert_eq!(
                decoded_x, x,
                "X coordinate mismatch for ({}, {}, {})",
                x, y, dir
            );
            assert_eq!(
                decoded_y, y,
                "Y coordinate mismatch for ({}, {}, {})",
                x, y, dir
            );
            assert_eq!(
                decoded_dir, dir,
                "Direction mismatch for ({}, {}, {})",
                x, y, dir
            );
        }
    }

    #[test]
    fn test_cz_request_move2_packet_id() {
        let packet = CzRequestMove2Packet::new(100, 100, 0);
        assert_eq!(packet.packet_id(), CZ_REQUEST_MOVE2);
    }
}
