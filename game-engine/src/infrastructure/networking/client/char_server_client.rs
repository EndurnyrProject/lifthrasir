use crate::infrastructure::networking::{
    client::NetworkClient,
    errors::{NetworkError, NetworkResult},
    protocol::{
        character::{
            AcceptDeletecharHandler, AcceptEnterHandler, AcceptMakecharHandler,
            AckCharinfoPerPageHandler, BlockCharacterHandler, BlockedCharactersReceived,
            ChDeleteCharPacket, ChEnterPacket, ChMakeCharPacket, ChPingPacket, ChSelectCharPacket,
            CharacterClientPacket, CharacterContext, CharacterCreated, CharacterCreationFailed,
            CharacterDeleted, CharacterDeletionFailed, CharacterInfo, CharacterInfoPageReceived,
            CharacterListHandler, CharacterProtocol, CharacterServerConnected,
            CharacterSlotInfoReceived, NotifyZonesvrHandler, PingHandler, PingReceived,
            RefuseDeletecharHandler, RefuseMakecharHandler, SecondPasswdLoginHandler,
            SecondPasswordRequested, ZoneServerInfo, ZoneServerInfoReceived,
        },
        dispatcher::PacketDispatcher,
        EventBuffer,
    },
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// High-level client for the Character Protocol
///
/// CharServerClient provides a convenient wrapper around NetworkClient<CharacterProtocol>
/// with a clean API for Bevy systems. It handles:
/// - Connection management
/// - Character listing and selection
/// - Character creation and deletion
/// - Zone server redirection
/// - Keep-alive pings
///
/// # Example
///
/// ```ignore
/// // In your Bevy app setup:
/// let context = CharacterContext::with_session(account_id, login_id1, login_id2);
/// app.insert_resource(CharServerClient::new(context));
///
/// // In a system:
/// fn char_select_system(mut client: ResMut<CharServerClient>) {
///     client.connect("127.0.0.1:6121").unwrap();
///     client.enter_server().unwrap();
/// }
///
/// // In your update system:
/// fn update_system(
///     mut client: ResMut<CharServerClient>,
///     mut connected: EventWriter<CharacterServerConnected>,
///     mut zone_info: EventWriter<ZoneServerInfoReceived>,
///     // ... other event writers
/// ) {
///     let mut event_buffer = EventBuffer::new();
///     client.update(&mut event_buffer).unwrap();
///     event_buffer.flush(&mut connected, &mut zone_info, /* ... */);
/// }
/// ```
#[derive(Resource)]
pub struct CharServerClient {
    inner: NetworkClient<CharacterProtocol>,
}

impl CharServerClient {
    /// Create a new character server client with session data
    ///
    /// The client is initialized with:
    /// - Session data from login server (account_id, login_id1, login_id2)
    /// - Registered packet handlers for all character server packets
    ///
    /// # Arguments
    ///
    /// * `context` - Character context with session data
    pub fn new(context: CharacterContext) -> Self {
        let mut dispatcher = PacketDispatcher::new();

        // Register handlers for all character server packets
        dispatcher.register(AcceptEnterHandler);
        dispatcher.register(NotifyZonesvrHandler);
        dispatcher.register(CharacterListHandler);
        dispatcher.register(AcceptMakecharHandler);
        dispatcher.register(RefuseMakecharHandler);
        dispatcher.register(AcceptDeletecharHandler);
        dispatcher.register(RefuseDeletecharHandler);
        dispatcher.register(PingHandler);
        dispatcher.register(BlockCharacterHandler);
        dispatcher.register(SecondPasswdLoginHandler);
        dispatcher.register(AckCharinfoPerPageHandler);

        let client = NetworkClient::new(context).with_dispatcher(dispatcher);

        Self { inner: client }
    }

    /// Create a client with session data from login response
    ///
    /// Convenience method that creates a CharacterContext with the session data.
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account ID from login
    /// * `login_id1` - First login ID from login
    /// * `login_id2` - Second login ID from login
    /// * `sex` - Account sex (0 = female, 1 = male)
    pub fn with_session(account_id: u32, login_id1: u32, login_id2: u32, sex: u8) -> Self {
        let context = CharacterContext::with_session(account_id, login_id1, login_id2, sex);
        Self::new(context)
    }

    /// Connect to a character server
    ///
    /// # Arguments
    ///
    /// * `address` - Server address as "ip:port" (e.g., "127.0.0.1:6121")
    ///
    /// # Returns
    ///
    /// Ok(()) if connection succeeds, NetworkError otherwise
    pub fn connect(&mut self, address: &str) -> NetworkResult<()> {
        self.inner.connect(address)
    }

    /// Enter the character server
    ///
    /// Sends a CH_ENTER packet with the session credentials.
    /// The server will respond with HC_ACCEPT_ENTER or HC_REFUSE_ENTER.
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    ///
    /// # Errors
    ///
    /// Returns error if not connected or if session data is missing.
    pub fn enter_server(&mut self) -> NetworkResult<()> {
        let ctx = self.inner.context();
        let account_id = ctx.account_id.ok_or(NetworkError::InvalidPacket)?;
        let login_id1 = ctx.login_id1.ok_or(NetworkError::InvalidPacket)?;
        let login_id2 = ctx.login_id2.ok_or(NetworkError::InvalidPacket)?;
        let sex = ctx.sex;

        let packet = CharacterClientPacket::ChEnter(ChEnterPacket::new(
            account_id, login_id1, login_id2, sex,
        ));
        self.inner.send_packet(&packet)
    }

    /// Select a character to enter the game
    ///
    /// # Arguments
    ///
    /// * `slot` - Character slot number (0-based)
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn select_character(&mut self, slot: u8) -> NetworkResult<()> {
        let packet = CharacterClientPacket::ChSelectChar(ChSelectCharPacket::new(slot));
        self.inner.send_packet(&packet)
    }

    /// Create a new character
    ///
    /// # Arguments
    ///
    /// * `name` - Character name (max 24 chars)
    /// * `slot` - Character slot (0-based)
    /// * `hair_color` - Hair color ID
    /// * `hair_style` - Hair style ID
    /// * `starting_job` - Starting job ID (typically 0 for novice)
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn create_character(
        &mut self,
        name: &str,
        slot: u8,
        hair_color: u16,
        hair_style: u16,
        starting_job: u16,
    ) -> NetworkResult<()> {
        let sex = self.inner.context().sex;
        let packet = CharacterClientPacket::ChMakeChar(ChMakeCharPacket::new(
            name,
            slot,
            hair_color,
            hair_style,
            starting_job,
            sex,
        ));
        self.inner.send_packet(&packet)
    }

    /// Delete a character
    ///
    /// # Arguments
    ///
    /// * `char_id` - Character ID to delete
    /// * `email` - Email address for confirmation (optional, empty string if not used)
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn delete_character(&mut self, char_id: u32, email: &str) -> NetworkResult<()> {
        let packet = CharacterClientPacket::ChDeleteChar(ChDeleteCharPacket::new(char_id, email));
        self.inner.send_packet(&packet)
    }

    /// Send a ping to keep the connection alive
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    pub fn send_ping(&mut self) -> NetworkResult<()> {
        let account_id = self.inner.context().account_id.unwrap_or(0);
        let packet = CharacterClientPacket::ChPing(ChPingPacket::new(account_id));
        self.inner.send_packet(&packet)
    }

    /// Process incoming packets and emit Bevy events
    ///
    /// This should be called regularly (e.g., in a Bevy Update system) to:
    /// 1. Read data from the socket
    /// 2. Handle the special 4-byte account ID acknowledgment (if not yet received)
    /// 3. Parse complete packets
    /// 4. Dispatch to handlers
    /// 5. Populate the event buffer
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
        // First, receive any new data from the socket
        self.inner.receive_data()?;

        // Handle the special 4-byte account ID acknowledgment before regular packet processing
        // This is a protocol quirk: the character server sends a raw 4-byte account ID
        // immediately after CH_ENTER, which is NOT a standard packet
        if !self.inner.context().received_account_ack {
            if let Some(buffer) = self.inner.peek_buffer() {
                if buffer.len() >= 4 {
                    // Read the 4-byte account ID
                    let account_id =
                        u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

                    // Verify it matches our session
                    let expected_id = self.inner.context().account_id.unwrap_or(0);
                    if account_id == expected_id {
                        info!(
                            "Received and verified account ID acknowledgment: {}",
                            account_id
                        );

                        // Mark as received and consume the 4 bytes
                        self.inner.context_mut().acknowledge_account();
                        self.inner.consume_bytes(4)?;
                    } else {
                        error!(
                            "Account ID mismatch! Expected: {}, Got: {}",
                            expected_id, account_id
                        );
                        return Err(NetworkError::InvalidPacket);
                    }
                }
            }
        }

        // Now process regular packets (update() will call receive_data() again but that's fine)
        self.inner.update(event_buffer)
    }

    /// Disconnect from the character server
    ///
    /// Closes the TCP connection and clears all buffers.
    pub fn disconnect(&mut self) {
        self.inner.disconnect();
    }

    /// Check if currently connected to the character server
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// Get the list of characters on this account
    pub fn characters(&self) -> &[CharacterInfo] {
        &self.inner.context().characters
    }

    /// Get zone server info (available after character selection)
    pub fn zone_server_info(&self) -> Option<&ZoneServerInfo> {
        self.inner.context().zone_server_info.as_ref()
    }

    /// Get the selected character's data
    pub fn selected_character(&self) -> Option<&CharacterInfo> {
        self.inner.context().get_selected_character()
    }

    /// Check if the account acknowledgment was received
    pub fn is_acknowledged(&self) -> bool {
        self.inner.context().received_account_ack
    }

    /// Reset the context for a new connection
    pub fn reset_context(&mut self) {
        self.inner.context_mut().reset();
    }
}

