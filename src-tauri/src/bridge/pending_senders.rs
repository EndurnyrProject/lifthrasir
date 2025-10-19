use bevy::prelude::*;
use game_engine::domain::entities::character::components::CharacterInfo;
use std::collections::HashMap;
use tokio::sync::oneshot;

use super::app_bridge::SessionData;
use super::request_id::RequestId;

// ============================================================================
// Individual Pending Sender Collections
// ============================================================================

/// Stores pending oneshot senders for login requests
#[derive(Default)]
pub struct PendingLoginSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<SessionData, String>>>,
}

/// Stores pending oneshot senders for server selection requests
#[derive(Default)]
pub struct PendingServerSelectionSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<(), String>>>,
}

/// Stores pending oneshot senders for character list requests
#[derive(Default)]
pub struct PendingCharacterListSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<Vec<CharacterInfo>, String>>>,
}

/// Stores pending oneshot senders for character selection requests
#[derive(Default)]
pub struct PendingCharacterSelectionSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<(), String>>>,
}

/// Stores pending oneshot senders for character creation requests
#[derive(Default)]
pub struct PendingCharacterCreationSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<CharacterInfo, String>>>,
}

/// Stores pending oneshot senders for character deletion requests
#[derive(Default)]
pub struct PendingCharacterDeletionSenders {
    pub senders: HashMap<RequestId, oneshot::Sender<Result<(), String>>>,
}

/// Stores pending oneshot senders for hairstyle requests
#[derive(Default)]
pub struct PendingHairstyleSenders {
    pub senders:
        HashMap<RequestId, oneshot::Sender<Result<Vec<super::app_bridge::HairstyleInfo>, String>>>,
}

// ============================================================================
// Aggregated Resource
// ============================================================================

/// Aggregated resource for all pending senders
/// Allows centralized management of all async request/response channels
#[derive(Resource, Default)]
pub struct PendingSenders {
    pub logins: PendingLoginSenders,
    pub servers: PendingServerSelectionSenders,
    pub char_lists: PendingCharacterListSenders,
    pub char_selections: PendingCharacterSelectionSenders,
    pub char_creations: PendingCharacterCreationSenders,
    pub char_deletions: PendingCharacterDeletionSenders,
    pub hairstyles: PendingHairstyleSenders,
}
