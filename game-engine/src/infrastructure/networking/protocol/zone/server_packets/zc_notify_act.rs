use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bevy::prelude::*;
use bytes::Buf;
use std::io::{self, Cursor};

/// Packet ID for ZC_NOTIFY_ACT (combat action notification)
/// PACKETVER >= 20131223
pub const ZC_NOTIFY_ACT: u16 = 0x08C8;

/// ZC_NOTIFY_ACT - Combat Action Notification
///
/// Sent by the server when a combat action occurs (attack, hit, damage, etc.)
/// This packet is the core of the combat system, informing clients about
/// all combat-related animations and damage numbers.
///
/// **Packet Structure (34 bytes)**:
/// - `packet_id` (u16): 0x08C8
/// - `src_id` (u32): Source entity ID (attacker/actor)
/// - `target_id` (u32): Target entity ID (defender/victim)
/// - `server_tick` (u32): Server timestamp
/// - `src_speed` (i32): Source entity attack speed
/// - `dmg_speed` (i32): Damage motion speed
/// - `damage` (i32): Primary damage value (signed for proper display)
/// - `is_sp_damage` (u8): Whether this is SP damage (0 = HP, 1 = SP)
/// - `div` (u16): Number of hits (>1 for critical or multi-hit)
/// - `action_type` (u8): Type of action
/// - `damage2` (i32): Secondary damage (for dual-wield)
///
/// Action types:
/// - 0: Attack / Normal attack
/// - 1: Pickup item
/// - 2: Sit down
/// - 3: Stand up
/// - 4: Hit / Multi-hit attack
/// - 5: Splash (unused)
/// - 6: Skill
/// - 7: Repeat (unused)
/// - 8: Damage / Critical hit
/// - 9: SP damage
/// - 10: Lucky dodge (miss)
#[derive(Debug, Clone)]
pub struct ZcNotifyActPacket {
    pub src_id: u32,
    pub target_id: u32,
    pub server_tick: u32,
    pub src_speed: i32,
    pub dmg_speed: i32,
    pub damage: i32,
    pub is_sp_damage: u8,
    pub div: u16,
    pub action_type: u8,
    pub damage2: i32,
}

impl ServerPacket for ZcNotifyActPacket {
    const PACKET_ID: u16 = ZC_NOTIFY_ACT;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 34 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_ACT packet too short: expected 34 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut cursor = Cursor::new(data);

