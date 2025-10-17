use crate::infrastructure::networking::protocol::traits::ServerPacket;
use nom::{
    bytes::complete::take,
    number::complete::{le_u16, le_u8},
    IResult,
};

pub const AC_REFUSE_LOGIN: u16 = 0x006A;
const BLOCK_DATE_BYTES: usize = 20;

/// AC_REFUSE_LOGIN (0x006A) - Login rejection
///
/// Sent by the login server when authentication fails. Contains an error
/// code indicating the reason for rejection.
///
/// # Packet Structure
/// - Packet ID: u16 (2 bytes)
/// - Error Code: u8 (1 byte)
/// - Block Date: [u8; 20] (null-terminated string)
///
/// Total: 23 bytes
///
/// # Error Codes
/// - 0: Unregistered ID
/// - 1: Incorrect password
/// - 2: Account expired
/// - 3: Rejected from server
/// - 4: Blocked by GM
/// - 5: Not latest game EXE
/// - 6: Banned
/// - 7: Already online
/// - 8: Server full
/// - 9: Company limited
/// - 99: Account has been locked. Please contact customer support.
///
/// # Direction
/// Login Server → Client
#[derive(Debug, Clone)]
pub struct AcRefuseLoginPacket {
    /// Error code indicating rejection reason
    pub error_code: u8,

    /// Block date (if applicable, null-terminated string)
    pub block_date: [u8; 20],
}

impl AcRefuseLoginPacket {
    /// Get a human-readable error message
    pub fn error_message(&self) -> &'static str {
        match self.error_code {
            0 => "Unregistered ID",
            1 => "Incorrect password",
            2 => "Account expired",
            3 => "Rejected from server",
            4 => "Blocked by GM",
            5 => "Not latest game EXE",
            6 => "Banned",
            7 => "Already online",
            8 => "Server full",
            9 => "Company limited",
            99 => "Account locked",
            _ => "Unknown error",
        }
    }

    /// Get block date as string (if available)
    pub fn block_date_string(&self) -> Option<String> {
        let end = self
            .block_date
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(BLOCK_DATE_BYTES);

        if end == 0 {
            None
        } else {
            Some(String::from_utf8_lossy(&self.block_date[..end]).to_string())
        }
    }
}

impl ServerPacket for AcRefuseLoginPacket {
    const PACKET_ID: u16 = AC_REFUSE_LOGIN;

    fn parse(data: &[u8]) -> std::io::Result<Self> {
        parse_ac_refuse_login(data)
            .map(|(_, packet)| packet)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse AC_REFUSE_LOGIN: {}", e),
                )
            })
    }
}

/// Parse AC_REFUSE_LOGIN packet using nom
fn parse_ac_refuse_login(input: &[u8]) -> IResult<&[u8], AcRefuseLoginPacket> {
    let (input, _packet_id) = le_u16(input)?;
    let (input, error_code) = le_u8(input)?;
    let (input, block_date) = take(BLOCK_DATE_BYTES)(input)?;
    let mut block_date_array = [0u8; BLOCK_DATE_BYTES];

    block_date_array.copy_from_slice(block_date);

    Ok((
        input,
        AcRefuseLoginPacket {
            error_code,
            block_date: block_date_array,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ac_refuse_login_packet_id() {
        assert_eq!(AcRefuseLoginPacket::PACKET_ID, 0x006A);
    }

    #[test]
    fn test_error_messages() {
        let packet = AcRefuseLoginPacket {
            error_code: 1,
            block_date: [0; 20],
        };
        assert_eq!(packet.error_message(), "Incorrect password");
    }

    #[test]
    fn test_block_date_empty() {
        let packet = AcRefuseLoginPacket {
            error_code: 1,
            block_date: [0; 20],
        };
        assert_eq!(packet.block_date_string(), None);
    }
}
