use crate::infrastructure::networking::protocol::{
    character::types::CharacterInfo,
    traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Read};

pub const HC_ACCEPT_ENTER: u16 = 0x006B;

/// HC_ACCEPT_ENTER (0x006B) - Accept character server entry
///
/// Server accepts the client's connection and sends the character list.
///
/// # Packet Structure (variable length)
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes)
/// - Max Slots: u8 (1 byte)
/// - Available Slots: u8 (1 byte)
/// - Premium Slots: u8 (1 byte)
/// - Unknown: [u8; 20] (20 bytes)
/// - Characters: [CharacterInfo; N] (175 bytes each)
///
/// Total: 27 + (175 * N) bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcAcceptEnterPacket {
    pub max_slots: u8,
    pub available_slots: u8,
    pub premium_slots: u8,
    pub characters: Vec<CharacterInfo>,
}

impl ServerPacket for HcAcceptEnterPacket {
    const PACKET_ID: u16 = HC_ACCEPT_ENTER;

    fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        // Skip packet ID (already parsed)
        cursor.set_position(2);

        let packet_len = cursor.read_u16::<LittleEndian>()?;

        let max_slots = cursor.read_u8()?;
        let available_slots = cursor.read_u8()?;
        let premium_slots = cursor.read_u8()?;

        // Skip 20 unknown bytes
        let mut unknown = [0u8; 20];
        cursor.read_exact(&mut unknown)?;

        // Calculate character data size
        // Header: 4 (id+len) + 3 (slots) + 20 (unknown) = 27 bytes
        let header_size = 27;
        let char_data_size = packet_len as usize - header_size;

        let mut characters = Vec::new();

        if char_data_size > 0 {
            let char_count = char_data_size / CharacterInfo::SIZE_ACCEPT_ENTER;

            if char_data_size % CharacterInfo::SIZE_ACCEPT_ENTER != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Character data size mismatch: received {} bytes, expected a multiple of {} bytes. \
                         Actual characters: {}, remainder: {} bytes. This indicates a protocol version mismatch.",
                        char_data_size,
                        CharacterInfo::SIZE_ACCEPT_ENTER,
                        char_data_size / CharacterInfo::SIZE_ACCEPT_ENTER,
                        char_data_size % CharacterInfo::SIZE_ACCEPT_ENTER
                    ),
                ));
            }

            for _ in 0..char_count {
                let position = cursor.position() as usize;
                let remaining = data.len() - position;

                if remaining < CharacterInfo::SIZE_ACCEPT_ENTER {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Not enough data for character info",
                    ));
                }

                let char_data = &data[position..position + CharacterInfo::SIZE_ACCEPT_ENTER];
                match CharacterInfo::parse(char_data) {
                    Ok(char_info) => {
                        characters.push(char_info);
                        cursor.set_position((position + CharacterInfo::SIZE_ACCEPT_ENTER) as u64);
                    }
                    Err(e) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Failed to parse character: {}", e),
                        ));
                    }
                }
            }
        }

        Ok(Self {
            max_slots,
            available_slots,
            premium_slots,
            characters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_accept_enter_parse_no_characters() {
        // Packet with no characters
        let mut data = vec![0u8; 27];
        data[0..2].copy_from_slice(&HC_ACCEPT_ENTER.to_le_bytes());
        data[2..4].copy_from_slice(&27u16.to_le_bytes());
        data[4] = 9; // max_slots
        data[5] = 9; // available_slots
        data[6] = 0; // premium_slots

        let packet = HcAcceptEnterPacket::parse(&data).unwrap();
        assert_eq!(packet.max_slots, 9);
        assert_eq!(packet.available_slots, 9);
        assert_eq!(packet.premium_slots, 0);
        assert_eq!(packet.characters.len(), 0);
    }
}
