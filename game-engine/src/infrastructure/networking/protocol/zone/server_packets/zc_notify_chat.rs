use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_NOTIFY_CHAT: u16 = 0x008D;

/// ZC_NOTIFY_CHAT (0x008D) - Zone Server → Client
///
/// Sent when a player speaks in the area (Normal Chat).
///
/// # Packet Structure
/// Variable-length packet:
/// ```text
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x008D                           |
/// | 2      | length      | u16      | 2    | Total packet length              |
/// | 4      | gid         | u32      | 4    | Game ID of the speaker           |
/// | 8      | message     | String   | Var  | "Name : Message"                 |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcNotifyChatPacket {
    pub gid: u32,
    pub message: String,
}

impl ServerPacket for ZcNotifyChatPacket {
    const PACKET_ID: u16 = ZC_NOTIFY_CHAT;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_CHAT packet too short: expected at least 8 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;
        buf.advance(2); // Skip packet_id

        let packet_length = buf.get_u16_le();
        if data.len() < packet_length as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_CHAT incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let gid = buf.get_u32_le();
        
        // Message is the rest of the packet
        let message_len = packet_length as usize - 8;
        let mut message_bytes = vec![0u8; message_len];
        buf.copy_to_slice(&mut message_bytes);
        
        // Remove potential null terminator if present (some servers send it)
        let message = if let Some(end) = message_bytes.iter().position(|&b| b == 0) {
             String::from_utf8_lossy(&message_bytes[..end]).to_string()
        } else {
             String::from_utf8_lossy(&message_bytes).to_string()
        };

        Ok(Self { gid, message })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}
