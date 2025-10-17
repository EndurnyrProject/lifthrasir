use crate::infrastructure::networking::{
    client::NetworkClient,
    errors::NetworkResult,
    protocol::{
        dispatcher::PacketDispatcher,
        zone::{
            AcceptEnterHandler, AccountIdReceived, AidHandler, CzEnter2Packet,
            CzNotifyActorinitPacket, RefuseEnterHandler, SpawnData, ZoneClientPacket, ZoneContext,
            ZoneEntryRefused, ZoneProtocol, ZoneServerConnected,
        },
        EventBuffer,
    },
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// High-level client for the Zone Protocol
///
/// ZoneServerClient provides a convenient wrapper around NetworkClient<ZoneProtocol>
/// with a clean API for Bevy systems. It handles:
/// - Connection management
/// - Zone server entry
/// - Spawn data reception
/// - Account ID acknowledgment
///
/// # Example
///
/// ```ignore
/// // In your Bevy app setup:
/// let context = ZoneContext::with_session(account_id, char_id);
/// app.insert_resource(ZoneServerClient::new(context));
///
/// // In a system:
/// fn zone_enter_system(
///     mut client: ResMut<ZoneServerClient>,
///     zone_info: Res<ZoneServerInfo>,
/// ) {
///     client.connect(&zone_info.address).unwrap();
///     client.enter_world(
///         account_id,
///         char_id,
///         &zone_info.auth_code
///     ).unwrap();
/// }
///
/// // In your update system:
/// fn update_system(
///     mut client: ResMut<ZoneServerClient>,
///     mut connected: EventWriter<ZoneServerConnected>,
///     mut aid_received: EventWriter<AccountIdReceived>,
///     mut entry_refused: EventWriter<ZoneEntryRefused>,
/// ) {
///     let mut event_buffer = EventBuffer::new();
///     client.update(&mut event_buffer).unwrap();
///     event_buffer.flush(&mut connected, &mut aid_received, &mut entry_refused);
/// }
/// ```
#[derive(Resource)]
pub struct ZoneServerClient {
    inner: NetworkClient<ZoneProtocol>,
}

impl ZoneServerClient {
    /// Create a new zone server client with session data
    ///
    /// The client is initialized with:
    /// - Session data (account_id, character_id)
    /// - Registered packet handlers for all zone server packets
    ///
    /// # Arguments
    ///
    /// * `context` - Zone context with session data
    pub fn new(context: ZoneContext) -> Self {
        let mut dispatcher = PacketDispatcher::new();

        // Register handlers for all zone server packets
        dispatcher.register(AcceptEnterHandler);
        dispatcher.register(AidHandler);
        dispatcher.register(RefuseEnterHandler);

        let client = NetworkClient::new(context).with_dispatcher(dispatcher);

        Self { inner: client }
    }

    /// Create a client with session data from character selection
    ///
    /// Convenience method that creates a ZoneContext with the session data.
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID
    /// * `character_id` - Character ID for the selected character
    pub fn with_session(account_id: u32, character_id: u32) -> Self {
        let context = ZoneContext::with_session(account_id, character_id);
        Self::new(context)
    }

    /// Connect to a zone server
    ///
    /// # Arguments
    ///
    /// * `address` - Server address as "ip:port" (e.g., "127.0.0.1:5121")
    ///
    /// # Returns
    ///
    /// Ok(()) if connection succeeds, NetworkError otherwise
    pub fn connect(&mut self, address: &str) -> NetworkResult<()> {
        self.inner.connect(address)
    }

    /// Enter the game world
    ///
    /// Sends a CZ_ENTER2 packet with the session credentials and authentication code.
    /// The server will respond with ZC_ACCEPT_ENTER or ZC_REFUSE_ENTER.
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID
    /// * `char_id` - Character ID
    /// * `auth_code` - Authentication code from character server (login_id1)
    /// * `client_time` - Client timestamp (can be 0 or current tick)
    /// * `sex` - Character sex (0 = female, 1 = male)
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn enter_world(
        &mut self,
        account_id: u32,
        char_id: u32,
        auth_code: u32,
        client_time: u32,
        sex: u8,
    ) -> NetworkResult<()> {
        let packet = ZoneClientPacket::CzEnter2(CzEnter2Packet::new(
            account_id,
            char_id,
            auth_code,
            client_time,
            sex,
        ));
        self.inner.send_packet(&packet)
    }

    /// Notify the server that the client is ready
    ///
    /// Sends a CZ_NOTIFY_ACTORINIT packet to indicate that the client
    /// has completed initialization and is ready to receive game data.
    ///
    /// This should be sent after receiving ZC_ACCEPT_ENTER and ZC_AID.
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn notify_ready(&mut self) -> NetworkResult<()> {
        let packet = ZoneClientPacket::CzNotifyActorinit(CzNotifyActorinitPacket::new());
        self.inner.send_packet(&packet)
    }

    /// Process incoming packets and emit Bevy events
    ///
    /// This should be called regularly (e.g., in a Bevy Update system) to:
    /// 1. Read data from the socket
    /// 2. Parse complete packets
    /// 3. Dispatch to handlers
    /// 4. Populate the event buffer
    ///
    /// After calling this, extract events from the buffer and send them to Bevy.
    ///
    /// # Arguments
    ///
    /// * `event_buffer` - Buffer to collect events
    ///
    /// # Returns
    ///
    /// Ok(()) if processing succeeded, NetworkError for critical errors
    pub fn update(&mut self, event_buffer: &mut EventBuffer) -> NetworkResult<()> {
        self.inner.update(event_buffer)
    }

    /// Disconnect from the zone server
    ///
    /// Closes the TCP connection and clears all buffers.
    pub fn disconnect(&mut self) {
        self.inner.disconnect();
    }

    /// Check if currently connected to the zone server
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// Check if fully connected and ready to play
    ///
    /// Returns true if both spawn data and account ID have been received.
    pub fn is_ready(&self) -> bool {
        self.inner.context().is_ready()
    }

    /// Get spawn data (available after entering the world)
    pub fn spawn_data(&self) -> Option<&SpawnData> {
        self.inner.context().spawn_data.as_ref()
    }

    /// Get the current server tick
    pub fn server_tick(&self) -> u32 {
        self.inner.context().server_tick
    }

    /// Check if the AID acknowledgment was received
    pub fn received_aid(&self) -> bool {
        self.inner.context().received_aid
    }

    /// Check if we've entered the world
    pub fn entered_world(&self) -> bool {
        self.inner.context().entered_world
    }

    /// Reset the context for a new connection
    pub fn reset_context(&mut self) {
        self.inner.context_mut().reset();
    }
}

/// SystemParam that bundles all ZoneServer event writers
///
/// This reduces the parameter count of zone_server_update_system from 4 to 2
/// by grouping all related EventWriters into a single logical parameter.
#[derive(SystemParam)]
pub struct ZoneServerEventWriters<'w> {
    pub connected: EventWriter<'w, ZoneServerConnected>,
    pub aid_received: EventWriter<'w, AccountIdReceived>,
    pub entry_refused: EventWriter<'w, ZoneEntryRefused>,
}

