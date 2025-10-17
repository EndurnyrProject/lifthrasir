use crate::infrastructure::networking::errors::{NetworkError, NetworkResult};
use bevy::prelude::*;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Maximum buffer size (1 MB) to prevent memory exhaustion
const MAX_BUFFER_SIZE: usize = 1024 * 1024;

/// TCP connection wrapper with buffer management
///
/// TcpTransport handles the low-level TCP connection and provides buffering
/// for incoming data. It manages:
/// - Connection establishment and teardown
/// - Non-blocking I/O
/// - Internal read buffer for partial packets
/// - Sending complete packets
///
/// The transport layer is protocol-agnostic and simply moves bytes between
/// the network and the client's buffer.
pub struct TcpTransport {
    stream: TcpStream,
    read_buffer: Vec<u8>,
}

impl TcpTransport {
    /// Connect to a server at the given address
    ///
    /// The connection is attempted with the specified timeout, then switched
    /// to non-blocking mode for ongoing operations.
    ///
    /// # Arguments
    ///
    /// * `address` - Server address as "ip:port" string
    /// * `connection_timeout` - Maximum time to wait for connection
    ///
    /// # Returns
    ///
    /// A connected TcpTransport or NetworkError
    pub fn connect(address: &str, connection_timeout: Duration) -> NetworkResult<Self> {
        let socket_addr: SocketAddr = address
            .parse()
            .map_err(|_| NetworkError::ConnectionFailed(format!("Invalid address: {}", address)))?;
        let stream = TcpStream::connect_timeout(&socket_addr, connection_timeout)?;

        stream.set_nonblocking(true)?;

        stream
            .set_nodelay(true)
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            stream,
            read_buffer: Vec::with_capacity(4096),
        })
    }

    /// Send data to the server
    ///
    /// This is a blocking call that ensures all data is sent. It should be
    /// called with complete, serialized packets.
    ///
    /// # Arguments
    ///
    /// * `data` - Complete packet bytes to send
    ///
    /// # Returns
    ///
    /// Ok(()) if all data was sent, NetworkError otherwise
    pub fn send(&mut self, data: &[u8]) -> NetworkResult<()> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Read available data into internal buffer
    ///
    /// This is a non-blocking call that reads whatever data is available from
    /// the socket and appends it to the internal buffer. If no data is available,
    /// it returns Ok(0) without blocking.
    ///
    /// # Returns
    ///
    /// Number of bytes read, or NetworkError
    /// - Returns 0 if no data available (WOULD_BLOCK)
    /// - Returns NetworkError::UnexpectedDisconnect if connection closed
    pub fn receive(&mut self) -> NetworkResult<usize> {
        let mut temp_buffer = [0u8; 4096];

        match self.stream.read(&mut temp_buffer) {
            Ok(0) => {
                // Connection closed by server
                Err(NetworkError::UnexpectedDisconnect)
            }
            Ok(n) => {
                // Check if the buffer would exceed the limit
                if self.read_buffer.len() + n > MAX_BUFFER_SIZE {
                    error!(
                        "Read buffer size limit exceeded: {} + {} > {}. Disconnecting.",
                        self.read_buffer.len(),
                        n,
                        MAX_BUFFER_SIZE
                    );
                    return Err(NetworkError::ConnectionFailed(
                        "Buffer size limit exceeded".to_string(),
                    ));
                }
                // Append received data to buffer
                self.read_buffer.extend_from_slice(&temp_buffer[..n]);
                Ok(n)
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available right now, not an error
                Ok(0)
            }
            Err(e) => {
                // Actual I/O error
                Err(NetworkError::from(e))
            }
        }
    }

    /// Get reference to the internal read buffer
    ///
    /// This allows the client to inspect buffered data without consuming it.
    pub fn buffer(&self) -> &[u8] {
        &self.read_buffer
    }

    /// Get the number of bytes currently in the buffer
    pub fn buffer_len(&self) -> usize {
        self.read_buffer.len()
    }

    /// Consume N bytes from the buffer
    ///
    /// This should be called after successfully parsing a packet to remove
    /// the processed bytes from the buffer.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of bytes to remove from the front of the buffer
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the buffer length
    pub fn consume(&mut self, n: usize) {
        assert!(
            n <= self.read_buffer.len(),
            "Attempted to consume {} bytes from buffer of length {}",
            n,
            self.read_buffer.len()
        );
        self.read_buffer.drain(..n);
    }

    /// Clear the entire buffer
    ///
    /// This is useful for error recovery scenarios where the buffer may
    /// contain corrupted data.
    pub fn clear_buffer(&mut self) {
        self.read_buffer.clear();
    }

    /// Disconnect from the server
    ///
    /// Shuts down the TCP connection and clears the buffer. After calling
    /// this, the transport can no longer be used.
    pub fn disconnect(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        self.read_buffer.clear();
    }

    /// Check if the connection is still alive
    ///
    /// This attempts to peek at the socket to see if it's still readable.
    /// Note: This is a best-effort check and may not catch all disconnect scenarios.
    pub fn is_connected(&self) -> bool {
        // Try to peek without actually reading
        let mut buf = [0u8; 1];
        match self.stream.peek(&mut buf) {
            Ok(_) => true,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    /// Get the peer address
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        // Ensure clean shutdown
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}
