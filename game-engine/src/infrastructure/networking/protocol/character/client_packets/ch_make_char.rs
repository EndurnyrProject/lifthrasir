use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_MAKE_CHAR: u16 = 0x0A39;
const PACKET_SIZE: usize = 36;
const NAME_MAX_BYTES: usize = 24;

/// CH_MAKE_CHAR (0x0A39) - Create new character
///
/// Requests creation of a new character with specified appearance.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character Name: [u8; 24] (null-padded)
/// - Slot: u8 (1 byte)
/// - Hair Color: u16 (2 bytes)
/// - Hair Style: u16 (2 bytes)
/// - Starting Job: u16 (2 bytes)
/// - Unknown: u16 (2 bytes)
/// - Sex: u8 (1 byte)
///
/// Total: 36 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChMakeCharPacket {
    pub name: String,
    pub slot: u8,
    pub hair_color: u16,
    pub hair_style: u16,
    pub starting_job: u16,
    pub sex: u8,
}

impl ChMakeCharPacket {
    /// Create a new CH_MAKE_CHAR packet
    ///
    /// # Arguments
    ///
    /// * `name` - Character name (max 23 characters)
    /// * `slot` - Character slot number
    /// * `hair_color` - Hair color ID
    /// * `hair_style` - Hair style ID
    /// * `starting_job` - Starting job ID (typically 0 for novice)
    /// * `sex` - Character sex (0 = female, 1 = male)
    pub fn new(
        name: &str,
        slot: u8,
        hair_color: u16,
        hair_style: u16,
        starting_job: u16,
        sex: u8,
    ) -> Self {
        Self {
            name: name.to_string(),
            slot,
            hair_color,
            hair_style,
            starting_job,
            sex,
        }
    }
}

impl ClientPacket for ChMakeCharPacket {
    const PACKET_ID: u16 = CH_MAKE_CHAR;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);

        // Character name (24 bytes, null-padded)
        let mut name_bytes = [0u8; NAME_MAX_BYTES];
        let name_data = self.name.as_bytes();
        let copy_len = name_data.len().min(NAME_MAX_BYTES - 1);
        name_bytes[..copy_len].copy_from_slice(&name_data[..copy_len]);
        buf.put_slice(&name_bytes);

        buf.put_u8(self.slot);
        buf.put_u16_le(self.hair_color);
        buf.put_u16_le(self.hair_style);
        buf.put_u16_le(self.starting_job);
        buf.put_u16_le(0); // Unknown
        buf.put_u8(self.sex);

        debug_assert_eq!(buf.len(), PACKET_SIZE, "CH_MAKE_CHAR packet size mismatch");

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_make_char_serialization() {
        let packet = ChMakeCharPacket::new("TestChar", 0, 5, 3, 0, 1);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_MAKE_CHAR);
    }

    #[test]
    fn test_ch_make_char_name_truncation() {
        let long_name = "a".repeat(50);
        let packet = ChMakeCharPacket::new(&long_name, 0, 0, 0, 0, 0);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
    }

    #[test]
    fn test_ch_make_char_packet_id() {
        let packet = ChMakeCharPacket::new("Test", 0, 0, 0, 0, 0);
        assert_eq!(packet.packet_id(), CH_MAKE_CHAR);
    }
}
