use crate::infrastructure::networking::protocol::{
    character::types::CharacterInfo,
    traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const HC_ACK_CHARINFO_PER_PAGE: u16 = 0x099D;

/// HC_ACK_CHARINFO_PER_PAGE (0x099D) - Character info per page
///
/// Server sends character information page by page. Uses a different
/// character structure size (175 bytes) compared to HC_ACCEPT_ENTER.
///
/// # Packet Structure (variable length)
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes)
/// - Characters: [CharacterInfo; N] (175 bytes each)
///
/// Total: 4 + (175 * N) bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcAckCharinfoPerPagePacket {
    pub characters: Vec<CharacterInfo>,
}

impl ServerPacket for HcAckCharinfoPerPagePacket {
    const PACKET_ID: u16 = HC_ACK_CHARINFO_PER_PAGE;

    fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        // Skip packet ID
        cursor.set_position(2);

        let packet_len = cursor.read_u16::<LittleEndian>()?;

        if packet_len < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_ACK_CHARINFO_PER_PAGE packet too short",
            ));
        }

        let data_len = (packet_len - 4) as usize;

        if data_len % CharacterInfo::SIZE_CHARINFO_PER_PAGE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid character data length: {} (not multiple of {})",
                    data_len,
                    CharacterInfo::SIZE_CHARINFO_PER_PAGE
                ),
            ));
        }

        let char_count = data_len / CharacterInfo::SIZE_CHARINFO_PER_PAGE;
        let mut characters = Vec::with_capacity(char_count);

        for _ in 0..char_count {
            let position = cursor.position() as usize;
            let remaining = data.len() - position;

            if remaining < CharacterInfo::SIZE_CHARINFO_PER_PAGE {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Not enough data for character info",
                ));
            }

            // Note: This packet uses a 175-byte structure, but we're using the 155-byte
            // parser. The extra 20 bytes are typically unknown/padding fields at the end.
            // For now, we parse what we can and skip the rest.
            let char_data = &data[position..position + CharacterInfo::SIZE_ACCEPT_ENTER];
            match CharacterInfo::parse(char_data) {
                Ok(char_info) => {
                    characters.push(char_info);
                    // Skip the full 175 bytes
                    cursor.set_position((position + CharacterInfo::SIZE_CHARINFO_PER_PAGE) as u64);
                }
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to parse character: {}", e),
                    ));
                }
            }
        }

        Ok(Self { characters })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_ack_charinfo_per_page_parse_empty() {
        let mut data = vec![0u8; 4];
        data[0..2].copy_from_slice(&HC_ACK_CHARINFO_PER_PAGE.to_le_bytes());
        data[2..4].copy_from_slice(&4u16.to_le_bytes());

        let packet = HcAckCharinfoPerPagePacket::parse(&data).unwrap();
        assert_eq!(packet.characters.len(), 0);
    }
}