/// Bevy system to update the zone server client
///
/// This system should run in the Update schedule. It processes incoming
/// packets and emits Bevy events for the game to handle.
///
/// # Requirements
///
/// - ZoneServerClient must be inserted as a Resource
/// - All zone server events must be registered
///
/// # Example
///
/// ```ignore
/// app.insert_resource(ZoneServerClient::with_session(account_id, char_id))
///    .add_event::<ZoneServerConnected>()
///    .add_event::<AccountIdReceived>()
///    .add_event::<ZoneEntryRefused>()
///    .add_systems(Update, zone_server_update_system);
/// ```
pub fn zone_server_update_system(
    client: Option<ResMut<ZoneServerClient>>,
    mut events: ZoneServerEventWriters,
) {
    let Some(mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    let mut event_buffer = EventBuffer::new();

    if let Err(e) = client.update(&mut event_buffer) {
        error!("Zone server client error: {:?}", e);
        return;
    }

    // Dispatch type-erased events to their corresponding EventWriters
    crate::dispatch_events!(
        event_buffer,
        events,
        [
            (ZoneServerConnected, connected),
            (AccountIdReceived, aid_received),
            (ZoneEntryRefused, entry_refused),
        ]
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_server_client_creation() {
        let client = ZoneServerClient::with_session(12345, 67890);
        assert!(!client.is_connected());
        assert!(!client.is_ready());
        assert!(!client.entered_world());
        assert!(!client.received_aid());
    }

    #[test]
    fn test_zone_server_client_with_context() {
        let context = ZoneContext::with_session(12345, 67890);
        let client = ZoneServerClient::new(context);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_spawn_data_access() {
        let client = ZoneServerClient::with_session(12345, 67890);
        assert!(client.spawn_data().is_none());
        assert_eq!(client.server_tick(), 0);
    }
}
