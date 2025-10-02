use byteorder::{LittleEndian, WriteBytesExt};
use nom::{bytes::complete::take, number::complete::le_u16, IResult};
use serde::{Deserialize, Serialize};

// Protocol constants
pub const USERNAME_MAX_BYTES: usize = 24;
pub const PASSWORD_MAX_BYTES: usize = 24;
pub const LAST_LOGIN_TIME_BYTES: usize = 26;
pub const SERVER_NAME_BYTES: usize = 20;
pub const WEB_AUTH_TOKEN_LENGTH: usize = 16; // Token length
pub const SERVER_INFO_SIZE: usize = 160; // 4 + 2 + 20 + 2 + 2 + 2 + 128 (unknown padding)
pub const BLOCK_DATE_BYTES: usize = 20;

// Packet Type Constants
pub const CT_AUTH: u16 = 0x0ACF;
pub const CA_LOGIN: u16 = 0x0064;
pub const AC_ACCEPT_LOGIN: u16 = 0x0AC4;
pub const AC_REFUSE_LOGIN: u16 = 0x006A;

// CA_LOGIN Packet (Client to Server)
#[derive(Debug, Clone)]
pub struct CaLoginPacket {
    pub version: u32,
    pub username: String,
    pub password: String,
    pub client_type: u8,
}

impl CaLoginPacket {
    pub fn new(username: &str, password: &str, version: u32) -> Self {
        Self {
            version,
            username: username.to_string(),
            password: password.to_string(),
            client_type: 0, // Default client type
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(55); // 2 bytes header + 53 bytes payload

        // Packet ID
        bytes.write_u16::<LittleEndian>(CA_LOGIN).unwrap();

        // Version
        bytes.write_u32::<LittleEndian>(self.version).unwrap();

        // Username (24 bytes, null-padded)
        let mut username_bytes = [0u8; USERNAME_MAX_BYTES];
        let username_str = self.username.as_bytes();
        let copy_len = username_str.len().min(USERNAME_MAX_BYTES - 1); // Leave space for null terminator
        username_bytes[..copy_len].copy_from_slice(&username_str[..copy_len]);
        bytes.extend_from_slice(&username_bytes);

        // Password (24 bytes, null-padded)
        let mut password_bytes = [0u8; PASSWORD_MAX_BYTES];
        let password_str = self.password.as_bytes();
        let copy_len = password_str.len().min(PASSWORD_MAX_BYTES - 1);
        password_bytes[..copy_len].copy_from_slice(&password_str[..copy_len]);
        bytes.extend_from_slice(&password_bytes);

        // Client type
        bytes.push(self.client_type);

        bytes
    }
}

// AC_ACCEPT_LOGIN Response (Server to Client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcAcceptLoginPacket {
    pub login_id1: u32,
    pub account_id: u32,
    pub login_id2: u32,
    pub last_login_ip: u32,
    pub last_login_time: [u8; 26],
    pub sex: u8,
    pub server_list: Vec<ServerInfo>,
}

// Server type enum for better type safety
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerType {
    Normal,
    Maintenance,
    PvP,
    PK,
    Special(u16),
}

impl From<u16> for ServerType {
    fn from(value: u16) -> Self {
        match value {
            0 => ServerType::Normal,
            1 => ServerType::Maintenance,
            2 => ServerType::PvP,
            3 => ServerType::PK,
            other => ServerType::Special(other),
        }
    }
}

impl ServerType {
    pub fn as_u16(&self) -> u16 {
        match self {
            ServerType::Normal => 0,
            ServerType::Maintenance => 1,
            ServerType::PvP => 2,
            ServerType::PK => 3,
            ServerType::Special(value) => *value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub ip: u32,
    pub port: u16,
    pub name: String,
    pub users: u16,
    pub server_type: ServerType,
    pub new_server: u16,
}

// AC_REFUSE_LOGIN Response (Server to Client)
#[derive(Debug, Clone)]
pub struct AcRefuseLoginPacket {
    pub error_code: u8,
    pub block_date: [u8; 20],
}

// Packet parsing functions using nom
pub fn parse_ac_accept_login(input: &[u8]) -> IResult<&[u8], AcAcceptLoginPacket> {
    let (input, _packet_id) = le_u16(input)?;
    let (input, packet_length) = le_u16(input)?;

    let (input, login_id1) = nom::number::complete::le_u32(input)?;
    let (input, account_id) = nom::number::complete::le_u32(input)?;
    let (input, login_id2) = nom::number::complete::le_u32(input)?;
    let (input, last_login_ip) = nom::number::complete::le_u32(input)?;
    let (input, last_login_time) = take(LAST_LOGIN_TIME_BYTES)(input)?;
    let (input, sex) = nom::number::complete::le_u8(input)?;

    // Read the web auth token (16 bytes + 1 null terminator)
    let (input, token_bytes) = take(WEB_AUTH_TOKEN_LENGTH + 1)(input)?;

    // Calculate number of servers from packet length
    // Base size: 2 (id) + 2 (len) + 4 (login1) + 4 (account) + 4 (login2) + 4 (ip) + 26 (time) + 1 (sex) + 17 (token) = 64
    // Each server: 4 (ip) + 2 (port) + 20 (name) + 2 (users) + 2 (type) + 2 (new) + 128 (unknown) = 160
    let base_size = 64;
    let remaining_size = packet_length as usize - base_size + 4; // +4 for packet id and length
    let server_count = remaining_size / SERVER_INFO_SIZE;

    let mut server_list = Vec::new();
    let mut current_input = input;

    for _i in 0..server_count {
        // Parse IP as 4 separate bytes (not little-endian u32)
        let (remaining, ip_bytes) = take(4usize)(current_input)?;
        let ip = u32::from_be_bytes([ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]]);

        let (remaining, port) = le_u16(remaining)?;
        let (remaining, name_bytes) = take(SERVER_NAME_BYTES)(remaining)?;
        let (remaining, users) = le_u16(remaining)?;
        let (remaining, server_type_raw) = le_u16(remaining)?;
        let (remaining, new_server) = le_u16(remaining)?;

        // Skip the 128-byte unknown field
        let (remaining, _unknown) = take(128usize)(remaining)?;

        // Parse server name (null-terminated string)
        let name_end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(SERVER_NAME_BYTES);

        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        // Convert raw server type to enum
        let server_type = ServerType::from(server_type_raw);

        server_list.push(ServerInfo {
            ip,
            port,
            name,
            users,
            server_type,
            new_server,
        });

        current_input = remaining;
    }

    Ok((
        current_input,
        AcAcceptLoginPacket {
            login_id1,
            account_id,
            login_id2,
            last_login_ip,
            last_login_time: {
                let mut array = [0u8; LAST_LOGIN_TIME_BYTES];
                array.copy_from_slice(last_login_time);
                array
            },
            sex,
            server_list,
        },
    ))
}

pub fn parse_ac_refuse_login(input: &[u8]) -> IResult<&[u8], AcRefuseLoginPacket> {
    let (input, _packet_id) = le_u16(input)?;
    let (input, error_code) = nom::number::complete::le_u8(input)?;
    let (input, block_date) = take(BLOCK_DATE_BYTES)(input)?;

    Ok((
        input,
        AcRefuseLoginPacket {
            error_code,
            block_date: {
                let mut array = [0u8; BLOCK_DATE_BYTES];
                array.copy_from_slice(block_date);
                array
            },
        },
    ))
}
