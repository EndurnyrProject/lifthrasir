use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_system, auto_init_resource};
use secrecy::ExposeSecret;

use super::{events::*, models::*};
use crate::{
    core::state::GameState,
    infrastructure::{
        config::ClientConfig,
        networking::{
            client::{login_client_update_system, CharServerClient, LoginClient, LoginEventWriters},
            protocol::login::{LoginAccepted, LoginRefused},
            session::UserSession,
        },
    },
    presentation::ui::events::{LoginAttemptEvent, ServerSelectedEvent},
};

/// System to handle login attempts from the UI
///
/// When a user submits login credentials via the UI, this system:
/// 1. Connects to the login server
/// 2. Sends the login packet
/// 3. Emits a LoginAttemptStartedEvent for UI feedback
///
/// The response (success/failure) is handled by other systems that listen
/// to LoginAccepted and LoginRefused protocol events.
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn handle_login_attempts(
    mut login_attempts: EventReader<LoginAttemptEvent>,
    mut login_started_events: EventWriter<LoginAttemptStartedEvent>,
    mut login_client: ResMut<LoginClient>,
    auth_context: Res<AuthenticationContext>,
) {
    for attempt in login_attempts.read() {
        let server_address = &auth_context.server_config.login_server_address;
        let client_version = auth_context.server_config.client_version;
        let username = &attempt.username;
        let password = attempt.password.expose_secret();

        info!("Login attempt for user: {}", username);

        // Connect to login server (non-blocking)
        if let Err(e) = login_client.connect(server_address) {
            error!(
                "Failed to connect to login server {}: {:?}",
                server_address, e
            );
            continue;
        }

        // Send login packet (non-blocking)
        if let Err(e) = login_client.attempt_login(username, password, client_version) {
            error!("Failed to send login packet for {}: {:?}", username, e);
            login_client.disconnect();
            continue;
        }

        // Emit event for UI feedback
        login_started_events.write(LoginAttemptStartedEvent {
            username: username.clone(),
        });
    }
}

/// System to handle successful login from protocol layer
///
/// When the login server accepts the login (AC_ACCEPT_LOGIN packet),
/// the LoginClient emits a LoginAccepted event. This system:
/// 1. Creates a UserSession with the login tokens
/// 2. Inserts it as a resource
/// 3. Transitions to ServerSelection state
/// 4. Disconnects from login server (no longer needed)
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn handle_login_accepted(
    mut protocol_events: EventReader<LoginAccepted>,
    mut domain_events: EventWriter<LoginSuccessEvent>,
    mut login_client: ResMut<LoginClient>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for event in protocol_events.read() {
        info!("✅ Login accepted for account_id: {}", event.account_id);
        info!(
            "📋 Server list contains {} servers",
            event.server_list.len()
        );

        // Create UserSession from LoginAccepted event data
        // We need to reconstruct AcAcceptLoginPacket from the event fields
        let login_packet =
            crate::infrastructure::networking::protocol::login::AcAcceptLoginPacket {
                account_id: event.account_id,
                login_id1: event.login_id1,
                login_id2: event.login_id2,
                last_login_ip: event.last_login_ip,
                last_login_time: [0u8; 26], // Not available in event, but not critical
                sex: event.sex,
                server_list: event.server_list.clone(),
            };

        let session = crate::infrastructure::networking::session::UserSession::new(
            event.username.clone(), // Use username from event
            login_packet,
        );

        info!(
            "💾 Inserting UserSession resource with {} server(s)",
            session.server_list.len()
        );
        // Insert session as resource for other systems
        commands.insert_resource(session.clone());

        info!("📤 Emitting LoginSuccessEvent for UI notification");
        // Emit domain event for UI feedback
        domain_events.write(LoginSuccessEvent { session });

        info!("🔌 Disconnecting from login server (no longer needed)");
        // Disconnect from login server - we don't need it anymore
        login_client.disconnect();
        login_client.reset_context();

        info!("🎯 Transitioning to GameState::ServerSelection");
        // Transition to server selection
        next_state.set(GameState::ServerSelection);
    }
}

/// System to handle failed login from protocol layer
///
/// When the login server refuses the login (AC_REFUSE_LOGIN packet),
/// the LoginClient emits a LoginRefused event. This system:
/// 1. Logs the error
/// 2. Emits a LoginFailureEvent for UI feedback
/// 3. Disconnects and resets the client
/// 4. Returns to Login state
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn handle_login_refused(
    mut protocol_events: EventReader<LoginRefused>,
    mut domain_events: EventWriter<LoginFailureEvent>,
    mut login_client: ResMut<LoginClient>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in protocol_events.read() {
        warn!("Login refused with error code: {}", event.error_code);

        // Convert error code to NetworkError
        let error = crate::infrastructure::networking::errors::NetworkError::AuthenticationFailed {
            reason: format!("Login refused by server (error code: {})", event.error_code),
        };

        // Emit domain event for UI feedback
        domain_events.write(LoginFailureEvent {
            error,
            username: String::new(), // Username not available in protocol event
        });

        // Disconnect and reset for next attempt
        login_client.disconnect();
        login_client.reset_context();

        // Return to login screen
        next_state.set(GameState::Login);
    }
}

