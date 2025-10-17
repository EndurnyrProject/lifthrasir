use crate::infrastructure::networking::protocol::{
    character::types::CharacterSlotInfo,
    traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Read};

pub const HC_CHARACTER_LIST: u16 = 0x082D;
const PACKET_SIZE: usize = 29;

/// HC_CHARACTER_LIST (0x082D) - Character slot information
///
/// Provides information about character slots (normal, premium, billing).
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes, always 29)
/// - Normal Slots: u8 (1 byte)
/// - Premium Slots: u8 (1 byte)
/// - Billing Slots: u8 (1 byte)
/// - Producible Slots: u8 (1 byte)
/// - Valid Slots: u8 (1 byte)
/// - Unknown: [u8; 20] (20 bytes)
///
/// Total: 29 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcCharacterListPacket {
    pub slot_info: CharacterSlotInfo,
}

impl ServerPacket for HcCharacterListPacket {
    const PACKET_ID: u16 = HC_CHARACTER_LIST;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("HC_CHARACTER_LIST packet too short: {} bytes", data.len()),
            ));
        }

        let mut cursor = Cursor::new(data);

        // Skip packet ID
        cursor.set_position(2);

        let packet_len = cursor.read_u16::<LittleEndian>()?;
        if packet_len != PACKET_SIZE as u16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid HC_CHARACTER_LIST length: {}", packet_len),
            ));
        }

        let normal_slots = cursor.read_u8()?;
        let premium_slots = cursor.read_u8()?;
        let billing_slots = cursor.read_u8()?;
        let producible_slots = cursor.read_u8()?;
        let valid_slots = cursor.read_u8()?;

        // Skip 20 unknown bytes
        let mut unknown = [0u8; 20];
        cursor.read_exact(&mut unknown)?;

        Ok(Self {
            slot_info: CharacterSlotInfo {
                normal_slots,
                premium_slots,
                billing_slots,
                producible_slots,
                valid_slots,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_character_list_parse() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_CHARACTER_LIST.to_le_bytes());
        data[2..4].copy_from_slice(&(PACKET_SIZE as u16).to_le_bytes());
        data[4] = 9;  // normal_slots
        data[5] = 6;  // premium_slots
        data[6] = 0;  // billing_slots
        data[7] = 9;  // producible_slots
        data[8] = 9;  // valid_slots

        let packet = HcCharacterListPacket::parse(&data).unwrap();
        assert_eq!(packet.slot_info.normal_slots, 9);
        assert_eq!(packet.slot_info.premium_slots, 6);
        assert_eq!(packet.slot_info.billing_slots, 0);
        assert_eq!(packet.slot_info.producible_slots, 9);
        assert_eq!(packet.slot_info.valid_slots, 9);
    }
}
