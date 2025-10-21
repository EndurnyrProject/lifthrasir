use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bevy::prelude::*;
use bytes::Buf;
use std::io::{self, Cursor};

/// Packet ID for ZC_NOTIFY_VANISH (entity disappeared/died)
pub const ZC_NOTIFY_VANISH: u16 = 0x0080;

/// ZC_NOTIFY_VANISH - Entity Vanish Notification
///
/// Sent by the server when an entity (player, NPC, mob, etc.) disappears from view
/// either by moving out of range, dying, or being despawned.
///
/// **Packet Structure (7 bytes)**:
/// - `packet_id` (u16): 0x0080
/// - `gid` (u32): Game/Entity ID of the vanishing entity
/// - `vanish_type` (u8): Type of vanish (0 = out of sight, 1 = died, 2 = logged out, 3 = teleported)
#[derive(Debug, Clone)]
pub struct ZcNotifyVanishPacket {
    /// Game/Entity ID (GID) of the entity that vanished
    pub gid: u32,

    /// Type of vanish
    /// - 0: Out of sight (moved out of view range)
    /// - 1: Died
    /// - 2: Logged out
    /// - 3: Teleported
    pub vanish_type: u8,
}

impl ServerPacket for ZcNotifyVanishPacket {
    const PACKET_ID: u16 = ZC_NOTIFY_VANISH;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_VANISH packet too short: expected 7 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut cursor = Cursor::new(data);

        let packet_id = cursor.get_u16_le();
        if packet_id != ZC_NOTIFY_VANISH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid packet ID for ZC_NOTIFY_VANISH: expected 0x{:04X}, got 0x{:04X}",
                    ZC_NOTIFY_VANISH, packet_id
                ),
            ));
        }

        let gid = cursor.get_u32_le();
        let vanish_type = cursor.get_u8();

        let vanish_reason = match vanish_type {
            0 => "out of sight",
            1 => "died",
            2 => "logged out",
            3 => "teleported",
            _ => "unknown",
        };

        info!(
            "[PARSE] ZC_NOTIFY_VANISH: GID {} ({}, type: {})",
            gid, vanish_reason, vanish_type
        );

        Ok(Self { gid, vanish_type })
    }

    fn packet_id(&self) -> u16 {
        ZC_NOTIFY_VANISH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vanish_packet() {
        let data = [
            0x80, 0x00, // Packet ID: 0x0080
            0x01, 0x02, 0x03, 0x04, // GID: 0x04030201
            0x00, // Vanish type: 0 (out of sight)
        ];

        let packet = ZcNotifyVanishPacket::parse(&data).unwrap();
        assert_eq!(packet.gid, 0x04030201);
        assert_eq!(packet.vanish_type, 0);
    }

    #[test]
    fn test_parse_vanish_died() {
        let data = [
            0x80, 0x00, // Packet ID
            0x10, 0x20, 0x30, 0x40, // GID
            0x01, // Vanish type: 1 (died)
        ];

        let packet = ZcNotifyVanishPacket::parse(&data).unwrap();
        assert_eq!(packet.gid, 0x40302010);
        assert_eq!(packet.vanish_type, 1);
    }

    #[test]
    fn test_parse_invalid_length() {
        let data = [0x80, 0x00, 0x01, 0x02]; // Too short

        let result = ZcNotifyVanishPacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_packet_id() {
        let data = [
            0x81, 0x00, // Wrong packet ID
            0x01, 0x02, 0x03, 0x04, 0x00,
        ];

        let result = ZcNotifyVanishPacket::parse(&data);
        assert!(result.is_err());
    }
}
