use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_INVENTORY_START: u16 = 0x0B08;

/// ZC_INVENTORY_START (0x0B08) - Zone Server → Client
///
/// Begin-transaction marker for an inventory dump. Variable layout:
/// `[id u16][len u16][inv_type u8][name bytes + NUL]`. The name region is
/// `len - 5` bytes; the name is taken up to the first NUL.
#[derive(Debug, Clone)]
pub struct ZcInventoryStartPacket {
    pub inv_type: u8,
    pub name: String,
}

impl ServerPacket for ZcInventoryStartPacket {
    const PACKET_ID: u16 = ZC_INVENTORY_START;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_INVENTORY_START packet too short: expected at least 5 bytes, got {}",
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
                    "ZC_INVENTORY_START incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let inv_type = buf.get_u8();

        let name_bytes = &data[5..packet_length];
        let end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(name_bytes.len());
        let name = String::from_utf8_lossy(&name_bytes[..end]).into_owned();

        Ok(Self { inv_type, name })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(inv_type: u8, name: &[u8]) -> Vec<u8> {
        let len = (5 + name.len()) as u16;
        let mut data = vec![0x08, 0x0B];
        data.extend_from_slice(&len.to_le_bytes());
        data.push(inv_type);
        data.extend_from_slice(name);
        data
    }

    #[test]
    fn parses_name_with_nul() {
        let data = build(0, b"Player\0");
        let packet = ZcInventoryStartPacket::parse(&data).expect("parse");
        assert_eq!(packet.inv_type, 0);
        assert_eq!(packet.name, "Player");
    }

    #[test]
    fn parses_empty_name() {
        let data = build(0, b"\0");
        let packet = ZcInventoryStartPacket::parse(&data).expect("parse");
        assert_eq!(packet.name, "");
    }

    #[test]
    fn too_short_is_err() {
        assert!(ZcInventoryStartPacket::parse(&[0x08, 0x0B, 0x04]).is_err());
    }
}
