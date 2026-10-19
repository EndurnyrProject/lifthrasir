use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ip: String,
    pub port: u16,
    #[serde(default = "default_client_version")]
    pub client_version: u32,
}

fn default_client_version() -> u32 {
    20180620
}

impl ServerConfig {
    pub fn to_address(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                ip: "127.0.0.1".to_string(),
                port: 6900,
                client_version: default_client_version(),
            },
        }
    }
}
