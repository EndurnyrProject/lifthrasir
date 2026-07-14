//! Connection state resources.

use crate::dto::ServerInfo;
use crate::events::LoginAccepted;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use serde::{Deserialize, Serialize};

/// Neutral, adapter-agnostic identity of the active zone session.
#[derive(Resource, Default, Debug, Clone)]
#[auto_init_resource(plugin = crate::NetContractPlugin)]
pub struct ZoneSession {
    pub char_id: u32,
    pub account_id: u32,
    pub map_name: String,
}

/// Monotonic identity of the current zone/character session.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_init_resource(plugin = crate::NetContractPlugin)]
pub struct ZoneSessionGeneration(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokens {
    pub login_id1: u32,
    pub account_id: u32,
    pub login_id2: u32,
    pub character_server_info: Option<ServerInfo>,
}

#[derive(Debug, Clone, bevy::prelude::Resource)]
pub struct UserSession {
    pub username: String,
    pub tokens: SessionTokens,
    pub login_timestamp: std::time::SystemTime,
    // NOTE: always 0 today; server does not send it yet.
    pub last_login_ip: u32,
    pub sex: u8,
    pub server_list: Vec<ServerInfo>,
    pub selected_server: Option<ServerInfo>,
    pub auth_token: String,
}

impl From<&LoginAccepted> for UserSession {
    fn from(event: &LoginAccepted) -> Self {
        Self {
            username: event.username.clone(),
            tokens: SessionTokens {
                login_id1: event.login_id1,
                account_id: event.account_id,
                login_id2: event.login_id2,
                character_server_info: event.server_list.first().cloned(),
            },
            login_timestamp: std::time::SystemTime::now(),
            last_login_ip: 0,
            sex: event.sex,
            server_list: event.server_list.clone(),
            selected_server: None,
            auth_token: event.auth_token.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ServerType;

    fn sample_server() -> ServerInfo {
        ServerInfo {
            ip: 0x7F000001,
            port: 6121,
            name: "Aesir".to_string(),
            users: 3,
            server_type: ServerType::Normal,
            new_server: 0,
        }
    }

    #[test]
    fn maps_login_accepted_into_session() {
        let event = LoginAccepted {
            account_id: 2000000,
            login_id1: 11,
            login_id2: 22,
            sex: 1,
            server_list: vec![sample_server()],
            username: "hero".to_string(),
            auth_token: "deadbeefcafef00d".to_string(),
        };

        let session = UserSession::from(&event);

        assert_eq!(session.username, "hero");
        assert_eq!(session.sex, 1);
        assert_eq!(session.tokens.account_id, 2000000);
        assert_eq!(session.tokens.login_id1, 11);
        assert_eq!(session.tokens.login_id2, 22);
        assert_eq!(session.auth_token, "deadbeefcafef00d");
        assert_eq!(session.last_login_ip, 0);
        assert!(session.selected_server.is_none());
        assert_eq!(session.server_list.len(), 1);
        assert_eq!(session.tokens.character_server_info.unwrap().name, "Aesir");
    }

    #[test]
    fn character_server_info_is_none_without_servers() {
        let event = LoginAccepted {
            account_id: 1,
            login_id1: 0,
            login_id2: 0,
            sex: 0,
            server_list: vec![],
            username: "nobody".to_string(),
            auth_token: String::new(),
        };

        let session = UserSession::from(&event);

        assert!(session.tokens.character_server_info.is_none());
    }
}
