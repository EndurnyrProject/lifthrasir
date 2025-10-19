use crate::infrastructure::networking::{
    errors::{NetworkError, NetworkResult},
    protocol::{
        dispatcher::PacketDispatcher,
        traits::{ClientPacket, EventWriter, PacketSize, Protocol},
    },
    transport::TcpTransport,
};
use bevy::prelude::*;
use std::time::Duration;

/// Maximum packet size in RO protocol (65535 bytes)
const MAX_PACKET_SIZE: usize = 65535;
/// Minimum packet size (at least packet ID - 2 bytes)
const MIN_PACKET_SIZE: usize = 2;

/// Generic network client for any protocol
///
/// NetworkClient provides a reusable implementation of network client behavior
/// that works with any protocol implementing the Protocol trait. It handles:
/// - Connection management
/// - Packet sending
/// - Packet receiving and parsing
/// - Handler dispatch
/// - Buffer management
///
/// The client maintains a connection state, internal context, and a dispatcher
/// that routes packets to their handlers.
///
/// # Type Parameters
///
/// * `P` - The protocol implementation (Login, Character, Zone, etc.)
///
/// # Example
///
/// ```ignore
/// let dispatcher = PacketDispatcher::new();
/// // Register handlers...
///
/// let context = LoginContext::new();
/// let mut client = NetworkClient::new(context).with_dispatcher(dispatcher);
///
/// client.connect("127.0.0.1:6900")?;
/// client.send_packet(&login_packet)?;
///
/// // In your update loop:
/// client.update(&mut event_writer)?;
/// ```
pub struct NetworkClient<P: Protocol> {
    transport: Option<TcpTransport>,
    dispatcher: PacketDispatcher<P>,
    context: P::Context,
}

impl<P: Protocol> NetworkClient<P> {
    /// Create a new network client with the given context
    ///
    /// The client starts in a disconnected state. Call `connect()` to
    /// establish a connection.
    pub fn new(context: P::Context) -> Self {
        Self {
            transport: None,
            dispatcher: PacketDispatcher::new(),
            context,
        }
    }

    /// Set the packet dispatcher
    ///
    /// This is a builder-style method that allows you to configure the
    /// dispatcher before using the client.
    pub fn with_dispatcher(mut self, dispatcher: PacketDispatcher<P>) -> Self {
        self.dispatcher = dispatcher;
        self
    }

    /// Connect to a server
    ///
    /// Establishes a TCP connection to the specified address. If already
    /// connected, the existing connection is closed first.
    ///
    /// # Arguments
    ///
    /// * `address` - Server address as "ip:port"
    ///
    /// # Returns
    ///
    /// Ok(()) if connection succeeds, NetworkError otherwise
    pub fn connect(&mut self, address: &str) -> NetworkResult<()> {
        // Disconnect if already connected
        if self.transport.is_some() {
            self.disconnect();
        }

        // Establish new connection
        let transport = TcpTransport::connect(address, Duration::from_secs(30))?;
        self.transport = Some(transport);

        info!("Connected to {} server at {}", P::NAME, address);
        Ok(())
    }

