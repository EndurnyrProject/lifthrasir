use crate::infrastructure::networking::protocol::{
    character::types::CharDeletionError,
    traits::ServerPacket,
};
use std::io::{self, Cursor};
use byteorder::ReadBytesExt;

pub const HC_REFUSE_DELETECHAR: u16 = 0x0070;
const PACKET_SIZE: usize = 3;

/// HC_REFUSE_DELETECHAR (0x0070) - Character deletion failed
///
/// Server refuses character deletion with an error code.
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
pub struct HcRefuseDeletecharPacket {
    pub error: CharDeletionError,
}

impl ServerPacket for HcRefuseDeletecharPacket {
    const PACKET_ID: u16 = HC_REFUSE_DELETECHAR;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_REFUSE_DELETECHAR packet too short",
            ));
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(2); // Skip packet ID

        let error_code = cursor.read_u8()?;

        Ok(Self {
            error: CharDeletionError::from(error_code),
        })
    }
}
