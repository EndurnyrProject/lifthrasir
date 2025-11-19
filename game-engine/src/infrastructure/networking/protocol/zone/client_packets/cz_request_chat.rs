use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

/// Packet ID for CZ_REQUEST_CHAT
pub const CZ_REQUEST_CHAT: u16 = 0x008C;

/// CZ_REQUEST_CHAT (0x008C) - Client → Zone Server
///
/// Requests to send a chat message to the area (Normal Chat).
///
/// # Packet Structure
/// Variable-length packet:
/// ```text
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x008C                           |
/// | 2      | length      | u16      | 2    | Total packet length              |
/// | 4      | message     | String   | Var  | "Name : Message"                 |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct CzRequestChatPacket {
    pub message: String,
}

impl CzRequestChatPacket {
    /// Create a new CZ_REQUEST_CHAT packet
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl ClientPacket for CzRequestChatPacket {
    const PACKET_ID: u16 = CZ_REQUEST_CHAT;

    fn serialize(&self) -> Bytes {
        let len = 4 + self.message.len();
        let mut buf = BytesMut::with_capacity(len);
        
        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u16_le(len as u16);
        buf.put_slice(self.message.as_bytes());

        buf.freeze()
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cz_request_chat_serialization() {
        let message = "Player : Hello World".to_string();
        let packet = CzRequestChatPacket::new(message.clone());

        let bytes = packet.serialize();
        let len = 4 + message.len();
        
        assert_eq!(bytes.len(), len);

        // Verify packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CZ_REQUEST_CHAT);

        // Verify length
        let packet_len = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(packet_len, len as u16);

        // Verify message
        let msg_bytes = &bytes[4..];
        let msg = String::from_utf8(msg_bytes.to_vec()).unwrap();
        assert_eq!(msg, message);
    }
}
