use super::protocols::ro_login::{AcAcceptLoginPacket, ServerInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokens {
    pub login_id1: u32,
    pub account_id: u32,
    pub login_id2: u32,
    pub character_server_info: Option<ServerInfo>,
}

impl From<AcAcceptLoginPacket> for SessionTokens {
    fn from(packet: AcAcceptLoginPacket) -> Self {
        Self {
            login_id1: packet.login_id1,
            account_id: packet.account_id,
            login_id2: packet.login_id2,
            character_server_info: packet.server_list.first().cloned(),
        }
    }
}

#[derive(Debug, Clone, bevy::prelude::Resource)]
pub struct UserSession {
    pub username: String,
    pub tokens: SessionTokens,
    pub login_timestamp: std::time::SystemTime,
    pub last_login_ip: u32,
    pub sex: u8,
}

impl UserSession {
    pub fn new(username: String, login_response: AcAcceptLoginPacket) -> Self {
        Self {
            username,
            tokens: SessionTokens::from(login_response.clone()),
            login_timestamp: std::time::SystemTime::now(),
            last_login_ip: login_response.last_login_ip,
            sex: login_response.sex,
        }
    }
}
