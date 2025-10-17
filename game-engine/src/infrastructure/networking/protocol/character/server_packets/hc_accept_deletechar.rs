use crate::infrastructure::networking::protocol::traits::ServerPacket;
use std::io;

pub const HC_ACCEPT_DELETECHAR: u16 = 0x006F;
const PACKET_SIZE: usize = 2;

/// HC_ACCEPT_DELETECHAR (0x006F) - Character deletion success
///
/// Server confirms successful character deletion.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
///
/// Total: 2 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone, Copy)]
pub struct HcAcceptDeletecharPacket;

impl ServerPacket for HcAcceptDeletecharPacket {
    const PACKET_ID: u16 = HC_ACCEPT_DELETECHAR;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_ACCEPT_DELETECHAR packet too short",
            ));
        }

        Ok(Self)
    }
}
