use serde::{Deserialize, Serialize};

/// Character information structure
///
/// Contains all data about a character including stats, appearance,
/// equipment, and progression. This structure is used across multiple
/// packets (HC_ACCEPT_ENTER, HC_ACCEPT_MAKECHAR, HC_ACK_CHARINFO_PER_PAGE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub char_id: u32,
    pub base_exp: u64,
    pub zeny: u32,
    pub job_exp: u64,
    pub job_level: u32,
    pub body_state: u32,
    pub health_state: u32,
    pub option: u32,
    pub karma: u32,
    pub manner: u32,
    pub status_point: u16,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub walk_speed: u16,
    pub class: u16,
    pub hair: u16,
    pub body: u16,
    pub weapon: u16,
    pub base_level: u16,
    pub skill_point: u16,
    pub head_bottom: u16,
    pub shield: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub hair_color: u16,
    pub clothes_color: u16,
    pub name: String,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
    pub char_num: u8,
    pub rename: u8,
    pub last_map: String,
    pub delete_date: u32,
    pub robe: u32,
    pub char_slot_change: u32,
    pub char_rename: u32,
    pub sex: u8,
}

/// Zone server connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneServerInfo {
    pub char_id: u32,
    pub map_name: String,
    pub ip: [u8; 4],
    pub port: u16,
    /// Single-use handoff token to echo back in zone `SessionAuth.zone_auth_token`.
    pub auth_token: Vec<u8>,
}

impl ZoneServerInfo {
    /// Convert IP address to dotted decimal notation
    pub fn ip_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip[0], self.ip[1], self.ip[2], self.ip[3]
        )
    }
}

/// Character slot information from HC_CHARACTER_LIST packet
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterSlotInfo {
    pub normal_slots: u8,
    pub premium_slots: u8,
    pub billing_slots: u8,
    pub producible_slots: u8,
    pub valid_slots: u8,
}

/// Character creation error codes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharCreationError {
    /// Character name already exists
    NameExists,
    /// Character name contains invalid characters
    InvalidName,
    /// Other error
    Unknown(u8),
}

impl From<u8> for CharCreationError {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CharCreationError::NameExists,
            0xFF => CharCreationError::InvalidName,
            other => CharCreationError::Unknown(other),
        }
    }
}

/// Character deletion error codes (HC_CHAR_DELETE2_ACK result codes)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharDeletionError {
    /// Database error
    DatabaseError,
    /// Character doesn't belong to account
    NotFound,
    /// Character already marked for deletion
    AlreadyDeleting,
    /// Cannot delete character (guild member, has items, etc.)
    CannotDelete,
    /// Other error
    Unknown(u32),
}

impl From<u32> for CharDeletionError {
    fn from(value: u32) -> Self {
        match value {
            1 => CharDeletionError::DatabaseError,
            2 => CharDeletionError::NotFound,
            3 => CharDeletionError::AlreadyDeleting,
            4 => CharDeletionError::CannotDelete,
            other => CharDeletionError::Unknown(other),
        }
    }
}

impl CharDeletionError {
    pub fn description(&self) -> &'static str {
        match self {
            CharDeletionError::DatabaseError => "Database error",
            CharDeletionError::NotFound => "Character not found",
            CharDeletionError::AlreadyDeleting => "Character already marked for deletion",
            CharDeletionError::CannotDelete => "Cannot delete character",
            CharDeletionError::Unknown(_) => "Unknown error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_creation_error_from_code() {
        assert_eq!(CharCreationError::from(0x00), CharCreationError::NameExists);
        assert_eq!(
            CharCreationError::from(0xFF),
            CharCreationError::InvalidName
        );
        assert_eq!(CharCreationError::from(7), CharCreationError::Unknown(7));
    }

    #[test]
    fn char_deletion_error_from_code() {
        assert_eq!(CharDeletionError::from(2), CharDeletionError::NotFound);
    }

    #[test]
    fn zone_server_info_ip_string() {
        let info = ZoneServerInfo {
            char_id: 1,
            map_name: "prontera".into(),
            ip: [127, 0, 0, 1],
            port: 5121,
            auth_token: vec![],
        };
        assert_eq!(info.ip_string(), "127.0.0.1");
    }
}
