use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read};

// Packet IDs for zone server communication
pub const CZ_ENTER2: u16 = 0x0436;
pub const ZC_ACCEPT_ENTER: u16 = 0x02EB;
pub const ZC_AID: u16 = 0x0283;
pub const CZ_NOTIFY_ACTORINIT: u16 = 0x007D;
pub const ZC_REFUSE_ENTER: u16 = 0x0074;

/// CZ_ENTER2 (0x0436) - Client → Zone Server
/// Initial packet sent when entering the zone server
/// Size: 23 bytes
#[derive(Debug, Clone)]
pub struct CzEnter2Packet {
    pub account_id: u32,
    pub char_id: u32,
    pub auth_code: u32,
    pub client_time: u32,
    pub unknown: u32,
    pub sex: u8,
}

impl CzEnter2Packet {
    /// Serialize the packet to bytes for sending to the zone server
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(23);
        buf.write_u16::<LittleEndian>(CZ_ENTER2).unwrap();
        buf.write_u32::<LittleEndian>(self.account_id).unwrap();
        buf.write_u32::<LittleEndian>(self.char_id).unwrap();
        buf.write_u32::<LittleEndian>(self.auth_code).unwrap();
        buf.write_u32::<LittleEndian>(self.client_time).unwrap();
        buf.write_u32::<LittleEndian>(self.unknown).unwrap();
        buf.write_u8(self.sex).unwrap();
        buf
    }
}

/// ZC_ACCEPT_ENTER (0x02EB) - Zone Server → Client
/// Sent when zone server accepts the player into the map
/// Size: 13 bytes
#[derive(Debug, Clone)]
pub struct ZcAcceptEnterPacket {
    pub start_time: u32,
    pub x: u16,
    pub y: u16,
    pub dir: u8,
    pub x_size: u8,
    pub y_size: u8,
    pub font: u16,
}

impl ZcAcceptEnterPacket {
    /// Parse packet from received bytes
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let start_time = cursor.read_u32::<LittleEndian>()?;

        // Read and decode position+direction (3 bytes)
        let mut pos_dir = [0u8; 3];
        cursor.read_exact(&mut pos_dir)?;
        let (x, y, dir) = decode_position(pos_dir);

        let x_size = cursor.read_u8()?;
        let y_size = cursor.read_u8()?;
        let font = cursor.read_u16::<LittleEndian>()?;

        Ok(Self {
            start_time,
            x,
            y,
            dir,
            x_size,
            y_size,
            font,
        })
    }
}

/// ZC_AID (0x0283) - Zone Server → Client
/// Sends the account ID to the client after accepting entry
/// Size: 6 bytes
#[derive(Debug, Clone)]
pub struct ZcAidPacket {
    pub account_id: u32,
}

impl ZcAidPacket {
    /// Parse packet from received bytes
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let account_id = cursor.read_u32::<LittleEndian>()?;

        Ok(Self { account_id })
    }
}

/// CZ_NOTIFY_ACTORINIT (0x007D) - Client → Zone Server
/// Notifies zone server that the client is ready to receive actor information
/// Size: 2 bytes (just packet ID)
#[derive(Debug, Clone)]
pub struct CzNotifyActorinitPacket;

impl CzNotifyActorinitPacket {
    /// Build the packet (static method since it has no fields)
    pub fn build() -> Vec<u8> {
        let mut buf = Vec::with_capacity(2);
        buf.write_u16::<LittleEndian>(CZ_NOTIFY_ACTORINIT).unwrap();
        buf
    }
}

/// ZC_REFUSE_ENTER (0x0074) - Zone Server → Client
/// Sent when zone server refuses entry
/// Size: 3 bytes
#[derive(Debug, Clone)]
pub struct ZcRefuseEnterPacket {
    pub error_code: u8,
}

impl ZcRefuseEnterPacket {
    /// Parse packet from received bytes
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(data);

        let _packet_id = cursor.read_u16::<LittleEndian>()?;
        let error_code = cursor.read_u8()?;

        Ok(Self { error_code })
    }

    /// Get a human-readable description of the error code
    pub fn error_description(&self) -> &'static str {
        match self.error_code {
            0 => "Normal (no error)",
            1 => "Server closed",
            2 => "Someone has already logged in with this ID",
            3 => "Already logged in",
            4 => "Environment error (?)",
            8 => "Server still recognizes last connection",
            _ => "Unknown error",
        }
    }
}

/// Encode position and direction into 3 bytes
/// Format: X and Y are 10-bit values, direction is 4-bit
fn encode_position(x: u16, y: u16, dir: u8) -> [u8; 3] {
    let byte0 = (x >> 2) as u8;
    let byte1 = (((x << 6) | ((y >> 4) & 0x3F)) & 0xFF) as u8;
    let byte2 = (((y << 4) | (dir as u16 & 0x0F)) & 0xFF) as u8;
    [byte0, byte1, byte2]
}

/// Decode position and direction from 3 bytes
/// Returns: (x, y, direction)
fn decode_position(pos_dir: [u8; 3]) -> (u16, u16, u8) {
    let x = ((pos_dir[0] as u16) << 2) | ((pos_dir[1] as u16) >> 6);
    let y = (((pos_dir[1] as u16) & 0x3F) << 4) | ((pos_dir[2] as u16) >> 4);
    let dir = (pos_dir[2] & 0x0F) as u8;
    (x, y, dir)
}

/// Convert IP byte array to string representation
pub fn ip_array_to_string(ip: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_encoding_decoding() {
        let test_cases = vec![
            (100, 200, 3),
            (0, 0, 0),
            (1023, 1023, 15), // Max values (10-bit for x/y, 4-bit for dir)
            (512, 512, 7),
        ];

        for (x, y, dir) in test_cases {
            let encoded = encode_position(x, y, dir);
            let (decoded_x, decoded_y, decoded_dir) = decode_position(encoded);
            assert_eq!(x, decoded_x, "X coordinate mismatch");
            assert_eq!(y, decoded_y, "Y coordinate mismatch");
            assert_eq!(dir, decoded_dir, "Direction mismatch");
        }
    }

    #[test]
    fn test_cz_enter2_serialization() {
        let packet = CzEnter2Packet {
            account_id: 12345,
            char_id: 67890,
            auth_code: 11111,
            client_time: 22222,
            unknown: 0,
            sex: 1,
        };

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 23, "Packet size should be 23 bytes");

        // Verify packet ID
        let mut cursor = Cursor::new(&bytes);
        let packet_id = cursor.read_u16::<LittleEndian>().unwrap();
        assert_eq!(packet_id, CZ_ENTER2);
    }

    #[test]
    fn test_cz_notify_actorinit_build() {
        let bytes = CzNotifyActorinitPacket::build();
        assert_eq!(bytes.len(), 2, "Packet size should be 2 bytes");

        let mut cursor = Cursor::new(&bytes);
        let packet_id = cursor.read_u16::<LittleEndian>().unwrap();
        assert_eq!(packet_id, CZ_NOTIFY_ACTORINIT);
    }

    #[test]
    fn test_ip_array_to_string() {
        assert_eq!(ip_array_to_string([127, 0, 0, 1]), "127.0.0.1");
        assert_eq!(ip_array_to_string([192, 168, 1, 1]), "192.168.1.1");
    }
}
