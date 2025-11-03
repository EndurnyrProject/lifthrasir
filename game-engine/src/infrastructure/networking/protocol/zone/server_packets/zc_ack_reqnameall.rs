use crate::infrastructure::networking::protocol::traits::ServerPacket;
use bytes::Buf;
use std::io;

pub const ZC_ACK_REQNAMEALL: u16 = 0x0195;

/// ZC_ACK_REQNAMEALL (0x0195) - Zone Server → Client
///
/// Response to CZ_REQNAME2 containing full character details including
/// party name, guild name, and guild position.
///
/// # Packet Structure
/// ```text
/// Size: 102 bytes
/// +--------+----------------+----------+------+-------------------------------------+
/// | Offset | Field          | Type     | Size | Description                         |
/// +--------+----------------+----------+------+-------------------------------------+
/// | 0      | packet_id      | u16      | 2    | 0x0195                              |
/// | 2      | gid            | u32      | 4    | Game ID (Character ID)              |
/// | 6      | name           | char[24] | 24   | Character name (null-terminated)    |
/// | 30     | party_name     | char[24] | 24   | Party name (null-terminated)        |
/// | 54     | guild_name     | char[24] | 24   | Guild name (null-terminated)        |
/// | 78     | position_name  | char[24] | 24   | Guild position (null-terminated)    |
/// +--------+----------------+----------+------+-------------------------------------+
/// ```
#[derive(Debug, Clone)]
pub struct ZcAckReqnameallPacket {
    pub gid: u32,
    pub name: String,
    pub party_name: String,
    pub guild_name: String,
    pub position_name: String,
}

impl ServerPacket for ZcAckReqnameallPacket {
    const PACKET_ID: u16 = ZC_ACK_REQNAMEALL;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 102 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_ACK_REQNAMEALL packet too short: expected 102 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let gid = buf.get_u32_le();

        let name = parse_null_terminated_string(&buf[..24]);
        buf.advance(24);

        let party_name = parse_null_terminated_string(&buf[..24]);
        buf.advance(24);

        let guild_name = parse_null_terminated_string(&buf[..24]);
        buf.advance(24);

        let position_name = parse_null_terminated_string(&buf[..24]);

        Ok(Self {
            gid,
            name,
            party_name,
            guild_name,
            position_name,
        })
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
    fn test_zc_ack_reqnameall_parse() {
        let mut data = vec![0u8; 102];

        data[0..2].copy_from_slice(&ZC_ACK_REQNAMEALL.to_le_bytes());

        data[2..6].copy_from_slice(&123456u32.to_le_bytes());

        let name = "TestPlayer";
        data[6..6 + name.len()].copy_from_slice(name.as_bytes());

        let party = "TestParty";
        data[30..30 + party.len()].copy_from_slice(party.as_bytes());

        let guild = "TestGuild";
        data[54..54 + guild.len()].copy_from_slice(guild.as_bytes());

        let position = "Master";
        data[78..78 + position.len()].copy_from_slice(position.as_bytes());

        let packet = ZcAckReqnameallPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.gid, 123456);
        assert_eq!(packet.name, "TestPlayer");
        assert_eq!(packet.party_name, "TestParty");
        assert_eq!(packet.guild_name, "TestGuild");
        assert_eq!(packet.position_name, "Master");
    }

    #[test]
    fn test_zc_ack_reqnameall_parse_invalid_size() {
        let data = vec![0u8; 50];
        let result = ZcAckReqnameallPacket::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_strings() {
        let mut data = vec![0u8; 102];

        data[0..2].copy_from_slice(&ZC_ACK_REQNAMEALL.to_le_bytes());
        data[2..6].copy_from_slice(&123456u32.to_le_bytes());

        let name = "Solo";
        data[6..6 + name.len()].copy_from_slice(name.as_bytes());

        let packet = ZcAckReqnameallPacket::parse(&data).expect("Failed to parse packet");

        assert_eq!(packet.gid, 123456);
        assert_eq!(packet.name, "Solo");
        assert_eq!(packet.party_name, "");
        assert_eq!(packet.guild_name, "");
        assert_eq!(packet.position_name, "");
    }
}
