use bevy::prelude::*;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LoginCredentials {
    pub username: String,
    pub password: SecretString,
    pub remember_me: bool,
}

// Custom Serialize implementation to avoid serializing the password
impl Serialize for LoginCredentials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LoginCredentials", 2)?;
        state.serialize_field("username", &self.username)?;
        state.serialize_field("remember_me", &self.remember_me)?;
        state.end()
    }
}

// Custom Deserialize implementation
impl<'de> Deserialize<'de> for LoginCredentials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TempCredentials {
            username: String,
            password: String,
            remember_me: bool,
        }

        let temp = TempCredentials::deserialize(deserializer)?;
        Ok(LoginCredentials {
            username: temp.username,
            password: SecretString::from(temp.password),
            remember_me: temp.remember_me,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
pub struct ServerConfiguration {
    pub login_server_address: String,
    pub client_version: u32,
    pub default_port: u16,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            login_server_address: "127.0.0.1:6900".to_string(),
            client_version: 20180620, // Example RO client version
            default_port: 6900,
        }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct AuthenticationContext {
    pub server_config: ServerConfiguration,
    pub retry_attempts: u32,
    pub max_retry_attempts: u32,
}

impl Default for AuthenticationContext {
    fn default() -> Self {
        Self {
            server_config: ServerConfiguration::default(),
            retry_attempts: 0,
            max_retry_attempts: 3,
        }
    }
}
