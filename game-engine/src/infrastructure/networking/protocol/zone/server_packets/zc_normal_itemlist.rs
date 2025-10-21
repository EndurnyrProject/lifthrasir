use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_NORMAL_ITEMLIST: u16 = 0x00A3;

/// Item entry in the normal item list
///
/// Represents a single inventory item (non-equipment) with all its properties.
/// Each item is exactly 34 bytes in the packet structure.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub index: u16,
    pub nameid: u16,
    pub amount: u16,
    pub item_type: u8,
    pub identify: u8,
    pub attribute: u8,
    pub refine: u8,
    pub card0: u16,
    pub card1: u16,
    pub card2: u16,
    pub card3: u16,
    pub expire_time: u32,
    pub favorite: u8,
    pub bound: u8,
    pub random_options: [u8; 10],
}

impl InventoryItem {
    const ITEM_SIZE: usize = 34;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < Self::ITEM_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "InventoryItem data too short: expected {} bytes, got {}",
                    Self::ITEM_SIZE,
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        let index = buf.get_u16_le();
        let nameid = buf.get_u16_le();
        let amount = buf.get_u16_le();
        let item_type = buf.get_u8();
        let identify = buf.get_u8();
        let attribute = buf.get_u8();
        let refine = buf.get_u8();
        let card0 = buf.get_u16_le();
        let card1 = buf.get_u16_le();
        let card2 = buf.get_u16_le();
        let card3 = buf.get_u16_le();
        let expire_time = buf.get_u32_le();
        let favorite = buf.get_u8();
        let bound = buf.get_u8();

        let mut random_options = [0u8; 10];
        buf.copy_to_slice(&mut random_options);

        Ok(Self {
            index,
            nameid,
            amount,
            item_type,
            identify,
            attribute,
            refine,
            card0,
            card1,
            card2,
            card3,
            expire_time,
            favorite,
            bound,
            random_options,
        })
    }
}

/// ZC_NORMAL_ITEMLIST (0x00A3) - Zone Server → Client
///
/// Sends the player's regular inventory items (non-equipment) to the client.
/// This packet is sent during the login sequence after LoadEndAck.
///
/// Only includes items where equip == 0 (non-equipped items).
/// Equipment items are sent via ZC_EQUIPITEM_LIST instead.
///
/// # Packet Structure
/// Variable-length packet with structure: [packet_id:u16][length:u16][items...]
///
/// Each item is 34 bytes containing:
/// - index: inventory slot (0-based)
/// - nameid: item ID from item database
/// - amount: item count/stack size
/// - item_type: item type (0=usable, 2=usable, 3=etc, 6=pet egg, 7=weapon, 8=armor, etc.)
/// - identify: identification flag (0=unidentified, 1=identified)
/// - attribute: item attribute (0=normal, 1=broken)
/// - refine: refinement/upgrade level
/// - card[0-3]: four card slots for inserted cards
/// - expire_time: expiration timestamp (Unix time, 0 = no expiration)
/// - favorite: favorite flag
/// - bound: bound type
/// - random_options: random option data (10 bytes, currently zeros)
///
/// All fields are little-endian.
#[derive(Debug, Clone)]
pub struct ZcNormalItemlistPacket {
    pub items: Vec<InventoryItem>,
}

impl ServerPacket for ZcNormalItemlistPacket {
    const PACKET_ID: u16 = ZC_NORMAL_ITEMLIST;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NORMAL_ITEMLIST packet too short: expected at least 4 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let packet_length = buf.get_u16_le();
        if data.len() < packet_length as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NORMAL_ITEMLIST incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let header_size = 4;
        let items_data_size = packet_length as usize - header_size;

        if items_data_size % InventoryItem::ITEM_SIZE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NORMAL_ITEMLIST invalid items data size: {} bytes is not a multiple of {} (item size)",
                    items_data_size,
                    InventoryItem::ITEM_SIZE
                ),
            ));
        }

        let item_count = items_data_size / InventoryItem::ITEM_SIZE;
        let mut items = Vec::with_capacity(item_count);

        for i in 0..item_count {
            let start = header_size + (i * InventoryItem::ITEM_SIZE);
            let end = start + InventoryItem::ITEM_SIZE;

            if end > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "Not enough data for item {}: need {} bytes, only {} available",
                        i,
                        InventoryItem::ITEM_SIZE,
                        data.len() - start
                    ),
                ));
            }

            let item_data = &data[start..end];
            let item = InventoryItem::parse(item_data)?;
            items.push(item);
        }

        Ok(Self { items })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}
