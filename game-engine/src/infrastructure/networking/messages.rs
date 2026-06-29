use crate::infrastructure::networking::server_info::ServerInfo;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Event emitted when login is accepted
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAccepted {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u8,
    pub server_list: Vec<ServerInfo>,
    pub username: String,
    pub auth_token: String,
}

/// Event emitted when login is refused
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginRefused {
    pub username: String,
    pub error_code: u8,
    pub error_message: String,
    pub block_date: Option<String>,
}
