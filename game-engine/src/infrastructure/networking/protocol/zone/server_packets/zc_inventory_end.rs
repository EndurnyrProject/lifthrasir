use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_INVENTORY_END: u16 = 0x0B0B;

/// ZC_INVENTORY_END (0x0B0B) - Zone Server → Client
///
/// End-of-transaction marker for an inventory dump. Fixed 4 bytes:
/// `[id u16][inv_type u8][flag u8]` (flag 0 = success).
#[derive(Debug, Clone)]
pub struct ZcInventoryEndPacket {
    pub inv_type: u8,
    pub flag: u8,
}

impl ServerPacket for ZcInventoryEndPacket {
    const PACKET_ID: u16 = ZC_INVENTORY_END;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_END packet too short: expected 4 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;
        buf.advance(2);

        let inv_type = buf.get_u8();
        let flag = buf.get_u8();

        Ok(Self { inv_type, flag })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixed_four_bytes() {
        let data = [0x0B, 0x0B, 0x00, 0x01];
        let packet = ZcInventoryEndPacket::parse(&data).expect("parse");
        assert_eq!(packet.inv_type, 0);
        assert_eq!(packet.flag, 1);
    }

    #[test]
    fn too_short_is_err() {
        assert!(ZcInventoryEndPacket::parse(&[0x0B, 0x0B, 0x00]).is_err());
    }
}
