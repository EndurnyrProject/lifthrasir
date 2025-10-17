use crate::infrastructure::networking::protocol::{
    character::types::CharCreationError, traits::ServerPacket,
};
use byteorder::ReadBytesExt;
use std::io::{self, Cursor};

pub const HC_REFUSE_MAKECHAR: u16 = 0x006E;
const PACKET_SIZE: usize = 3;

/// HC_REFUSE_MAKECHAR (0x006E) - Character creation failed
///
/// Server refuses character creation with an error code.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Error Code: u8 (1 byte)
///
/// Total: 3 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcRefuseMakecharPacket {
    pub error: CharCreationError,
}

impl ServerPacket for HcRefuseMakecharPacket {
    const PACKET_ID: u16 = HC_REFUSE_MAKECHAR;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_REFUSE_MAKECHAR packet too short",
            ));
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(2); // Skip packet ID

        let error_code = cursor.read_u8()?;

        Ok(Self {
            error: CharCreationError::from(error_code),
        })
    }
}
