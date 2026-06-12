use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bevy::prelude::*;
use bytes::Buf;
use std::io::{self, Cursor};

/// Packet ID for ZC_HP_INFO (monster HP information)
pub const ZC_HP_INFO: u16 = 0x0977;

/// ZC_HP_INFO - Monster HP Information
///
/// Sent by the server to update a monster's HP bar display to nearby players.
/// Used when a monster takes damage or is healed.
///
/// **Packet Structure (14 bytes)**:
/// - `packet_id` (u16): 0x0977
/// - `id` (u32): Monster's GID/instance ID
/// - `hp` (u32): Current HP value
/// - `max_hp` (u32): Maximum HP value
#[derive(Debug, Clone)]
pub struct ZcHpInfoPacket {
    pub id: u32,
    pub hp: u32,
    pub max_hp: u32,
}

impl ZcHpInfoPacket {
    pub fn hp_percentage(&self) -> f32 {
        if self.max_hp == 0 {
            return 0.0;
        }
        (self.hp as f32 / self.max_hp as f32) * 100.0
    }
}

impl ServerPacket for ZcHpInfoPacket {
    const PACKET_ID: u16 = ZC_HP_INFO;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 14 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_HP_INFO packet too short: expected 14 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut cursor = Cursor::new(data);

        let packet_id = cursor.get_u16_le();
        if packet_id != ZC_HP_INFO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid packet ID for ZC_HP_INFO: expected 0x{:04X}, got 0x{:04X}",
                    ZC_HP_INFO, packet_id
                ),
            ));
        }

        let id = cursor.get_u32_le();
        let hp = cursor.get_u32_le();
        let max_hp = cursor.get_u32_le();

        info!(
            "[PARSE] ZC_HP_INFO: monster_id={}, hp={}/{} ({:.1}%)",
            id,
            hp,
            max_hp,
            if max_hp > 0 {
                (hp as f32 / max_hp as f32) * 100.0
            } else {
                0.0
            }
        );

        Ok(Self { id, hp, max_hp })
    }

    fn packet_id(&self) -> u16 {
        ZC_HP_INFO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hp_info() {
        let data = [
            0x77, 0x09, // Packet ID: 0x0977
            0xD1, 0x07, 0x00, 0x00, // Monster ID: 2001
            0xF4, 0x01, 0x00, 0x00, // Current HP: 500
            0xE8, 0x03, 0x00, 0x00, // Max HP: 1000
        ];

        let packet = ZcHpInfoPacket::parse(&data).unwrap();
        assert_eq!(packet.id, 2001);
        assert_eq!(packet.hp, 500);
        assert_eq!(packet.max_hp, 1000);
        assert!((packet.hp_percentage() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_full_hp() {
        let data = [
            0x77, 0x09, // Packet ID: 0x0977
            0x01, 0x00, 0x00, 0x00, // Monster ID: 1
            0xE8, 0x03, 0x00, 0x00, // Current HP: 1000
            0xE8, 0x03, 0x00, 0x00, // Max HP: 1000
        ];

        let packet = ZcHpInfoPacket::parse(&data).unwrap();
        assert_eq!(packet.hp, packet.max_hp);
        assert!((packet.hp_percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_zero_hp() {
        let data = [
            0x77, 0x09, // Packet ID: 0x0977
            0x01, 0x00, 0x00, 0x00, // Monster ID: 1
            0x00, 0x00, 0x00, 0x00, // Current HP: 0
            0xE8, 0x03, 0x00, 0x00, // Max HP: 1000
        ];

        let packet = ZcHpInfoPacket::parse(&data).unwrap();
        assert_eq!(packet.hp, 0);
        assert!((packet.hp_percentage() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_invalid_length() {
        let data = [0x77, 0x09, 0x01, 0x02];
        let result = ZcHpInfoPacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_packet_id() {
        let data = [
            0x78, 0x09, // Wrong packet ID
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xE8, 0x03, 0x00, 0x00,
        ];

        let result = ZcHpInfoPacket::parse(&data);
        assert!(result.is_err());
    }
}
