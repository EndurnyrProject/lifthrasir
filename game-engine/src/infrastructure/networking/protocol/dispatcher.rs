use super::traits::{EventWriter, PacketHandler, Protocol, ServerPacket};
use crate::infrastructure::networking::errors::NetworkError;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Type-erased packet handler trait object
trait DynPacketHandler<P: Protocol>: Send + Sync {
    /// Handle a packet using dynamic dispatch
    fn handle_dyn(
        &self,
        packet_id: u16,
        data: &[u8],
        context: &mut P::Context,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError>;
}

/// Wrapper to convert a typed PacketHandler into a trait object
struct HandlerWrapper<P, H>
where
    P: Protocol,
    H: PacketHandler<P>,
{
    handler: H,
    _phantom: std::marker::PhantomData<P>,
}

impl<P, H> DynPacketHandler<P> for HandlerWrapper<P, H>
where
    P: Protocol,
    H: PacketHandler<P>,
{
    fn handle_dyn(
        &self,
        packet_id: u16,
        data: &[u8],
        context: &mut P::Context,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        // Parse the specific packet type
        // We panic on parse failures to prevent buffer corruption from propagating
        let packet = H::Packet::parse(data).unwrap_or_else(|e| {
            panic!(
                "CRITICAL: Failed to parse packet 0x{:04X} ({} bytes): {}. \
                 This indicates a protocol mismatch or corrupted buffer. \
                 Data (first 32 bytes): {:02X?}",
                packet_id,
                data.len(),
                e,
                &data[..data.len().min(32)]
            );
        });

        // Handle the parsed packet
        self.handler.handle(packet, context, event_writer)
    }
}

/// Registry and dispatcher for packet handlers
///
/// The PacketDispatcher maintains a registry of packet handlers and routes
/// incoming packets to the appropriate handler based on packet ID. This
/// replaces giant match statements with a type-safe, extensible system.
///
/// # Example
///
/// ```ignore
/// let mut dispatcher = PacketDispatcher::<LoginProtocol>::new();
/// dispatcher.register(AcceptLoginHandler);
/// dispatcher.register(RefuseLoginHandler);
///
/// // Later, when processing packets:
/// dispatcher.dispatch(packet, &mut context, &mut event_writer)?;
/// ```
pub struct PacketDispatcher<P: Protocol> {
    handlers: HashMap<u16, Arc<dyn DynPacketHandler<P>>>,
}

impl<P: Protocol> PacketDispatcher<P> {
    /// Create a new empty dispatcher
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for a specific packet type
    ///
    /// The packet ID is automatically extracted from the handler's associated
    /// Packet type. If a handler for this packet ID is already registered,
    /// it will be replaced with a warning logged.
    pub fn register<H>(&mut self, handler: H)
    where
        H: PacketHandler<P>,
    {
        let packet_id = H::Packet::PACKET_ID;

        // Check if we're replacing an existing handler
        if self.handlers.contains_key(&packet_id) {
            warn!(
                "Replacing existing handler for packet 0x{:04X} in {} protocol",
                packet_id,
                P::NAME
            );
        }

        let wrapper = HandlerWrapper {
            handler,
            _phantom: std::marker::PhantomData,
        };

        self.handlers.insert(packet_id, Arc::new(wrapper));

        debug!(
            "Registered handler for packet 0x{:04X} in {} protocol",
            packet_id,
            P::NAME
        );
    }

    /// Dispatch a packet to its registered handler
    pub fn dispatch(
        &self,
        packet_id: u16,
        data: &[u8],
        context: &mut P::Context,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        if let Some(handler) = self.handlers.get(&packet_id) {
            handler.handle_dyn(packet_id, data, context, event_writer)
        } else {
            panic!(
                "No handler registered for packet 0x{:04X} in {} protocol",
                packet_id,
                P::NAME
            )
        }
    }

    /// Get the number of registered handlers
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Check if a handler is registered for a specific packet ID
    pub fn has_handler(&self, packet_id: u16) -> bool {
        self.handlers.contains_key(&packet_id)
    }

    /// Get all registered packet IDs
    pub fn registered_packets(&self) -> Vec<u16> {
        self.handlers.keys().copied().collect()
    }
}

impl<P: Protocol> Default for PacketDispatcher<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    // Mock protocol for testing
    struct TestProtocol;

    impl Protocol for TestProtocol {
        const NAME: &'static str = "Test";
        type ClientPacket = TestClientPacket;
        type ServerPacket = TestServerPacket;
        type Context = ();

        fn parse_server_packet(
            _packet_id: u16,
            _data: &[u8],
        ) -> std::io::Result<Self::ServerPacket> {
            Ok(TestServerPacket)
        }

        fn packet_size(_packet_id: u16) -> super::super::traits::PacketSize {
            super::super::traits::PacketSize::Fixed(10)
        }
    }

    struct TestClientPacket;
    impl super::super::traits::ClientPacket for TestClientPacket {
        const PACKET_ID: u16 = 0x0001;
        fn serialize(&self) -> Bytes {
            Bytes::new()
        }
    }

    struct TestServerPacket;
    impl ServerPacket for TestServerPacket {
        const PACKET_ID: u16 = 0x0002;
        fn parse(_data: &[u8]) -> std::io::Result<Self> {
            Ok(TestServerPacket)
        }
    }

    struct TestHandler;
    impl PacketHandler<TestProtocol> for TestHandler {
        type Packet = TestServerPacket;

        fn handle(
            &self,
            _packet: Self::Packet,
            _context: &mut (),
            _event_writer: &mut dyn EventWriter,
        ) -> Result<(), NetworkError> {
            Ok(())
        }
    }

    #[test]
    fn test_dispatcher_register() {
        let mut dispatcher = PacketDispatcher::<TestProtocol>::new();
        assert_eq!(dispatcher.handler_count(), 0);

        dispatcher.register(TestHandler);
        assert_eq!(dispatcher.handler_count(), 1);
        assert!(dispatcher.has_handler(0x0002));
    }

    #[test]
    #[should_panic(expected = "No handler registered for packet 0x9999")]
    fn test_dispatcher_dispatch_unknown_packet() {
        let dispatcher = PacketDispatcher::<TestProtocol>::new();
        let mut context = ();
        let mut event_writer = super::super::traits::EventBuffer::new();

        // Should panic on unknown packet to prevent buffer corruption
        dispatcher
            .dispatch(0x9999, &[], &mut context, &mut event_writer)
            .unwrap();
    }
}
