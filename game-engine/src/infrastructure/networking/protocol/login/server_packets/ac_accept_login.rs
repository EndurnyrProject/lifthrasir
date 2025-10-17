use crate::infrastructure::networking::protocol::{login::types::ServerInfo, traits::ServerPacket};
use nom::{
    bytes::complete::take,
    number::complete::{le_u16, le_u32, le_u8},
    IResult,
};
use serde::{Deserialize, Serialize};

pub const AC_ACCEPT_LOGIN: u16 = 0x0AC4;
const LAST_LOGIN_TIME_BYTES: usize = 26;
const SERVER_NAME_BYTES: usize = 20;
const WEB_AUTH_TOKEN_LENGTH: usize = 16;
const SERVER_INFO_SIZE: usize = 160; // 4 + 2 + 20 + 2 + 2 + 2 + 128

/// AC_ACCEPT_LOGIN (0x0AC4) - Login acceptance with server list
///
/// Sent by the login server when authentication succeeds. Contains session
/// tokens and a list of available game servers.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Packet Length: u16 (2 bytes)
/// - Login ID 1: u32 (4 bytes)
/// - Account ID: u32 (4 bytes)
/// - Login ID 2: u32 (4 bytes)
/// - Last Login IP: u32 (4 bytes)
/// - Last Login Time: [u8; 26]
/// - Sex: u8 (1 byte)
/// - Web Auth Token: [u8; 17] (16 + null terminator)
/// - Server List: Vec<ServerInfo> (variable length)
///
/// # Direction
/// Login Server → Client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcAcceptLoginPacket {
    /// Session token 1 (used for character server authentication)
    pub login_id1: u32,

    /// Account ID
    pub account_id: u32,

    /// Session token 2 (used for character server authentication)
    pub login_id2: u32,

    /// Last login IP address (network byte order)
    pub last_login_ip: u32,

    /// Last login timestamp (null-terminated string)
    pub last_login_time: [u8; 26],

    /// Character gender (0 = female, 1 = male)
    pub sex: u8,

    /// List of available game servers
    pub server_list: Vec<ServerInfo>,
}

impl ServerPacket for AcAcceptLoginPacket {
    const PACKET_ID: u16 = AC_ACCEPT_LOGIN;

    fn parse(data: &[u8]) -> std::io::Result<Self> {
        parse_ac_accept_login(data)
            .map(|(_, packet)| packet)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse AC_ACCEPT_LOGIN: {}", e),
                )
            })
    }
}

/// Parse AC_ACCEPT_LOGIN packet using nom
fn parse_ac_accept_login(input: &[u8]) -> IResult<&[u8], AcAcceptLoginPacket> {
    // Skip packet ID (already read by dispatcher)
    let (input, _packet_id) = le_u16(input)?;

    // Read packet length
    let (input, packet_length) = le_u16(input)?;

    // Read session tokens
    let (input, login_id1) = le_u32(input)?;
    let (input, account_id) = le_u32(input)?;
    let (input, login_id2) = le_u32(input)?;

    // Read last login info
    let (input, last_login_ip) = le_u32(input)?;
    let (input, last_login_time) = take(LAST_LOGIN_TIME_BYTES)(input)?;
    let (input, sex) = le_u8(input)?;

    // Read web auth token (16 bytes + 1 null terminator)
    let (input, _token_bytes) = take(WEB_AUTH_TOKEN_LENGTH + 1)(input)?;

    // Calculate number of servers from packet length
    // Base size: 2 (id) + 2 (len) + 4 (login1) + 4 (account) + 4 (login2) +
    //            4 (ip) + 26 (time) + 1 (sex) + 17 (token) = 64
    // Each server: 160 bytes
    let base_size = 64;
    let remaining_size = packet_length as usize - base_size + 4; // +4 for packet id and length
    let server_count = remaining_size / SERVER_INFO_SIZE;

    // Parse server list
    let mut server_list = Vec::with_capacity(server_count);
    let mut current_input = input;

    for _ in 0..server_count {
        // Parse IP as 4 separate bytes (network byte order)
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
        let server_type = server_type_raw.into();

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

    // Convert last_login_time to array
    let mut last_login_time_array = [0u8; LAST_LOGIN_TIME_BYTES];
    last_login_time_array.copy_from_slice(last_login_time);

    Ok((
        current_input,
        AcAcceptLoginPacket {
            login_id1,
            account_id,
            login_id2,
            last_login_ip,
            last_login_time: last_login_time_array,
            sex,
            server_list,
        },
    ))
}
