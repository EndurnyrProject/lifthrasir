use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
#[auto_init_resource(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct ServerConfiguration {
    pub login_server_address: String,
    pub client_version: u32,
    pub default_port: u16,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            login_server_address: "127.0.0.1:6900".to_string(),
            client_version: 1,
            default_port: 6900,
        }
    }
}

#[derive(Debug, Clone, Default, Resource)]
#[auto_init_resource(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct AuthenticationContext {
    pub server_config: ServerConfiguration,
}
