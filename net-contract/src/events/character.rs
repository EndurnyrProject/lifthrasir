use crate::dto::{
    CharCreationError, CharDeletionError, CharacterInfo, CharacterSlotInfo, ZoneServerInfo,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Event emitted when character server connection is accepted
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterServerConnected {
    pub max_slots: u8,
    pub available_slots: u8,
    pub premium_slots: u8,
    /// Number of char-select pages the server reports (`CharList.page_count`).
    pub display_pages: u32,
    pub characters: Vec<CharacterInfo>,
}

/// Event emitted when character slot information is received
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterSlotInfoReceived {
    pub slot_info: CharacterSlotInfo,
}

/// Event emitted when zone server connection info is received
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ZoneServerInfoReceived {
    pub zone_server_info: ZoneServerInfo,
}

/// Event emitted when character creation succeeds
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterCreated {
    pub character: CharacterInfo,
}

/// Event emitted when character creation fails
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterCreationFailed {
    pub error: CharCreationError,
}

/// Event emitted when character deletion succeeds
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterDeleted {
    pub char_id: u32,
}

/// Event emitted when character deletion fails
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct CharacterDeletionFailed {
    pub char_id: u32,
    pub error: CharDeletionError,
}
