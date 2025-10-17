use crate::infrastructure::networking::protocol::{
    traits::ServerPacket,
    zone::types::{Position, SpawnData},
};
use bytes::Buf;
use std::io;

/// Packet ID for ZC_ACCEPT_ENTER
pub const ZC_ACCEPT_ENTER: u16 = 0x02EB;

/// ZC_ACCEPT_ENTER (0x02EB) - Zone Server → Client
///
/// Sent when the zone server accepts the player into the map.
/// Contains the player's initial spawn position, server tick for synchronization,
/// and character size information.
///
/// # Packet Structure
/// ```text
/// Size: 13 bytes
/// +--------+-------------+----------+----------+--------------------------------+
/// | Offset | Field       | Type     | Size     | Description                    |
/// +--------+-------------+----------+----------+--------------------------------+
/// | 0      | packet_id   | u16      | 2        | 0x02EB                         |
/// | 2      | start_time  | u32      | 4        | Server tick                    |
/// | 6      | pos_dir     | u8[3]    | 3        | Position (x, y, dir) encoded   |
/// | 9      | x_size      | u8       | 1        | Character X size               |
/// | 10     | y_size      | u8       | 1        | Character Y size               |
/// | 11     | font        | u16      | 2        | Font ID                        |
/// +--------+-------------+----------+----------+--------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcAcceptEnterPacket {
    pub spawn_data: SpawnData,
}

impl ZcAcceptEnterPacket {
    /// Get server tick
    pub fn server_tick(&self) -> u32 {
        self.spawn_data.server_tick
    }

    /// Get spawn position
    pub fn position(&self) -> Position {
        self.spawn_data.position
    }

    /// Get character size
    pub fn size(&self) -> (u8, u8) {
        (self.spawn_data.x_size, self.spawn_data.y_size)
    }
}

impl ServerPacket for ZcAcceptEnterPacket {
    const PACKET_ID: u16 = ZC_ACCEPT_ENTER;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 13 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_ACCEPT_ENTER packet too short: expected 13 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        // Skip packet ID (already parsed)
        buf.advance(2);

        // Read server tick
        let start_time = buf.get_u32_le();

        // Read position and direction (3 bytes)
        let pos_dir = [buf.get_u8(), buf.get_u8(), buf.get_u8()];
        let position = Position::decode(pos_dir);

        // Read character size
        let x_size = buf.get_u8();
        let y_size = buf.get_u8();

        // Read font
        let font = buf.get_u16_le();

        Ok(Self {
            spawn_data: SpawnData::new(start_time, position, x_size, y_size, font),
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
    fn test_zc_accept_enter_parse() {
        // Create test data: packet_id (2) + start_time (4) + pos_dir (3) + sizes (2) + font (2)
        let mut data = vec![0u8; 13];

        // Packet ID: 0x02EB
        data[0..2].copy_from_slice(&ZC_ACCEPT_ENTER.to_le_bytes());

        // Start time: 12345
        data[2..6].copy_from_slice(&12345u32.to_le_bytes());

        // Position: (100, 200, 3)
        let position = Position::new(100, 200, 3);
        let encoded_pos = position.encode();
        data[6..9].copy_from_slice(&encoded_pos);

        // Sizes: 5, 5
        data[9] = 5;
        data[10] = 5;

        // Font: 0
        data[11..13].copy_from_slice(&0u16.to_le_bytes());

        let packet = ZcAcceptEnterPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.server_tick(), 12345);
        assert_eq!(packet.position().x, 100);
        assert_eq!(packet.position().y, 200);
        assert_eq!(packet.position().dir, 3);
        assert_eq!(packet.size(), (5, 5));
    }

    #[test]
    fn test_zc_accept_enter_parse_invalid_size() {
        let data = vec![0u8; 5]; // Too short
        let result = ZcAcceptEnterPacket::parse(&data);
        assert!(result.is_err());
    }
}
