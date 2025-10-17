use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_SELECT_CHAR: u16 = 0x0066;
const PACKET_SIZE: usize = 3;

/// CH_SELECT_CHAR (0x0066) - Select character
///
/// Selects a character to enter the game world with.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character Slot Number: u8 (1 byte)
///
/// Total: 3 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChSelectCharPacket {
    pub char_num: u8,
}

impl ChSelectCharPacket {
    /// Create a new CH_SELECT_CHAR packet
    ///
    /// # Arguments
    ///
    /// * `char_num` - Character slot number (0-based)
    pub fn new(char_num: u8) -> Self {
        Self { char_num }
    }
}

impl ClientPacket for ChSelectCharPacket {
    const PACKET_ID: u16 = CH_SELECT_CHAR;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u8(self.char_num);

        debug_assert_eq!(
            buf.len(),
            PACKET_SIZE,
            "CH_SELECT_CHAR packet size mismatch"
        );

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_select_char_serialization() {
        let packet = ChSelectCharPacket::new(2);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_SELECT_CHAR);
        assert_eq!(bytes[2], 2);
    }

    #[test]
    fn test_ch_select_char_packet_id() {
        let packet = ChSelectCharPacket::new(0);
        assert_eq!(packet.packet_id(), CH_SELECT_CHAR);
    }
}
