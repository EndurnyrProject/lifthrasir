use crate::infrastructure::networking::protocol::traits::ServerPacket;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const HC_CHARLIST_NOTIFY: u16 = 0x09A0;
const PACKET_SIZE: usize = 6;

/// HC_CHARLIST_NOTIFY (0x09A0) - Character list page count notification
///
/// Sent as part of the character-list response sequence. Reports how many
/// pages the client should expect for the per-page flow. This server delivers
/// the full character list up front in HC_ACCEPT_ENTER, so the count is purely
/// informational here.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Page Count: u32 (4 bytes)
///
/// Total: 6 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcCharlistNotifyPacket {
    pub page_count: u32,
}

impl ServerPacket for HcCharlistNotifyPacket {
    const PACKET_ID: u16 = HC_CHARLIST_NOTIFY;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_CHARLIST_NOTIFY packet too short",
            ));
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(2); // Skip packet ID

        let page_count = cursor.read_u32::<LittleEndian>()?;

        Ok(Self { page_count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_charlist_notify_parse() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_CHARLIST_NOTIFY.to_le_bytes());
        data[2..6].copy_from_slice(&3u32.to_le_bytes());

        let packet = HcCharlistNotifyPacket::parse(&data).unwrap();
        assert_eq!(packet.page_count, 3);
    }
}
