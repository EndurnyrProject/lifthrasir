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
use bevy_auto_plugin::modes::global::prelude::{
    AutoPlugin, auto_add_system, auto_init_resource, auto_plugin,
};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AuthenticationPlugin;

// Resource to hold the client config handle
#[derive(Resource)]
struct ClientConfigHandle(Handle<ClientConfig>);

// Resource to track if config is already loaded
#[derive(Resource, Default)]
struct ConfigLoaded(bool);

// System to load client configuration (runs only once)
#[auto_add_system(
    plugin = AuthenticationPlugin,
    schedule = Update
)]
fn load_client_config(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config_loaded: Option<Res<ConfigLoaded>>,
    config_handle: Option<Res<ClientConfigHandle>>,
) {
    // Only load if not already loaded and handle doesn't exist
    if config_loaded.is_none() && config_handle.is_none() {
        let handle = asset_server.load::<ClientConfig>("config/clientinfo.client.toml");
        commands.insert_resource(ClientConfigHandle(handle));
        commands.insert_resource(ConfigLoaded(false));
        info!("Loading client configuration from config/clientinfo.client.toml");
    }
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
    config_handle: Option<Res<ClientConfigHandle>>,
    client_configs: Res<Assets<ClientConfig>>,
    mut config_loaded: Option<ResMut<ConfigLoaded>>,
    mut auth_context: ResMut<AuthenticationContext>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let (Some(handle), Some(mut loaded)) = (config_handle, config_loaded) {
        if !loaded.0 {
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
                loaded.0 = true;

                // Transition to Login state once config is loaded
                next_state.set(GameState::Login);
            }
        }
    }
}
