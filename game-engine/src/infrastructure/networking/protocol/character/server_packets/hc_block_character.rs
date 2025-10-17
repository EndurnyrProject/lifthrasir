use crate::infrastructure::networking::protocol::{
    character::types::BlockedCharacterEntry, traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const HC_BLOCK_CHARACTER: u16 = 0x020D;

/// HC_BLOCK_CHARACTER (0x020D) - Blocked characters list
///
/// Server sends list of blocked characters with expiration dates.
///
/// # Packet Structure (variable length)
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes)
/// - Blocked Characters: [BlockedCharacterEntry; N] (24 bytes each)
///
/// Total: 4 + (24 * N) bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcBlockCharacterPacket {
    pub blocked_chars: Vec<BlockedCharacterEntry>,
}

impl ServerPacket for HcBlockCharacterPacket {
    const PACKET_ID: u16 = HC_BLOCK_CHARACTER;

    fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        // Skip packet ID
        cursor.set_position(2);

        let packet_len = cursor.read_u16::<LittleEndian>()?;

        let data_len = packet_len as usize - 4;
        let entry_count = data_len / BlockedCharacterEntry::SIZE;

        if data_len % BlockedCharacterEntry::SIZE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid HC_BLOCK_CHARACTER data length: {} (not multiple of {})",
                    data_len,
                    BlockedCharacterEntry::SIZE
                ),
            ));
        }

        let mut blocked_chars = Vec::with_capacity(entry_count);

        for _ in 0..entry_count {
            let entry = BlockedCharacterEntry::parse(&mut cursor)?;
            blocked_chars.push(entry);
        }

        Ok(Self { blocked_chars })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_block_character_parse_empty() {
        let mut data = vec![0u8; 4];
        data[0..2].copy_from_slice(&HC_BLOCK_CHARACTER.to_le_bytes());
        data[2..4].copy_from_slice(&4u16.to_le_bytes());

        let packet = HcBlockCharacterPacket::parse(&data).unwrap();
        assert_eq!(packet.blocked_chars.len(), 0);
    }
}
