use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_DELETE_CHAR: u16 = 0x0068;
const PACKET_SIZE: usize = 56;
const EMAIL_MAX_BYTES: usize = 50;

/// CH_DELETE_CHAR (0x0068) - Delete character
///
/// Requests deletion of a character with email verification.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character ID: u32 (4 bytes)
/// - Email: [u8; 50] (null-padded)
///
/// Total: 56 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChDeleteCharPacket {
    pub char_id: u32,
    pub email: String,
}

impl ChDeleteCharPacket {
    /// Create a new CH_DELETE_CHAR packet
    ///
    /// # Arguments
    ///
    /// * `char_id` - ID of character to delete
    /// * `email` - Account email for verification
    pub fn new(char_id: u32, email: &str) -> Self {
        Self {
            char_id,
            email: email.to_string(),
        }
    }
}

impl ClientPacket for ChDeleteCharPacket {
    const PACKET_ID: u16 = CH_DELETE_CHAR;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.char_id);

        // Email (50 bytes, null-padded)
        let mut email_bytes = [0u8; EMAIL_MAX_BYTES];
        let email_data = self.email.as_bytes();
        let copy_len = email_data.len().min(EMAIL_MAX_BYTES - 1);
        email_bytes[..copy_len].copy_from_slice(&email_data[..copy_len]);
        buf.put_slice(&email_bytes);

        debug_assert_eq!(buf.len(), PACKET_SIZE, "CH_DELETE_CHAR packet size mismatch");

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_delete_char_serialization() {
        let packet = ChDeleteCharPacket::new(150000, "test@example.com");
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_DELETE_CHAR);
        assert_eq!(u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]), 150000);
    }

    #[test]
    fn test_ch_delete_char_email_truncation() {
        let long_email = "a".repeat(100);
        let packet = ChDeleteCharPacket::new(1, &long_email);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
    }

    #[test]
    fn test_ch_delete_char_packet_id() {
        let packet = ChDeleteCharPacket::new(1, "test@test.com");
        assert_eq!(packet.packet_id(), CH_DELETE_CHAR);
    }
}
