use crate::{
    core::state::GameState,
    domain::authentication::{
        events::*,
        models::{AuthenticationContext, ServerConfiguration},
        systems::*,
    },
    infrastructure::config::ClientConfig,
};
use bevy::prelude::*;

pub struct AuthenticationPlugin;

impl Plugin for AuthenticationPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add authentication resources
            .insert_resource(AuthenticationContext::default())
            // Add authentication events
            .add_event::<LoginAttemptStartedEvent>()
            .add_event::<LoginSuccessEvent>()
            .add_event::<LoginFailureEvent>()
            .add_event::<LogoutEvent>()
            .add_event::<SessionExpiredEvent>()
            // Load and apply client configuration
            .add_systems(OnEnter(GameState::Loading), load_client_config)
            .add_systems(
                Update,
                check_client_config_loaded.run_if(in_state(GameState::Loading)),
            )
            // Add authentication systems for Connecting state
            .add_systems(
                Update,
                (handle_login_attempts, poll_login_tasks).run_if(in_state(GameState::Connecting)),
            )
            // Add authentication result handlers
            .add_systems(
                Update,
                (
                    handle_login_success,
                    handle_login_failure,
                    cleanup_failed_connections,
                ),
            );
    }
}

// Resource to hold the client config handle
#[derive(Resource)]
struct ClientConfigHandle(Handle<ClientConfig>);

// System to load client configuration
fn load_client_config(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Load the client configuration file with .client.toml extension
    let handle = asset_server.load::<ClientConfig>("config/clientinfo.client.toml");
    commands.insert_resource(ClientConfigHandle(handle));
    info!("Loading client configuration from config/clientinfo.client.toml");
}

// System to check if config is loaded and apply it
fn check_client_config_loaded(
    config_handle: Res<ClientConfigHandle>,
    client_configs: Res<Assets<ClientConfig>>,
    mut auth_context: ResMut<AuthenticationContext>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(config) = client_configs.get(&config_handle.0) {
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

        // Transition to Login state once config is loaded
        next_state.set(GameState::Login);
    }
}
