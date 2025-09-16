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
use bevy_auto_plugin::modes::global::prelude::{auto_plugin, auto_init_resource, auto_add_system, AutoPlugin};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AuthenticationPlugin;

// Resource to hold the client config handle
#[derive(Resource)]
struct ClientConfigHandle(Handle<ClientConfig>);

// System to load client configuration
#[auto_add_system(
    plugin = AuthenticationPlugin,
    schedule = Update
)]
fn load_client_config(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Load the client configuration file with .client.toml extension
    let handle = asset_server.load::<ClientConfig>("config/clientinfo.client.toml");
    commands.insert_resource(ClientConfigHandle(handle));
    info!("Loading client configuration from config/clientinfo.client.toml");
}

// System to check if config is loaded and apply it
#[auto_add_system(
    plugin = AuthenticationPlugin,
    schedule = Update,
    config(
        after = load_client_config
    )
)]
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
