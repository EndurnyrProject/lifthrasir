use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

/// Packet ID for ZC_NOTIFY_PLAYERMOVE
pub const ZC_NOTIFY_PLAYERMOVE: u16 = 0x0087;

/// ZC_NOTIFY_PLAYERMOVE (0x0087) - Zone Server → Client
///
/// **LOCAL PLAYER MOVEMENT ONLY** - This packet does NOT include an Account ID
/// or entity identifier. It is implicitly for the local player character.
///
/// Sent by the server to confirm and synchronize the local player's movement.
/// Contains the source position, destination position, and server timing
/// for smooth client-side interpolation.
///
/// # Multi-Character Movement
///
/// For other players and NPCs on screen, the server uses `ZC_NOTIFY_MOVE` (0x007B)
/// which includes an Account ID field. That packet is not yet implemented.
/// See the movement module documentation for multi-character architecture.
///
/// # Packet Structure
/// ```text
/// Size: 12 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x0087                           |
/// | 2      | server_tick | u32      | 4    | Server timestamp for sync        |
/// | 6      | move_data   | u8[6]    | 6    | Encoded movement data            |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
///
/// Movement data encoding (6 bytes):
/// ```text
/// byte0: x0[9:2]
/// byte1: x0[1:0] y0[9:4]
/// byte2: y0[3:0] x1[9:6]
/// byte3: x1[5:0] y1[9:8]
/// byte4: y1[7:0]
/// byte5: sx[3:0] sy[3:0] (sub-cell offsets, typically ignored)
/// ```
///
/// Where:
/// - (x0, y0) is the source position
/// - (x1, y1) is the destination position
/// - (sx, sy) are sub-cell offsets (can be ignored)
#[derive(Debug, Clone)]
pub struct ZcNotifyPlayermovePacket {
    pub src_x: u16,
    pub src_y: u16,
    pub dest_x: u16,
    pub dest_y: u16,
    pub server_tick: u32,
}

impl ZcNotifyPlayermovePacket {
    /// Decode movement data from 6 bytes
    ///
    /// Extracts source and destination coordinates from the compressed format
    fn decode_movement_data(data: [u8; 6]) -> (u16, u16, u16, u16) {
        // Extract source position (x0, y0)
        let src_x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
        let src_y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);

        // Extract destination position (x1, y1)
        let dest_x = (((data[2] as u16) & 0x0F) << 6) | ((data[3] as u16) >> 2);
        let dest_y = (((data[3] as u16) & 0x03) << 8) | (data[4] as u16);

        // byte5 contains sub-cell offsets sx[3:0] sy[3:0], typically ignored

        (src_x, src_y, dest_x, dest_y)
    }
}

impl ServerPacket for ZcNotifyPlayermovePacket {
    const PACKET_ID: u16 = ZC_NOTIFY_PLAYERMOVE;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 12 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_PLAYERMOVE packet too short: expected 12 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read server tick FIRST (matches server packet structure)
        let server_tick = buf.get_u32_le();

        // Read movement data SECOND (6 bytes)
        let move_data = [
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
        ];

        // Decode movement data
        let (src_x, src_y, dest_x, dest_y) = Self::decode_movement_data(move_data);

        Ok(Self {
            src_x,
            src_y,
            dest_x,
            dest_y,
            server_tick,
        })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_data_decoding() {
        // Test case: src(100, 200) -> dest(150, 250)
        // Manual encoding for verification
        let src_x: u16 = 100;
        let src_y: u16 = 200;
        let dest_x: u16 = 150;
        let dest_y: u16 = 250;

        // Encode
        let byte0 = (src_x >> 2) as u8;
        let byte1 = (((src_x << 6) | ((src_y >> 4) & 0x3F)) & 0xFF) as u8;
        let byte2 = (((src_y << 4) | ((dest_x >> 6) & 0x0F)) & 0xFF) as u8;
        let byte3 = (((dest_x << 2) | ((dest_y >> 8) & 0x03)) & 0xFF) as u8;
        let byte4 = (dest_y & 0xFF) as u8;
        let byte5 = 0; // sub-cell offsets

        let move_data = [byte0, byte1, byte2, byte3, byte4, byte5];

        // Decode
        let (decoded_src_x, decoded_src_y, decoded_dest_x, decoded_dest_y) =
            ZcNotifyPlayermovePacket::decode_movement_data(move_data);

        assert_eq!(decoded_src_x, src_x, "Source X mismatch");
        assert_eq!(decoded_src_y, src_y, "Source Y mismatch");
        assert_eq!(decoded_dest_x, dest_x, "Destination X mismatch");
        assert_eq!(decoded_dest_y, dest_y, "Destination Y mismatch");
    }

    #[test]
    fn test_zc_notify_playermove_parse() {
        // Create test data
        let mut data = vec![0u8; 12];

        // Packet ID: 0x0087
        data[0..2].copy_from_slice(&ZC_NOTIFY_PLAYERMOVE.to_le_bytes());

        // Server tick: 12345 (FIRST after packet ID)
        data[2..6].copy_from_slice(&12345u32.to_le_bytes());

        // Movement data: src(100, 200) -> dest(150, 250) (SECOND after server tick)
        let src_x: u16 = 100;
        let src_y: u16 = 200;
        let dest_x: u16 = 150;
        let dest_y: u16 = 250;

        let byte0 = (src_x >> 2) as u8;
        let byte1 = (((src_x << 6) | ((src_y >> 4) & 0x3F)) & 0xFF) as u8;
        let byte2 = (((src_y << 4) | ((dest_x >> 6) & 0x0F)) & 0xFF) as u8;
        let byte3 = (((dest_x << 2) | ((dest_y >> 8) & 0x03)) & 0xFF) as u8;
        let byte4 = (dest_y & 0xFF) as u8;
        let byte5 = 0;

        data[6] = byte0;
        data[7] = byte1;
        data[8] = byte2;
        data[9] = byte3;
        data[10] = byte4;
        data[11] = byte5;

        let packet = ZcNotifyPlayermovePacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.src_x, src_x);
        assert_eq!(packet.src_y, src_y);
        assert_eq!(packet.dest_x, dest_x);
        assert_eq!(packet.dest_y, dest_y);
        assert_eq!(packet.server_tick, 12345);
    }

    #[test]
    fn test_zc_notify_playermove_parse_invalid_size() {
        let data = vec![0u8; 8]; // Too short
        let result = ZcNotifyPlayermovePacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_boundary_values() {
        // Test with maximum 10-bit values
        let move_data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        let (src_x, src_y, dest_x, dest_y) =
            ZcNotifyPlayermovePacket::decode_movement_data(move_data);

        // All coordinates should be at or near their maximum (1023)
        assert!(src_x <= 1023, "Source X out of range");
        assert!(src_y <= 1023, "Source Y out of range");
        assert!(dest_x <= 1023, "Destination X out of range");
        assert!(dest_y <= 1023, "Destination Y out of range");
    }
}
