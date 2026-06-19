use crate::domain::inventory::{Item, ItemOption};
use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_INVENTORY_ITEMLIST_EQUIP: u16 = 0x0B0A;

/// Equippable inventory item, 67 bytes on the wire.
#[derive(Debug, Clone)]
pub struct EquipItem {
    pub index: u16,
    pub nameid: u32,
    pub item_type: u8,
    pub location: u32,
    pub wear_state: u32,
    pub refine: u8,
    pub cards: [u32; 4],
    pub expire_time: u32,
    pub bind_on_equip: u16,
    pub sprite: u16,
    pub options: Vec<ItemOption>,
    pub flag: u8,
}

impl EquipItem {
    const ITEM_SIZE: usize = 67;
    const MAX_OPTIONS: usize = 5;

    fn parse(data: &[u8]) -> Self {
        let mut buf = data;

        let index = buf.get_u16_le();
        let nameid = buf.get_u32_le();
        let item_type = buf.get_u8();
        let location = buf.get_u32_le();
        let wear_state = buf.get_u32_le();
        let refine = buf.get_u8();
        let cards = [
            buf.get_u32_le(),
            buf.get_u32_le(),
            buf.get_u32_le(),
            buf.get_u32_le(),
        ];
        let expire_time = buf.get_u32_le();
        let bind_on_equip = buf.get_u16_le();
        let sprite = buf.get_u16_le();
        let option_count = buf.get_u8() as usize;

        let mut options = Vec::with_capacity(option_count.min(Self::MAX_OPTIONS));
        for slot in 0..Self::MAX_OPTIONS {
            let index = buf.get_u16_le();
            let value = buf.get_u16_le();
            let param = buf.get_u8();
            if slot < option_count {
                options.push(ItemOption {
                    index,
                    value,
                    param,
                });
            }
        }

        let flag = buf.get_u8();

        Self {
            index,
            nameid,
            item_type,
            location,
            wear_state,
            refine,
            cards,
            expire_time,
            bind_on_equip,
            sprite,
            options,
            flag,
        }
    }
}

impl From<&EquipItem> for Item {
    fn from(item: &EquipItem) -> Self {
        Item {
            index: item.index,
            item_id: item.nameid,
            item_type: item.item_type,
            amount: 1,
            location: item.location,
            wear_state: item.wear_state,
            refine: item.refine,
            cards: item.cards,
            options: item.options.clone(),
            expire_time: item.expire_time,
            view_sprite: item.sprite,
            identified: item.flag & 0b1 != 0,
            damaged: item.flag & 0b10 != 0,
        }
    }
}

/// ZC_INVENTORY_ITEMLIST_EQUIP (0x0B0A) - Zone Server → Client
///
/// Variable-length packet: `[id u16][len u16][inv_type u8][items...]`.
/// Each item is 67 bytes; the items region is `len - 5` bytes.
#[derive(Debug, Clone)]
pub struct ZcInventoryItemlistEquipPacket {
    pub items: Vec<EquipItem>,
}

impl ServerPacket for ZcInventoryItemlistEquipPacket {
    const PACKET_ID: u16 = ZC_INVENTORY_ITEMLIST_EQUIP;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_ITEMLIST_EQUIP packet too short: expected at least 5 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;
        buf.advance(2);

        let packet_length = buf.get_u16_le() as usize;
        if data.len() < packet_length || packet_length < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_ITEMLIST_EQUIP incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let header_size = 5;
        let items_data_size = packet_length - header_size;

        if !items_data_size.is_multiple_of(EquipItem::ITEM_SIZE) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_ITEMLIST_EQUIP invalid items data size: {} bytes is not a multiple of {} (item size)",
                    items_data_size,
                    EquipItem::ITEM_SIZE
                ),
            ));
        }

        let item_count = items_data_size / EquipItem::ITEM_SIZE;
        let mut items = Vec::with_capacity(item_count);

        for i in 0..item_count {
            let start = header_size + (i * EquipItem::ITEM_SIZE);
            let end = start + EquipItem::ITEM_SIZE;
            items.push(EquipItem::parse(&data[start..end]));
        }

        Ok(Self { items })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_item(
        index: u16,
        nameid: u32,
        location: u32,
        wear_state: u32,
        sprite: u16,
        flag: u8,
    ) -> Vec<u8> {
        let mut item = Vec::with_capacity(EquipItem::ITEM_SIZE);
        item.extend_from_slice(&index.to_le_bytes());
        item.extend_from_slice(&nameid.to_le_bytes());
        item.push(5);
        item.extend_from_slice(&location.to_le_bytes());
        item.extend_from_slice(&wear_state.to_le_bytes());
        item.push(7);
        item.extend_from_slice(&[0u8; 16]);
        item.extend_from_slice(&0u32.to_le_bytes());
        item.extend_from_slice(&0u16.to_le_bytes());
        item.extend_from_slice(&sprite.to_le_bytes());
        item.push(0);
        item.extend_from_slice(&[0u8; 25]);
        item.push(flag);
        assert_eq!(item.len(), EquipItem::ITEM_SIZE);
        item
    }

    fn build_packet(items: &[Vec<u8>]) -> Vec<u8> {
        let body: usize = items.iter().map(|i| i.len()).sum();
        let len = (5 + body) as u16;
        let mut data = vec![0x0A, 0x0B];
        data.extend_from_slice(&len.to_le_bytes());
        data.push(0);
        for item in items {
            data.extend_from_slice(item);
        }
        data
    }

    #[test]
    fn parses_single_67_byte_item() {
        let data = build_packet(&[build_item(9, 1201, 2, 2, 13, 0b1)]);
        let packet = ZcInventoryItemlistEquipPacket::parse(&data).expect("parse");
        assert_eq!(packet.items.len(), 1);
        let item = &packet.items[0];
        assert_eq!(item.index, 9);
        assert_eq!(item.nameid, 1201);
        assert_eq!(item.location, 2);
        assert_eq!(item.wear_state, 2);
        assert_eq!(item.sprite, 13);
        assert_eq!(item.refine, 7);
        assert!(item.options.is_empty());
    }

    #[test]
    fn non_multiple_length_is_err() {
        let mut data = build_packet(&[build_item(1, 1, 1, 1, 1, 0)]);
        data.push(0xFF);
        let len = data.len() as u16;
        data[2..4].copy_from_slice(&len.to_le_bytes());
        assert!(ZcInventoryItemlistEquipPacket::parse(&data).is_err());
    }

    #[test]
    fn maps_into_item() {
        let data = build_packet(&[build_item(9, 1201, 2, 2, 13, 0b11)]);
        let packet = ZcInventoryItemlistEquipPacket::parse(&data).expect("parse");
        let item: Item = (&packet.items[0]).into();
        assert_eq!(item.index, 9);
        assert_eq!(item.amount, 1);
        assert_eq!(item.location, 2);
        assert_eq!(item.refine, 7);
        assert_eq!(item.view_sprite, 13);
        assert_eq!(item.wear_state, 2);
        assert!(item.is_equipped());
        assert!(item.identified);
        assert!(item.damaged);
    }

    #[test]
    fn not_equipped_when_wear_state_zero() {
        let data = build_packet(&[build_item(9, 1201, 2, 0, 13, 0b1)]);
        let packet = ZcInventoryItemlistEquipPacket::parse(&data).expect("parse");
        let item: Item = (&packet.items[0]).into();
        assert!(!item.is_equipped());
        assert!(!item.damaged);
    }
}
