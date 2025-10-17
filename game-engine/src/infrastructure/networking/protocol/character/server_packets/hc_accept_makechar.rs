use crate::infrastructure::networking::protocol::{
    character::types::CharacterInfo, traits::ServerPacket,
};
use std::io;

pub const HC_ACCEPT_MAKECHAR: u16 = 0x0B6F;

/// HC_ACCEPT_MAKECHAR (0x0B6F) - Character creation success
///
/// Server confirms successful character creation and sends the new
/// character's data.
///
/// # Packet Structure (variable length)
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes)
/// - Character Data: CharacterInfo (155 bytes)
///
/// Total: 159 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcAcceptMakecharPacket {
    pub character: CharacterInfo,
}

impl ServerPacket for HcAcceptMakecharPacket {
    const PACKET_ID: u16 = HC_ACCEPT_MAKECHAR;

    fn parse(data: &[u8]) -> io::Result<Self> {
        // Skip packet ID (2 bytes) and length (2 bytes)
        if data.len() < 4 + CharacterInfo::SIZE_ACCEPT_ENTER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_ACCEPT_MAKECHAR packet too short",
            ));
        }

        let char_data = &data[4..];
        let character = CharacterInfo::parse(char_data)?;

        Ok(Self { character })
    }
}
