use crate::infrastructure::networking::protocol::traits::ServerPacket;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const HC_CHAR_DELETE2_ACK: u16 = 0x0828;
const PACKET_SIZE: usize = 14;

/// Result code reported by HC_CHAR_DELETE2_ACK.
pub const DELETE2_RESULT_SUCCESS: u32 = 0;

/// HC_CHAR_DELETE2_ACK (0x0828) - Response to a delete2 request
///
/// Confirms or rejects a CH_REQ_CHAR_DELETE2 (0x0827) request. On success the
/// character is scheduled for deletion at `delete_date`.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character ID: u32 (4 bytes)
/// - Result: u32 (4 bytes)
/// - Delete Date: u32 (4 bytes)
///
/// Total: 14 bytes
///
/// Result codes:
/// - 0: Success (character will be deleted after timeout)
/// - 1: Database error
/// - 2: Character doesn't belong to account
/// - 3: Character already marked for deletion
/// - 4: Cannot delete character (guild member, has items, etc.)
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcCharDelete2AckPacket {
    pub char_id: u32,
    pub result: u32,
    pub delete_date: u32,
}

impl HcCharDelete2AckPacket {
    /// Whether the server accepted the deletion request.
    pub fn is_success(&self) -> bool {
        self.result == DELETE2_RESULT_SUCCESS
    }
}

impl ServerPacket for HcCharDelete2AckPacket {
    const PACKET_ID: u16 = HC_CHAR_DELETE2_ACK;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_CHAR_DELETE2_ACK packet too short",
            ));
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(2); // Skip packet ID

        let char_id = cursor.read_u32::<LittleEndian>()?;
        let result = cursor.read_u32::<LittleEndian>()?;
        let delete_date = cursor.read_u32::<LittleEndian>()?;

        Ok(Self {
            char_id,
            result,
            delete_date,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_char_delete2_ack_parse_success() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_CHAR_DELETE2_ACK.to_le_bytes());
        data[2..6].copy_from_slice(&150000u32.to_le_bytes());
        data[6..10].copy_from_slice(&0u32.to_le_bytes());
        data[10..14].copy_from_slice(&1234u32.to_le_bytes());

        let packet = HcCharDelete2AckPacket::parse(&data).unwrap();
        assert_eq!(packet.char_id, 150000);
        assert!(packet.is_success());
        assert_eq!(packet.delete_date, 1234);
    }

    #[test]
    fn test_hc_char_delete2_ack_parse_failure() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_CHAR_DELETE2_ACK.to_le_bytes());
        data[2..6].copy_from_slice(&150000u32.to_le_bytes());
        data[6..10].copy_from_slice(&4u32.to_le_bytes());

        let packet = HcCharDelete2AckPacket::parse(&data).unwrap();
        assert!(!packet.is_success());
        assert_eq!(packet.result, 4);
    }
}
