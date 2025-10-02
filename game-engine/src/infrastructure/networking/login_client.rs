use super::errors::{NetworkError, NetworkResult};
use super::protocols::ro_login::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

pub struct LoginClient {
    server_address: String,
    connection_timeout: Duration,
    read_timeout: Duration,
}

impl LoginClient {
    pub fn new(server_address: &str) -> Self {
        Self {
            server_address: server_address.to_string(),
            connection_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(15),
        }
    }

    pub async fn attempt_login(
        &self,
        username: &str,
        password: &str,
        client_version: u32,
    ) -> NetworkResult<AcAcceptLoginPacket> {
        // Connect to server with timeout
        let mut stream = timeout(
            self.connection_timeout,
            TcpStream::connect(&self.server_address),
        )
        .await
        .map_err(|_| NetworkError::Timeout)?
        .map_err(NetworkError::from)?;

        // Create and send login packet
        let login_packet = CaLoginPacket::new(username, password, client_version);
        let packet_bytes = login_packet.to_bytes();

        stream
            .write_all(&packet_bytes)
            .await
            .map_err(NetworkError::from)?;

        // Read response with timeout
        let response = timeout(self.read_timeout, self.read_response(&mut stream))
            .await
            .map_err(|_| NetworkError::Timeout)??;

        Ok(response)
    }

    async fn read_response(&self, stream: &mut TcpStream) -> NetworkResult<AcAcceptLoginPacket> {
        // Read packet header (packet ID)
        let mut header_buf = [0u8; 2];
        stream
            .read_exact(&mut header_buf)
            .await
            .map_err(NetworkError::from)?;

        let packet_id = u16::from_le_bytes(header_buf);

        match packet_id {
            AC_ACCEPT_LOGIN => {
                // Read packet length
                let mut length_buf = [0u8; 2];
                stream
                    .read_exact(&mut length_buf)
                    .await
                    .map_err(NetworkError::from)?;

                let packet_length = u16::from_le_bytes(length_buf) as usize;

                // Read remaining packet data
                let mut packet_data = vec![0u8; packet_length];
                packet_data[0..2].copy_from_slice(&header_buf);
                packet_data[2..4].copy_from_slice(&length_buf);

                stream
                    .read_exact(&mut packet_data[4..])
                    .await
                    .map_err(NetworkError::from)?;

                // Parse the packet
                let (_, packet) = parse_ac_accept_login(&packet_data)
                    .map_err(|e| NetworkError::PacketParsingFailed(e.to_string()))?;

                Ok(packet)
            }
            AC_REFUSE_LOGIN => {
                // Read error packet
                let mut error_data = [0u8; 21]; // error_code + block_date
                stream
                    .read_exact(&mut error_data)
                    .await
                    .map_err(NetworkError::from)?;

                let error_code = error_data[0];
                Err(NetworkError::LoginRefused { code: error_code })
            }
            _ => Err(NetworkError::InvalidPacket),
        }
    }
}

// Convenience function for one-shot login attempts
pub async fn attempt_login(
    server_address: &str,
    username: &str,
    password: &str,
    client_version: u32,
) -> NetworkResult<AcAcceptLoginPacket> {
    let client = LoginClient::new(server_address);
    client
        .attempt_login(username, password, client_version)
        .await
}