// ============================================================================
// Configuration and Client Initialization Systems
// ============================================================================

/// Resource to hold the client config handle
#[derive(Resource)]
struct ClientConfigHandle(Handle<ClientConfig>);

/// Resource to track if config is already loaded
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
struct ConfigLoaded(bool);

/// System to load client configuration (runs only once)
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
fn load_client_config(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config_handle: Option<Res<ClientConfigHandle>>,
) {
    // Only load if handle doesn't exist (ConfigLoaded is auto-initialized)
    if config_handle.is_none() {
        let handle = asset_server.load::<ClientConfig>("config/clientinfo.client.toml");
        commands.insert_resource(ClientConfigHandle(handle));
        info!("Loading client configuration from config/clientinfo.client.toml");
    }
}

/// System to check if config is loaded and apply it
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update,
    config(
        after = load_client_config
    )
)]
fn check_client_config_loaded(
    config_handle: Option<Res<ClientConfigHandle>>,
    client_configs: Res<Assets<ClientConfig>>,
    mut config_loaded: ResMut<ConfigLoaded>,
    mut auth_context: ResMut<AuthenticationContext>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // ConfigLoaded is now auto-initialized, so it always exists
    if let Some(handle) = config_handle {
        if !config_loaded.0 {
            if let Some(config) = client_configs.get(&handle.0) {
                info!("Applying client configuration from clientinfo.client.toml");

                // Update authentication context with loaded configuration
                auth_context.server_config = ServerConfiguration {
                    login_server_address: config.server.to_address(),
                    client_version: config.server.client_version,
                    default_port: config.server.port,
                };

                info!(
                    "Client configured - Server: {}, Version: {}",
                    auth_context.server_config.login_server_address,
                    auth_context.server_config.client_version
                );

                // Mark as loaded to prevent repeated execution
                config_loaded.0 = true;

                // Transition to Login state once config is loaded
                next_state.set(GameState::Login);
            }
        }
    }
}

/// Login client update system wrapper
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
fn run_login_client_update(
    client: Option<ResMut<LoginClient>>,
    events: LoginEventWriters,
) {
    login_client_update_system(client, events);
}

// ============================================================================
// Server Selection System
// ============================================================================

/// System that handles server selection events
///
/// Updates the session and connects to character server (UI flow handled by Tauri)
#[auto_add_system(
    plugin = crate::app::authentication_plugin::AuthenticationPlugin,
    schedule = Update
)]
pub fn handle_server_selection(
    mut commands: Commands,
    mut server_events: EventReader<ServerSelectedEvent>,
    session: Option<Res<UserSession>>,
    mut char_client: Option<ResMut<CharServerClient>>,
) {
    // Only process if UserSession exists
    let Some(mut session) = session.map(|s| s.clone()) else {
        return;
    };

    for event in server_events.read() {
        info!("Server selected: {}", event.server.name);

        session.selected_server = Some(event.server.clone());

        // Update the resource with the new session
        commands.insert_resource(session.clone());

        // Build address string
        let ip_bytes = event.server.ip.to_be_bytes();
        let server_ip = format!(
            "{}.{}.{}.{}",
            ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
        );
        let address = format!("{}:{}", server_ip, event.server.port);

        if let Some(ref mut client) = char_client {
            client.disconnect();

            // Connect to the selected server
            if let Err(e) = client.connect(&address) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!("Connected to character server at {}", address);
                // Send CH_ENTER packet immediately after connection
                if let Err(e) = client.enter_server() {
                    error!("Failed to send CH_ENTER: {:?}", e);
                }
            }
        } else {
            let mut client = CharServerClient::with_session(
                session.tokens.account_id,
                session.tokens.login_id1,
                session.tokens.login_id2,
                session.sex,
            );

            if let Err(e) = client.connect(&address) {
                error!("Failed to connect to character server: {:?}", e);
            } else {
                info!("Connected to character server at {}", address);
                // Send CH_ENTER packet immediately after connection
                if let Err(e) = client.enter_server() {
                    error!("Failed to send CH_ENTER: {:?}", e);
                }
            }

            commands.insert_resource(client);
        }
    }
}
