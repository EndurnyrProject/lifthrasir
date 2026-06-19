use crate::domain::inventory::Item;
use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_INVENTORY_ITEMLIST_NORMAL: u16 = 0x0B09;

/// Stackable (non-equipped) inventory item, 34 bytes on the wire.
#[derive(Debug, Clone)]
pub struct NormalItem {
    pub index: u16,
    pub nameid: u32,
    pub item_type: u8,
    pub amount: u16,
    pub wear_state: u32,
    pub cards: [u32; 4],
    pub expire_time: u32,
    pub flag: u8,
}

impl NormalItem {
    const ITEM_SIZE: usize = 34;

    fn parse(data: &[u8]) -> Self {
        let mut buf = data;

        let index = buf.get_u16_le();
        let nameid = buf.get_u32_le();
        let item_type = buf.get_u8();
        let amount = buf.get_u16_le();
        let wear_state = buf.get_u32_le();
        let cards = [
            buf.get_u32_le(),
            buf.get_u32_le(),
            buf.get_u32_le(),
            buf.get_u32_le(),
        ];
        let expire_time = buf.get_u32_le();
        let flag = buf.get_u8();

        Self {
            index,
            nameid,
            item_type,
            amount,
            wear_state,
            cards,
            expire_time,
            flag,
        }
    }
}

impl From<&NormalItem> for Item {
    fn from(item: &NormalItem) -> Self {
        Item {
            index: item.index,
            item_id: item.nameid,
            item_type: item.item_type,
            amount: item.amount,
            location: 0,
            wear_state: item.wear_state,
            refine: 0,
            cards: item.cards,
            options: Vec::new(),
            expire_time: item.expire_time,
            view_sprite: 0,
            identified: item.flag & 0b1 != 0,
            damaged: false,
        }
    }
}

/// ZC_INVENTORY_ITEMLIST_NORMAL (0x0B09) - Zone Server → Client
///
/// Variable-length packet: `[id u16][len u16][inv_type u8][items...]`.
/// Each item is 34 bytes; the items region is `len - 5` bytes.
#[derive(Debug, Clone)]
pub struct ZcInventoryItemlistNormalPacket {
    pub items: Vec<NormalItem>,
}

impl ServerPacket for ZcInventoryItemlistNormalPacket {
    const PACKET_ID: u16 = ZC_INVENTORY_ITEMLIST_NORMAL;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_ITEMLIST_NORMAL packet too short: expected at least 5 bytes, got {}",
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
                    "ZC_INVENTORY_ITEMLIST_NORMAL incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let header_size = 5;
        let items_data_size = packet_length - header_size;

        if !items_data_size.is_multiple_of(NormalItem::ITEM_SIZE) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_ITEMLIST_NORMAL invalid items data size: {} bytes is not a multiple of {} (item size)",
                    items_data_size,
                    NormalItem::ITEM_SIZE
                ),
            ));
        }

        let item_count = items_data_size / NormalItem::ITEM_SIZE;
        let mut items = Vec::with_capacity(item_count);

        for i in 0..item_count {
            let start = header_size + (i * NormalItem::ITEM_SIZE);
            let end = start + NormalItem::ITEM_SIZE;
            items.push(NormalItem::parse(&data[start..end]));
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

    fn build_item(index: u16, nameid: u32, amount: u16, flag: u8) -> Vec<u8> {
        let mut item = Vec::with_capacity(NormalItem::ITEM_SIZE);
        item.extend_from_slice(&index.to_le_bytes());
        item.extend_from_slice(&nameid.to_le_bytes());
        item.push(3);
        item.extend_from_slice(&amount.to_le_bytes());
        item.extend_from_slice(&0u32.to_le_bytes());
        item.extend_from_slice(&[0u8; 16]);
        item.extend_from_slice(&0u32.to_le_bytes());
        item.push(flag);
        assert_eq!(item.len(), NormalItem::ITEM_SIZE);
        item
    }

    fn build_packet(items: &[Vec<u8>]) -> Vec<u8> {
        let body: usize = items.iter().map(|i| i.len()).sum();
        let len = (5 + body) as u16;
        let mut data = vec![0x09, 0x0B];
        data.extend_from_slice(&len.to_le_bytes());
        data.push(0);
        for item in items {
            data.extend_from_slice(item);
        }
        data
    }

    #[test]
    fn parses_single_34_byte_item() {
        let data = build_packet(&[build_item(7, 501, 5, 0b1)]);
        let packet = ZcInventoryItemlistNormalPacket::parse(&data).expect("parse");
        assert_eq!(packet.items.len(), 1);
        let item = &packet.items[0];
        assert_eq!(item.index, 7);
        assert_eq!(item.nameid, 501);
        assert_eq!(item.amount, 5);
        assert!(item.flag & 0b1 != 0);
    }

    #[test]
    fn non_multiple_length_is_err() {
        let mut data = build_packet(&[build_item(1, 1, 1, 0)]);
        data.push(0xFF);
        let len = data.len() as u16;
        data[2..4].copy_from_slice(&len.to_le_bytes());
        assert!(ZcInventoryItemlistNormalPacket::parse(&data).is_err());
    }

    #[test]
    fn maps_into_item_with_defaults() {
        let data = build_packet(&[build_item(7, 501, 5, 0b1)]);
        let packet = ZcInventoryItemlistNormalPacket::parse(&data).expect("parse");
        let item: Item = (&packet.items[0]).into();
        assert_eq!(item.index, 7);
        assert_eq!(item.amount, 5);
        assert_eq!(item.location, 0);
        assert_eq!(item.refine, 0);
        assert_eq!(item.view_sprite, 0);
        assert!(item.options.is_empty());
        assert!(item.identified);
        assert!(!item.damaged);
        assert!(!item.is_equipped());
    }

    #[test]
    fn unidentified_flag_decodes() {
        let data = build_packet(&[build_item(7, 501, 1, 0b0)]);
        let packet = ZcInventoryItemlistNormalPacket::parse(&data).expect("parse");
        let item: Item = (&packet.items[0]).into();
        assert!(!item.identified);
    }
}
