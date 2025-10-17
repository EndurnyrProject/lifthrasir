use crate::infrastructure::networking::errors::NetworkError;
use bytes::Bytes;
use std::io;

/// Represents a network protocol (Login, Character, Zone)
///
/// This trait defines the core behavior for a network protocol, including
/// how packets are parsed, sized, and what context is maintained during
/// packet processing.
pub trait Protocol: Send + Sync + 'static {
    /// Unique identifier for this protocol
    const NAME: &'static str;

    /// The client packet type for this protocol (enum of all outgoing packets)
    type ClientPacket: ClientPacket;

    /// The server packet type for this protocol (enum of all incoming packets)
    type ServerPacket: ServerPacket;

    /// Context data passed to handlers (session info, client state, etc.)
    type Context;

    /// Parse a complete packet from bytes
    ///
    /// This method receives the packet ID and the complete packet data
    /// (including the packet ID) and returns the parsed server packet.
    fn parse_server_packet(packet_id: u16, data: &[u8]) -> io::Result<Self::ServerPacket>;

    /// Determine if a packet is fixed or variable length
    ///
    /// This is used by the client to know how much data to read before
    /// attempting to parse a packet.
    fn packet_size(packet_id: u16) -> PacketSize;
}

/// Size information for a packet
#[derive(Debug, Clone, Copy)]
pub enum PacketSize {
    /// Fixed size packet (includes packet ID, in bytes)
    Fixed(usize),
    /// Variable length packet with length field
    Variable {
        /// Offset of the length field from start of packet
        length_offset: usize,
        /// Size of the length field in bytes (2 or 4)
        length_bytes: usize,
    },
}

/// Trait for packets sent from client to server
pub trait ClientPacket: Send + Sync + 'static {
    /// Packet ID constant (0 for enum types, override packet_id() method instead)
    const PACKET_ID: u16;

    /// Serialize this packet to bytes
    fn serialize(&self) -> Bytes;

    /// Get the packet ID (default implementation uses const)
    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

/// Trait for packets received from server
pub trait ServerPacket: Send + Sync + 'static {
    /// Packet ID constant (0 for enum types, override packet_id() method instead)
    const PACKET_ID: u16;

    /// Parse from bytes (without packet ID at start)
    ///
    /// The implementation receives the complete packet data including
    /// the packet ID, so implementations should skip the first 2 bytes.
    fn parse(data: &[u8]) -> io::Result<Self>
    where
        Self: Sized;

    /// Get the packet ID
    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}

/// Handler for a specific server packet
///
/// Handlers are stateless and process packets in the context of a protocol's
/// context and emit Bevy events as needed.
pub trait PacketHandler<P: Protocol>: Send + Sync + 'static {
    /// The packet type this handler processes
    type Packet: ServerPacket;

    /// Handle the packet, emitting Bevy events as needed
    ///
    /// Returns Ok(()) if processing succeeded, Err if a critical error occurred.
    /// Non-critical errors should be logged but return Ok(()).
    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut P::Context,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError>;
}

/// Trait for writing protocol-specific events
///
/// This trait is object-safe and allows handlers to emit events without
/// knowing the concrete event type at compile time. Each protocol defines
/// its own event enum that implements Send.
///
/// The simple approach: handlers receive a reference to the protocol's
/// event buffer, which they can push events into. The events are then
/// sent to Bevy's ECS in batch after packet processing.
pub trait EventWriter: Send {
    /// Type-erased event sending
    ///
    /// This is intentionally minimal to maintain object safety.
    /// Protocols will provide their own type-safe wrapper methods.
    fn send_event(&mut self, event: Box<dyn std::any::Any + Send>);
}

/// Simple event buffer that collects events during packet processing
///
/// This is a basic implementation that stores type-erased events.
/// Each protocol should provide a typed wrapper that works with its
/// specific event enum.
pub struct EventBuffer {
    events: Vec<Box<dyn std::any::Any + Send>>,
}

impl EventBuffer {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push<E: Send + 'static>(&mut self, event: E) {
        self.events.push(Box::new(event));
    }

    pub fn drain(&mut self) -> impl Iterator<Item = Box<dyn std::any::Any + Send>> + '_ {
        self.events.drain(..)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

impl Default for EventBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl EventWriter for EventBuffer {
    fn send_event(&mut self, event: Box<dyn std::any::Any + Send>) {
        self.events.push(event);
    }
}
