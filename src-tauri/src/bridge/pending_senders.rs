use crate::bridge::SessionData;
use bevy::prelude::*;
use game_engine::domain::entities::character::components::CharacterInfo;
use std::collections::HashMap;
use tokio::sync::oneshot;

/// Stores pending oneshot senders for login requests
#[derive(Resource, Default)]
pub struct PendingLoginSenders {
    pub senders: HashMap<String, oneshot::Sender<Result<SessionData, String>>>,
}

/// Stores pending oneshot senders for server selection requests
#[derive(Resource, Default)]
pub struct PendingServerSelectionSenders {
    pub senders: HashMap<usize, oneshot::Sender<Result<(), String>>>,
}

/// Stores pending oneshot senders for character list requests
#[derive(Resource, Default)]
pub struct PendingCharacterListSenders {
    pub senders: Vec<oneshot::Sender<Result<Vec<CharacterInfo>, String>>>,
}

/// Stores pending oneshot senders for character selection requests
#[derive(Resource, Default)]
pub struct PendingCharacterSelectionSenders {
    pub senders: HashMap<u8, oneshot::Sender<Result<(), String>>>,
}

/// Stores pending oneshot senders for character creation requests
#[derive(Resource, Default)]
pub struct PendingCharacterCreationSenders {
    pub senders: HashMap<u8, oneshot::Sender<Result<CharacterInfo, String>>>,
}

/// Stores pending oneshot senders for character deletion requests
#[derive(Resource, Default)]
pub struct PendingCharacterDeletionSenders {
    pub senders: HashMap<u32, oneshot::Sender<Result<(), String>>>,
}

/// Aggregated resource for all pending senders
#[derive(Resource, Default)]
pub struct PendingSenders {
    pub logins: PendingLoginSenders,
    pub servers: PendingServerSelectionSenders,
    pub char_lists: PendingCharacterListSenders,
    pub char_selections: PendingCharacterSelectionSenders,
    pub char_creations: PendingCharacterCreationSenders,
    pub char_deletions: PendingCharacterDeletionSenders,
}
