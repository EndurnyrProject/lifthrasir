use serde::{Deserialize, Serialize};

/// Server type enum for better type safety
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerType {
    Normal,
    Maintenance,
    PvP,
    PK,
    Special(u16),
}

impl From<u16> for ServerType {
    fn from(value: u16) -> Self {
        match value {
            0 => ServerType::Normal,
            1 => ServerType::Maintenance,
            2 => ServerType::PvP,
            3 => ServerType::PK,
            other => ServerType::Special(other),
        }
    }
}

impl ServerType {
    pub fn as_u16(&self) -> u16 {
        match self {
            ServerType::Normal => 0,
            ServerType::Maintenance => 1,
            ServerType::PvP => 2,
            ServerType::PK => 3,
            ServerType::Special(value) => *value,
        }
    }
}

/// Information about a game server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub ip: u32,
    pub port: u16,
    pub name: String,
    pub users: u16,
    pub server_type: ServerType,
    pub new_server: u16,
}

impl ServerInfo {
    /// Convert IP address to dotted decimal notation
    pub fn ip_string(&self) -> String {
        let bytes = self.ip.to_be_bytes();
        format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
    }
}
