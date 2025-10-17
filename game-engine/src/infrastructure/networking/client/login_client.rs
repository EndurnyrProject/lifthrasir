use crate::infrastructure::networking::{
    client::NetworkClient,
    errors::NetworkResult,
    protocol::{
        dispatcher::PacketDispatcher,
        login::{
            AcceptLoginHandler, CaLoginPacket, LoginAccepted, LoginClientPacket, LoginContext,
            LoginProtocol, LoginRefused, RefuseLoginHandler,
        },
        EventBuffer,
    },
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_init_resource;

/// High-level client for the Login Protocol
///
/// LoginClient provides a convenient wrapper around NetworkClient<LoginProtocol>
/// with a clean API for Bevy systems. It handles:
/// - Connection management
/// - Login requests
/// - Event emission for Bevy ECS
///
/// # Example
///
/// ```ignore
/// // In your Bevy app setup:
/// app.insert_resource(LoginClient::new());
///
/// // In a system:
/// fn login_system(mut client: ResMut<LoginClient>) {
///     client.connect("127.0.0.1:6900").unwrap();
///     client.attempt_login("testuser", "testpass", 55).unwrap();
/// }
///
/// // In your update system:
/// fn update_system(
///     mut client: ResMut<LoginClient>,
///     mut login_accepted: EventWriter<LoginAccepted>,
///     mut login_refused: EventWriter<LoginRefused>,
/// ) {
///     let mut event_buffer = EventBuffer::new();
///     client.update(&mut event_buffer).unwrap();
///     event_buffer.flush(&mut login_accepted, &mut login_refused);
/// }
/// ```
#[derive(Resource)]
#[auto_init_resource(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginClient {
    inner: NetworkClient<LoginProtocol>,
}

impl LoginClient {
    /// Create a new login client
    ///
    /// The client is initialized with:
    /// - Empty login context
    /// - Registered packet handlers for AC_ACCEPT_LOGIN and AC_REFUSE_LOGIN
    pub fn new() -> Self {
        let mut dispatcher = PacketDispatcher::new();

        // Register handlers for all login server packets
        dispatcher.register(AcceptLoginHandler);
        dispatcher.register(RefuseLoginHandler);

        let context = LoginContext::new();
        let client = NetworkClient::new(context).with_dispatcher(dispatcher);

        Self { inner: client }
    }

    /// Connect to a login server
    ///
    /// # Arguments
    ///
    /// * `address` - Server address as "ip:port" (e.g., "127.0.0.1:6900")
    ///
    /// # Returns
    ///
    /// Ok(()) if connection succeeds, NetworkError otherwise
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.connect("127.0.0.1:6900")?;
    /// ```
    pub fn connect(&mut self, address: &str) -> NetworkResult<()> {
        self.inner.connect(address)
    }

    /// Attempt to log in with credentials
    ///
    /// Sends a CA_LOGIN packet to the server with the provided credentials.
    /// The server will respond with either AC_ACCEPT_LOGIN or AC_REFUSE_LOGIN.
    ///
    /// # Arguments
    ///
    /// * `username` - Account username (max 23 chars)
    /// * `password` - Account password (max 23 chars)
    /// * `version` - Client version (typically 55)
    ///
    /// # Returns
    ///
    /// Ok(()) if packet was sent, NetworkError otherwise
    ///
    /// # Note
    ///
    /// This does not wait for the server response. Use `update()` to process
    /// the response and emit Bevy events.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.attempt_login("myuser", "mypass", 55)?;
    /// ```
    pub fn attempt_login(
        &mut self,
        username: &str,
        password: &str,
        version: u32,
    ) -> NetworkResult<()> {
        // Record attempt in context with username
        self.inner
            .context_mut()
            .record_attempt(username.to_string());

        let packet = LoginClientPacket::CaLogin(CaLoginPacket::new(username, password, version));
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
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut event_buffer = EventBuffer::new();
    /// client.update(&mut event_buffer)?;
    /// // Extract events in update system
    /// ```
    pub fn update(&mut self, event_buffer: &mut EventBuffer) -> NetworkResult<()> {
        self.inner.update(event_buffer)
    }

    /// Disconnect from the login server
    ///
    /// Closes the TCP connection and clears all buffers.
    pub fn disconnect(&mut self) {
        self.inner.disconnect();
    }

    /// Check if currently connected to the login server
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// Get the number of login attempts made
    pub fn attempt_count(&self) -> u32 {
        self.inner.context().attempt_count
    }

    /// Get the last error code received (if any)
    pub fn last_error(&self) -> Option<u8> {
        self.inner.context().last_error
    }

    /// Reset the login context (attempts, errors)
    pub fn reset_context(&mut self) {
        self.inner.context_mut().reset();
    }
}

impl Default for LoginClient {
    fn default() -> Self {
        Self::new()
    }
}

/// SystemParam that bundles all LoginServer event writers
///
/// This reduces the parameter count of login_client_update_system from 3 to 2
/// by grouping all related EventWriters into a single logical parameter.
#[derive(SystemParam)]
pub struct LoginEventWriters<'w> {
    pub accepted: EventWriter<'w, LoginAccepted>,
    pub refused: EventWriter<'w, LoginRefused>,
}

/// Bevy system to update the login client
///
/// This system should run in the Update schedule. It processes incoming
/// packets and emits Bevy events for the game to handle.
///
/// # Requirements
///
/// - LoginClient must be inserted as a Resource
/// - LoginAccepted and LoginRefused events must be registered
///
/// # Example
///
/// ```ignore
/// app.insert_resource(LoginClient::new())
///    .add_event::<LoginAccepted>()
///    .add_event::<LoginRefused>()
///    .add_systems(Update, login_client_update_system);
/// ```
pub fn login_client_update_system(
    client: Option<ResMut<LoginClient>>,
    mut events: LoginEventWriters,
) {
    let Some(mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    let mut event_buffer = EventBuffer::new();

    if let Err(e) = client.update(&mut event_buffer) {
        error!("Login client error: {:?}", e);
        return;
    }

    // Dispatch type-erased events to their corresponding EventWriters
    crate::dispatch_events!(event_buffer, events, [
        (LoginAccepted, accepted),
        (LoginRefused, refused),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_client_creation() {
        let client = LoginClient::new();
        assert!(!client.is_connected());
        assert_eq!(client.attempt_count(), 0);
        assert_eq!(client.last_error(), None);
    }

    #[test]
    fn test_login_client_default() {
        let client = LoginClient::default();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_context_tracking() {
        let mut client = LoginClient::new();
        assert_eq!(client.attempt_count(), 0);

        // Simulate failed connection (can't actually connect in unit test)
        // Just verify the API works
        client.reset_context();
        assert_eq!(client.attempt_count(), 0);
    }
}