        let packet_id = cursor.get_u16_le();
        if packet_id != ZC_NOTIFY_ACT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid packet ID for ZC_NOTIFY_ACT: expected 0x{:04X}, got 0x{:04X}",
                    ZC_NOTIFY_ACT, packet_id
                ),
            ));
        }

        let src_id = cursor.get_u32_le();
        let target_id = cursor.get_u32_le();
        let server_tick = cursor.get_u32_le();
        let src_speed = cursor.get_i32_le();
        let dmg_speed = cursor.get_i32_le();
        let damage = cursor.get_i32_le();
        let is_sp_damage = cursor.get_u8();
        let div = cursor.get_u16_le();
        let action_type = cursor.get_u8();
        let damage2 = cursor.get_i32_le();

        let action_name = match action_type {
            0 => "attack",
            1 => "pickup",
            2 => "sit",
            3 => "stand",
            4 => "hit",
            5 => "splash",
            6 => "skill",
            7 => "repeat",
            8 => "damage",
            9 => "sp_damage",
            10 => "lucky_dodge",
            _ => "unknown",
        };

        info!(
            "[PARSE] ZC_NOTIFY_ACT: src={} -> target={}, action={} ({}), damage={}, damage2={}, div={}, tick={}",
            src_id, target_id, action_name, action_type, damage, damage2, div, server_tick
        );

        Ok(Self {
            src_id,
            target_id,
            server_tick,
            src_speed,
            dmg_speed,
            damage,
            is_sp_damage,
            div,
            action_type,
            damage2,
        })
    }

    fn packet_id(&self) -> u16 {
        ZC_NOTIFY_ACT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_attack_action() {
        let data = [
            0xC8, 0x08, // Packet ID: 0x08C8
            0x01, 0x00, 0x00, 0x00, // Source ID: 1
            0x02, 0x00, 0x00, 0x00, // Target ID: 2
            0x10, 0x27, 0x00, 0x00, // Server tick: 10000
            0xE8, 0x03, 0x00, 0x00, // Source speed: 1000
            0xF4, 0x01, 0x00, 0x00, // Dmg speed: 500
            0x64, 0x00, 0x00, 0x00, // Damage: 100
            0x00, // Is SP damage: 0
            0x01, 0x00, // Div: 1
            0x00, // Action type: 0 (attack)
            0x00, 0x00, 0x00, 0x00, // Damage2: 0
        ];

        let packet = ZcNotifyActPacket::parse(&data).unwrap();
        assert_eq!(packet.src_id, 1);
        assert_eq!(packet.target_id, 2);
        assert_eq!(packet.server_tick, 10000);
        assert_eq!(packet.src_speed, 1000);
        assert_eq!(packet.dmg_speed, 500);
        assert_eq!(packet.damage, 100);
        assert_eq!(packet.is_sp_damage, 0);
        assert_eq!(packet.div, 1);
        assert_eq!(packet.action_type, 0);
        assert_eq!(packet.damage2, 0);
    }

    #[test]
    fn test_parse_critical_hit() {
        let data = [
            0xC8, 0x08, // Packet ID: 0x08C8
            0x01, 0x00, 0x00, 0x00, // Source ID: 1
            0x02, 0x00, 0x00, 0x00, // Target ID: 2
            0x10, 0x27, 0x00, 0x00, // Server tick: 10000
            0xE8, 0x03, 0x00, 0x00, // Source speed: 1000
            0xF4, 0x01, 0x00, 0x00, // Dmg speed: 500
            0xC8, 0x00, 0x00, 0x00, // Damage: 200
            0x00, // Is SP damage: 0
            0x02, 0x00, // Div: 2 (critical)
            0x08, // Action type: 8 (critical/damage)
            0x00, 0x00, 0x00, 0x00, // Damage2: 0
        ];

        let packet = ZcNotifyActPacket::parse(&data).unwrap();
        assert_eq!(packet.damage, 200);
        assert_eq!(packet.div, 2);
        assert_eq!(packet.action_type, 8);
    }

    #[test]
    fn test_parse_miss() {
        let data = [
            0xC8, 0x08, // Packet ID: 0x08C8
            0x01, 0x00, 0x00, 0x00, // Source ID: 1
            0x02, 0x00, 0x00, 0x00, // Target ID: 2
            0x10, 0x27, 0x00, 0x00, // Server tick: 10000
            0xE8, 0x03, 0x00, 0x00, // Source speed: 1000
            0xF4, 0x01, 0x00, 0x00, // Dmg speed: 500
            0x00, 0x00, 0x00, 0x00, // Damage: 0 (miss)
            0x00, // Is SP damage: 0
            0x01, 0x00, // Div: 1
            0x0A, // Action type: 10 (lucky dodge)
            0x00, 0x00, 0x00, 0x00, // Damage2: 0
        ];

        let packet = ZcNotifyActPacket::parse(&data).unwrap();
        assert_eq!(packet.damage, 0);
        assert_eq!(packet.action_type, 10);
    }

    #[test]
    fn test_parse_dual_wield() {
        let data = [
            0xC8, 0x08, // Packet ID: 0x08C8
            0x01, 0x00, 0x00, 0x00, // Source ID: 1
            0x02, 0x00, 0x00, 0x00, // Target ID: 2
            0x10, 0x27, 0x00, 0x00, // Server tick: 10000
            0xE8, 0x03, 0x00, 0x00, // Source speed: 1000
            0xF4, 0x01, 0x00, 0x00, // Dmg speed: 500
            0x64, 0x00, 0x00, 0x00, // Damage: 100
            0x00, // Is SP damage: 0
            0x01, 0x00, // Div: 1
            0x00, // Action type: 0 (attack)
            0x32, 0x00, 0x00, 0x00, // Damage2: 50 (dual-wield secondary)
        ];

        let packet = ZcNotifyActPacket::parse(&data).unwrap();
        assert_eq!(packet.damage, 100);
        assert_eq!(packet.damage2, 50);
    }

    #[test]
    fn test_parse_sp_damage() {
        let data = [
            0xC8, 0x08, // Packet ID: 0x08C8
            0x01, 0x00, 0x00, 0x00, // Source ID: 1
            0x02, 0x00, 0x00, 0x00, // Target ID: 2
            0x10, 0x27, 0x00, 0x00, // Server tick: 10000
            0xE8, 0x03, 0x00, 0x00, // Source speed: 1000
            0xF4, 0x01, 0x00, 0x00, // Dmg speed: 500
            0x0A, 0x00, 0x00, 0x00, // Damage: 10 SP
            0x01, // Is SP damage: 1 (SP)
            0x01, 0x00, // Div: 1
            0x00, // Action type: 0
            0x00, 0x00, 0x00, 0x00, // Damage2: 0
        ];

        let packet = ZcNotifyActPacket::parse(&data).unwrap();
        assert_eq!(packet.damage, 10);
        assert_eq!(packet.is_sp_damage, 1);
    }

    #[test]
    fn test_parse_invalid_length() {
        let data = [0xC8, 0x08, 0x01, 0x02];

        let result = ZcNotifyActPacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_packet_id() {
        let data = [
            0xC9, 0x08, // Wrong packet ID
            0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x10, 0x27, 0x00, 0x00, 0xE8, 0x03,
            0x00, 0x00, 0xF4, 0x01, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = ZcNotifyActPacket::parse(&data);
        assert!(result.is_err());
    }
}