/// SystemParam that bundles all CharacterServer event writers
///
/// This reduces the parameter count of char_server_update_system from 11 to 2
/// by grouping all related EventWriters into a single logical parameter.
#[derive(SystemParam)]
pub struct CharServerEventWriters<'w> {
    pub connected: EventWriter<'w, CharacterServerConnected>,
    pub zone_info: EventWriter<'w, ZoneServerInfoReceived>,
    pub char_created: EventWriter<'w, CharacterCreated>,
    pub char_creation_failed: EventWriter<'w, CharacterCreationFailed>,
    pub char_deleted: EventWriter<'w, CharacterDeleted>,
    pub char_deletion_failed: EventWriter<'w, CharacterDeletionFailed>,
    pub ping_received: EventWriter<'w, PingReceived>,
    pub second_password: EventWriter<'w, SecondPasswordRequested>,
    pub char_info_page: EventWriter<'w, CharacterInfoPageReceived>,
    pub char_slot_info: EventWriter<'w, CharacterSlotInfoReceived>,
    pub blocked_chars: EventWriter<'w, BlockedCharactersReceived>,
}

/// Bevy system to update the character server client
///
/// This system should run in the Update schedule. It processes incoming
/// packets and emits Bevy events for the game to handle.
///
/// # Requirements
///
/// - CharServerClient must be inserted as a Resource
/// - All character server events must be registered
///
/// # Example
///
/// ```ignore
/// app.insert_resource(CharServerClient::with_session(account_id, login_id1, login_id2))
///    .add_event::<CharacterServerConnected>()
///    .add_event::<ZoneServerInfoReceived>()
///    // ... other events
///    .add_systems(Update, char_server_update_system);
/// ```
pub fn char_server_update_system(
    client: Option<ResMut<CharServerClient>>,
    mut events: CharServerEventWriters,
) {
    let Some(mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    let mut event_buffer = EventBuffer::new();

    if let Err(e) = client.update(&mut event_buffer) {
        error!("Character server client error: {:?}", e);
        return;
    }

    // Dispatch type-erased events to their corresponding EventWriters
    crate::dispatch_events!(
        event_buffer,
        events,
        [
            (CharacterServerConnected, connected),
            (ZoneServerInfoReceived, zone_info),
            (CharacterCreated, char_created),
            (CharacterCreationFailed, char_creation_failed),
            (CharacterDeleted, char_deleted),
            (CharacterDeletionFailed, char_deletion_failed),
            (PingReceived, ping_received),
            (SecondPasswordRequested, second_password),
            (CharacterInfoPageReceived, char_info_page),
            (CharacterSlotInfoReceived, char_slot_info),
            (BlockedCharactersReceived, blocked_chars),
        ]
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_server_client_creation() {
        let client = CharServerClient::with_session(12345, 67890, 11111, 1);
        assert!(!client.is_connected());
        assert_eq!(client.characters().len(), 0);
        assert!(client.zone_server_info().is_none());
    }

    #[test]
    fn test_char_server_client_with_context() {
        let context = CharacterContext::with_session(12345, 67890, 11111, 1);
        let client = CharServerClient::new(context);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_character_list_access() {
        let client = CharServerClient::with_session(12345, 67890, 11111, 0);
        assert_eq!(client.characters().len(), 0);
        assert!(client.selected_character().is_none());
    }
}
