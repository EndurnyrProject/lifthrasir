use crate::infrastructure::networking::protocol::traits::ClientPacket;
use bytes::{BufMut, Bytes, BytesMut};

pub const CH_ENTER: u16 = 0x0065;
const PACKET_SIZE: usize = 17;

/// CH_ENTER (0x0065) - Enter character server
///
/// Sends authentication data to the character server after successful login.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Account ID: u32 (4 bytes)
/// - Login ID1: u32 (4 bytes)
/// - Login ID2: u32 (4 bytes)
/// - Unknown: u16 (2 bytes)
/// - Sex: u8 (1 byte)
///
/// Total: 17 bytes
///
/// # Direction
/// Client → Character Server
#[derive(Debug, Clone)]
pub struct ChEnterPacket {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub unknown: u16,
    pub sex: u8,
}

impl ChEnterPacket {
    /// Create a new CH_ENTER packet
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID from login server
    /// * `login_id1` - Login ID 1 from login server
    /// * `login_id2` - Login ID 2 from login server
    /// * `sex` - Character sex (0 = female, 1 = male)
    pub fn new(account_id: u32, login_id1: u32, login_id2: u32, sex: u8) -> Self {
        Self {
            account_id,
            login_id1,
            login_id2,
            unknown: 0,
            sex,
        }
    }
}

impl ClientPacket for ChEnterPacket {
    const PACKET_ID: u16 = CH_ENTER;

    fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(PACKET_SIZE);

        buf.put_u16_le(Self::PACKET_ID);
        buf.put_u32_le(self.account_id);
        buf.put_u32_le(self.login_id1);
        buf.put_u32_le(self.login_id2);
        buf.put_u16_le(self.unknown);
        buf.put_u8(self.sex);

        debug_assert_eq!(buf.len(), PACKET_SIZE, "CH_ENTER packet size mismatch");

        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_enter_serialization() {
        let packet = ChEnterPacket::new(123456, 789012, 345678, 1);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), PACKET_SIZE);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), CH_ENTER);
        assert_eq!(
            u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            123456
        );
    }

    #[test]
    fn test_ch_enter_packet_id() {
        let packet = ChEnterPacket::new(1, 2, 3, 0);
        assert_eq!(packet.packet_id(), CH_ENTER);
    }
}
