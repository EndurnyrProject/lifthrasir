use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_ACK_REQNAME: u16 = 0x0095;

/// ZC_ACK_REQNAME (0x0095) - Zone Server → Client
///
/// Response to CZ_REQNAME2 containing the character's name.
/// This is the basic response for player characters.
///
/// # Packet Structure
/// ```text
/// Size: 30 bytes
/// +--------+-------------+----------+------+----------------------------------+
/// | Offset | Field       | Type     | Size | Description                      |
/// +--------+-------------+----------+------+----------------------------------+
/// | 0      | packet_id   | u16      | 2    | 0x0095                           |
/// | 2      | char_id     | u32      | 4    | Character ID                     |
/// | 6      | name        | char[24] | 24   | Character name (null-terminated) |
/// +--------+-------------+----------+------+----------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcAckReqnamePacket {
    pub char_id: u32,
    pub name: String,
}

impl ServerPacket for ZcAckReqnamePacket {
    const PACKET_ID: u16 = ZC_ACK_REQNAME;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 30 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_ACK_REQNAME packet too short: expected 30 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let char_id = buf.get_u32_le();

        let name_bytes = &buf[..24];
        let name = parse_null_terminated_string(name_bytes);

        Ok(Self { char_id, name })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

fn parse_null_terminated_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zc_ack_reqname_parse() {
        let mut data = vec![0u8; 30];

        data[0..2].copy_from_slice(&ZC_ACK_REQNAME.to_le_bytes());

        data[2..6].copy_from_slice(&123456u32.to_le_bytes());

        let name = "TestPlayer";
        data[6..6 + name.len()].copy_from_slice(name.as_bytes());

        let packet = ZcAckReqnamePacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.char_id, 123456);
        assert_eq!(packet.name, "TestPlayer");
    }

    #[test]
    fn test_zc_ack_reqname_parse_invalid_size() {
        let data = vec![0u8; 10];
        let result = ZcAckReqnamePacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_null_terminated_string() {
        let bytes = b"Hello\0World\0\0\0\0\0\0\0\0\0";
        let result = parse_null_terminated_string(bytes);
        assert_eq!(result, "Hello");
    }
}
