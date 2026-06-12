use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_REQUEST_ACT2
pub const CZ_REQUEST_ACT2: u16 = 0x0437;

/// CZ_REQUEST_ACT2 (0x0437) - Client → Zone Server
///
/// Requests a combat action (attack, skill, sit/stand, pickup, etc.)
/// The server will validate the action and respond with ZC_NOTIFY_ACT.
///
/// # Packet Structure
/// ```text
/// Size: 7 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x0437                           |
/// | 2      | target_gid  | u32      | 4    | Target entity GID                |
/// | 6      | action      | u8       | 1    | Action type (0=attack, 2=sit...) |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
///
/// Action types:
/// - 0: Attack
/// - 2: Sit down
/// - 3: Stand up
/// - 7: Continuous attack (auto-attack)
#[derive(Debug, Clone)]
pub struct CzRequestAct2Packet {
    pub target_gid: u32,
    pub action: u8,
}

impl CzRequestAct2Packet {
    /// Create a new CZ_REQUEST_ACT2 packet for attacking a target
    pub fn attack(target_gid: u32) -> Self {
        Self {
            target_gid,
            action: 0,
        }
    }

    /// Create a new CZ_REQUEST_ACT2 packet for sitting down
    pub fn sit() -> Self {
        Self {
            target_gid: 0,
            action: 2,
        }
    }

    /// Create a new CZ_REQUEST_ACT2 packet for standing up
    pub fn stand() -> Self {
        Self {
            target_gid: 0,
            action: 3,
        }
    }

    /// Create a new CZ_REQUEST_ACT2 packet for continuous attack
    pub fn continuous_attack(target_gid: u32) -> Self {
        Self {
            target_gid,
            action: 7,
        }
    }
}

impl ClientPacket for CzRequestAct2Packet {
    const PACKET_ID: u16 = CZ_REQUEST_ACT2;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(7);
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.target_gid);
        buf.put_u8(self.action);
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
    fn test_attack_packet_serialization() {
        let packet = CzRequestAct2Packet::attack(12345);

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 7, "Packet size should be 7 bytes");

        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_REQUEST_ACT2);

        let target_gid = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(target_gid, 12345);

        assert_eq!(bytes[6], 0);
    }

    #[test]
    fn test_sit_packet() {
        let packet = CzRequestAct2Packet::sit();
        let bytes = packet.serialize();

        assert_eq!(bytes[6], 2);
        let target_gid = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(target_gid, 0);
    }

    #[test]
    fn test_stand_packet() {
        let packet = CzRequestAct2Packet::stand();
        let bytes = packet.serialize();

        assert_eq!(bytes[6], 3);
    }

    #[test]
    fn test_continuous_attack_packet() {
        let packet = CzRequestAct2Packet::continuous_attack(99999);
        let bytes = packet.serialize();

        assert_eq!(bytes[6], 7);
        let target_gid = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(target_gid, 99999);
    }

    #[test]
    fn test_packet_id() {
        let packet = CzRequestAct2Packet::attack(1);
        assert_eq!(packet.packet_id(), CZ_REQUEST_ACT2);
    }
}