    /// Disconnect from the server
    ///
    /// Closes the TCP connection and clears all buffers. After calling this,
    /// the client is in a disconnected state and can be reconnected with `connect()`.
    pub fn disconnect(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.disconnect();
            info!("Disconnected from {} server", P::NAME);
        }
    }

    /// Check if the client is currently connected
    pub fn is_connected(&self) -> bool {
        self.transport.as_ref().is_some_and(|t| t.is_connected())
    }

    /// Send a packet to the server
    ///
    /// Serializes the packet and sends it over the TCP connection.
    ///
    /// # Arguments
    ///
    /// * `packet` - The client packet to send
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    ///
    /// # Errors
    ///
    /// Returns UnexpectedDisconnect if not connected
    pub fn send_packet(&mut self, packet: &P::ClientPacket) -> NetworkResult<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or(NetworkError::UnexpectedDisconnect)?;

        let bytes = packet.serialize();
        transport.send(&bytes)?;

        debug!(
            "{} client sent packet 0x{:04X} ({} bytes)",
            P::NAME,
            packet.packet_id(),
            bytes.len()
        );
        Ok(())
    }

    /// Receive data from the socket without processing packets
    ///
    /// This is useful for protocol-specific pre-processing that needs to
    /// examine the buffer before standard packet processing.
    ///
    /// # Returns
    ///
    /// Ok(()) if receive succeeded, NetworkError otherwise
    pub fn receive_data(&mut self) -> NetworkResult<()> {
        let transport = match self.transport.as_mut() {
            Some(t) => t,
            None => return Ok(()), // Not connected, nothing to do
        };

        // Read available data
        match transport.receive() {
            Ok(0) => {} // No data available
            Ok(n) => {
                trace!("{} client received {} bytes", P::NAME, n);
            }
            Err(e) => return Err(e),
        }

        Ok(())
    }

    /// Process incoming packets
    ///
    /// This should be called regularly (e.g., in a Bevy system) to:
    /// 1. Read available data from the socket
    /// 2. Parse complete packets from the buffer
    /// 3. Dispatch packets to handlers
    /// 4. Emit Bevy events
    ///
    /// # Arguments
    ///
    /// * `event_writer` - Event writer for emitting Bevy events
    ///
    /// # Returns
    ///
    /// Ok(()) if processing succeeded, NetworkError for critical errors
    ///
    /// # Note
    ///
    /// Non-critical errors (like malformed packets) are logged but don't
    /// return an error. This allows the client to continue processing.
    pub fn update(&mut self, event_writer: &mut dyn EventWriter) -> NetworkResult<()> {
        // Receive new data
        self.receive_data()?;

        let transport = match self.transport.as_mut() {
            Some(t) => t,
            None => return Ok(()), // Not connected, nothing to do
        };

        // Process all complete packets in buffer
        loop {
            let buffer = transport.buffer();

            // Need at least packet ID (2 bytes)
            if buffer.len() < 2 {
                break;
            }

            let packet_id = u16::from_le_bytes([buffer[0], buffer[1]]);

            // Determine packet size
            let packet_size = match P::packet_size(packet_id) {
                PacketSize::Fixed(size) => {
                    if buffer.len() < size {
                        break; // Wait for more data
                    }
                    size
                }
                PacketSize::Variable {
                    length_offset,
                    length_bytes,
                } => {
                    // Check if we have the length field
                    if buffer.len() < length_offset + length_bytes {
                        break; // Wait for length field
                    }

                    // Read length field
                    let length = match length_bytes {
                        2 => u16::from_le_bytes([buffer[length_offset], buffer[length_offset + 1]])
                            as usize,
                        4 => u32::from_le_bytes([
                            buffer[length_offset],
                            buffer[length_offset + 1],
                            buffer[length_offset + 2],
                            buffer[length_offset + 3],
                        ]) as usize,
                        _ => {
                            error!(
                                "{} protocol: Invalid length field size: {}",
                                P::NAME,
                                length_bytes
                            );
                            return Err(NetworkError::InvalidPacket);
                        }
                    };

                    // Validate length is reasonable
                    if !(MIN_PACKET_SIZE..=MAX_PACKET_SIZE).contains(&length) {
                        error!(
                            "{} protocol: Invalid packet length {} for packet 0x{:04X}. Disconnecting.",
                            P::NAME, length, packet_id
                        );
                        self.disconnect();
                        return Err(NetworkError::InvalidPacketLength {
                            id: packet_id,
                            length,
                        });
                    }

                    // Check if we have the complete packet
                    if buffer.len() < length {
                        break; // Wait for complete packet
                    }

                    length
                }
            };

            // CRITICAL: Copy packet data BEFORE consuming
            let packet_data = buffer[..packet_size].to_vec();

            // CRITICAL: Consume immediately (buffer now safe)
            transport.consume(packet_size);

            // Check if we have a handler for this packet
            if !self.dispatcher.has_handler(packet_id) {
                warn!(
                    "{} client: Skipping unknown packet 0x{:04X} ({} bytes)",
                    P::NAME,
                    packet_id,
                    packet_size
                );
                // Buffer already consumed, just continue
                continue;
            }

            // Dispatch packet to handler (buffer already safe)
            match self
                .dispatcher
                .dispatch(packet_id, &packet_data, &mut self.context, event_writer)
            {
                Ok(()) => {
                    debug!(
                        "{} client processed packet 0x{:04X} ({} bytes)",
                        P::NAME,
                        packet_id,
                        packet_size
                    );
                }
                Err(e) => {
                    warn!(
                        "{} client: Failed to process packet 0x{:04X}: {:?}. Skipping.",
                        P::NAME,
                        packet_id,
                        e
                    );
                    // Buffer already consumed, just continue
                }
            }
        }

        Ok(())
    }

    /// Get a reference to the protocol context
    pub fn context(&self) -> &P::Context {
        &self.context
    }

    /// Get a mutable reference to the protocol context
    pub fn context_mut(&mut self) -> &mut P::Context {
        &mut self.context
    }

    /// Get the current buffer size (for debugging/monitoring)
    pub fn buffer_size(&self) -> usize {
        self.transport.as_ref().map_or(0, |t| t.buffer_len())
    }

    /// Get the number of registered packet handlers
    pub fn handler_count(&self) -> usize {
        self.dispatcher.handler_count()
    }

    /// Peek at the buffer without consuming
    ///
    /// Returns a reference to the current buffer contents.
    /// Useful for protocol-specific pre-processing before standard packet handling.
    ///
    /// # Returns
    ///
    /// Some(&[u8]) if connected and has data, None otherwise
    pub fn peek_buffer(&self) -> Option<&[u8]> {
        self.transport.as_ref().map(|t| t.buffer())
    }

    /// Consume bytes from the buffer
    ///
    /// Removes the specified number of bytes from the front of the buffer.
    /// Useful for protocol-specific pre-processing (e.g., acknowledgments).
    ///
    /// # Arguments
    ///
    /// * `count` - Number of bytes to consume
    ///
    /// # Returns
    ///
    /// Ok(()) if bytes consumed, NetworkError if not connected
    pub fn consume_bytes(&mut self, count: usize) -> NetworkResult<()> {
        let transport = self
            .transport
            .as_mut()
            .ok_or(NetworkError::UnexpectedDisconnect)?;
        transport.consume(count);
        Ok(())
    }
}
