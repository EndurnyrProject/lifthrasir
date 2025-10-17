use crate::infrastructure::networking::protocol::{
    character::types::ZoneServerInfo,
    traits::ServerPacket,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Read};

pub const HC_NOTIFY_ZONESVR: u16 = 0x0071;
const PACKET_SIZE: usize = 28;

/// HC_NOTIFY_ZONESVR (0x0071) - Zone server connection info
///
/// Provides connection information for the zone (map) server after
/// character selection.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Character ID: u32 (4 bytes)
/// - Map Name: [u8; 16] (null-terminated)
/// - IP Address: [u8; 4] (4 bytes)
/// - Port: u16 (2 bytes)
///
/// Total: 28 bytes
///
/// # Direction
/// Character Server → Client
#[derive(Debug, Clone)]
pub struct HcNotifyZonesvrPacket {
    pub zone_server_info: ZoneServerInfo,
}

impl ServerPacket for HcNotifyZonesvrPacket {
    const PACKET_ID: u16 = HC_NOTIFY_ZONESVR;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < PACKET_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("HC_NOTIFY_ZONESVR packet too short: {} bytes", data.len()),
            ));
        }

        let mut cursor = Cursor::new(data);

        // Skip packet ID
        cursor.set_position(2);

        let char_id = cursor.read_u32::<LittleEndian>()?;

        // Read map name (16 bytes)
        let mut map_bytes = [0u8; 16];
        cursor.read_exact(&mut map_bytes)?;
        let map_name = String::from_utf8_lossy(&map_bytes)
            .trim_end_matches('\0')
            .to_string();

        // Read IP address
        let mut ip = [0u8; 4];
        cursor.read_exact(&mut ip)?;

        let port = cursor.read_u16::<LittleEndian>()?;

        Ok(Self {
            zone_server_info: ZoneServerInfo {
                char_id,
                map_name,
                ip,
                port,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hc_notify_zonesvr_parse() {
        let mut data = vec![0u8; PACKET_SIZE];
        data[0..2].copy_from_slice(&HC_NOTIFY_ZONESVR.to_le_bytes());
        data[2..6].copy_from_slice(&150000u32.to_le_bytes());
        data[6..22].copy_from_slice(b"prontera\0\0\0\0\0\0\0\0");
        data[22..26].copy_from_slice(&[127, 0, 0, 1]);
        data[26..28].copy_from_slice(&6900u16.to_le_bytes());

        let packet = HcNotifyZonesvrPacket::parse(&data).unwrap();
        assert_eq!(packet.zone_server_info.char_id, 150000);
        assert_eq!(packet.zone_server_info.map_name, "prontera");
        assert_eq!(packet.zone_server_info.ip, [127, 0, 0, 1]);
        assert_eq!(packet.zone_server_info.port, 6900);
        assert_eq!(packet.zone_server_info.ip_string(), "127.0.0.1");
    }
}
