use crate::infrastructure::networking::protocol::{
    character::types::SecondPasswordState,
    traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};

pub const HC_SECOND_PASSWD_LOGIN: u16 = 0x08B9;
const PACKET_SIZE: usize = 12;

/// HC_SECOND_PASSWD_LOGIN (0x08B9) - Second password (pincode) request
///
/// Server requests second password/pincode authentication or indicates
/// its state.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Seed: u32 (4 bytes)
/// - Account ID: u32 (4 bytes)
/// - State: u16 (2 bytes)
///
/// Total: 12 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcSecondPasswdLoginPacket {
    pub seed: u32,
    pub account_id: u32,
    pub state: SecondPasswordState,
}

impl ServerPacket for HcSecondPasswdLoginPacket {
    const PACKET_ID: u16 = HC_SECOND_PASSWD_LOGIN;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HC_SECOND_PASSWD_LOGIN packet too short",
            ));
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(2); // Skip packet ID

        let seed = cursor.read_u32::<LittleEndian>()?;
        let account_id = cursor.read_u32::<LittleEndian>()?;
        let state_code = cursor.read_u16::<LittleEndian>()?;

        Ok(Self {
            seed,
            account_id,
            state: SecondPasswordState::from(state_code),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_second_passwd_login_parse() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_SECOND_PASSWD_LOGIN.to_le_bytes());
        data[2..6].copy_from_slice(&12345u32.to_le_bytes());
        data[6..10].copy_from_slice(&67890u32.to_le_bytes());
        data[10..12].copy_from_slice(&0u16.to_le_bytes());

        let packet = HcSecondPasswdLoginPacket::parse(&data).unwrap();
        assert_eq!(packet.seed, 12345);
        assert_eq!(packet.account_id, 67890);
        assert_eq!(packet.state, SecondPasswordState::Disabled);
    }
}
