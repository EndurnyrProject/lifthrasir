use crate::infrastructure::networking::protocol::traits::ServerPacket;
use std::io;

pub const HC_PING: u16 = 0x0187;
const PACKET_SIZE: usize = 2;

/// HC_PING (0x0187) - Ping response
///
/// Server responds to client ping to keep connection alive.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
///
/// Total: 2 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone, Copy)]
pub struct HcPingPacket;

impl ServerPacket for HcPingPacket {
    const PACKET_ID: u16 = HC_PING;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_PING packet too short",
            ));
        }

        Ok(Self)
    }
}
