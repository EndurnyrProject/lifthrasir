use bevy::prelude::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::request_id::RequestId;

/// Timeout for correlation entries (30 seconds)
const CORRELATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Wrapper for correlation entry with timestamp
#[derive(Clone, Copy)]
struct CorrelationEntry {
    request_id: RequestId,
    created_at: Instant,
}

impl CorrelationEntry {
    fn new(request_id: RequestId) -> Self {
        Self {
            request_id,
            created_at: Instant::now(),
        }
    }

    fn is_stale(&self) -> bool {
        self.created_at.elapsed() > CORRELATION_TIMEOUT
    }
}

/// Maps username to RequestId for correlating login responses
/// Populated when login request is processed, consumed when login response arrives
#[derive(Resource, Default)]
pub struct LoginCorrelation {
    entries: HashMap<String, CorrelationEntry>,
}

impl LoginCorrelation {
    pub fn insert(&mut self, username: String, request_id: RequestId) {
        self.entries
            .insert(username, CorrelationEntry::new(request_id));
    }

    pub fn remove(&mut self, username: &str) -> Option<RequestId> {
        self.entries.remove(username).map(|e| e.request_id)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial_count = self.entries.len();
        self.entries.retain(|_, entry| !entry.is_stale());
        initial_count - self.entries.len()
    }
}

/// Maps character slot and char_id to RequestId for correlating character responses
/// - slot is used for selection and creation
/// - char_id is used for deletion
#[derive(Resource, Default)]
pub struct CharacterCorrelation {
    slot_entries: HashMap<u8, CorrelationEntry>,
    char_id_entries: HashMap<u32, CorrelationEntry>,
}

impl CharacterCorrelation {
    pub fn insert_slot(&mut self, slot: u8, request_id: RequestId) {
        self.slot_entries
            .insert(slot, CorrelationEntry::new(request_id));
    }

    pub fn remove_slot(&mut self, slot: &u8) -> Option<RequestId> {
        self.slot_entries.remove(slot).map(|e| e.request_id)
    }

    pub fn insert_char_id(&mut self, char_id: u32, request_id: RequestId) {
        self.char_id_entries
            .insert(char_id, CorrelationEntry::new(request_id));
    }

    pub fn remove_char_id(&mut self, char_id: &u32) -> Option<RequestId> {
        self.char_id_entries.remove(char_id).map(|e| e.request_id)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial_slot = self.slot_entries.len();
        let initial_char_id = self.char_id_entries.len();

        self.slot_entries.retain(|_, entry| !entry.is_stale());
        self.char_id_entries.retain(|_, entry| !entry.is_stale());

        (initial_slot - self.slot_entries.len())
            + (initial_char_id - self.char_id_entries.len())
    }
}

/// Maps server index to RequestId for correlating server selection responses
#[derive(Resource, Default)]
pub struct ServerCorrelation {
    entries: HashMap<usize, CorrelationEntry>,
}

impl ServerCorrelation {
    pub fn insert(&mut self, index: usize, request_id: RequestId) {
        self.entries
            .insert(index, CorrelationEntry::new(request_id));
    }

    pub fn remove(&mut self, index: &usize) -> Option<RequestId> {
        self.entries.remove(index).map(|e| e.request_id)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial_count = self.entries.len();
        self.entries.retain(|_, entry| !entry.is_stale());
        initial_count - self.entries.len()
    }
}

/// System that periodically cleans up stale correlation entries
/// Runs every 10 seconds to remove entries older than 30 seconds
pub fn cleanup_stale_correlations(
    mut login_corr: ResMut<LoginCorrelation>,
    mut char_corr: ResMut<CharacterCorrelation>,
    mut server_corr: ResMut<ServerCorrelation>,
    mut last_cleanup: Local<Option<Instant>>,
) {
    let now = Instant::now();

    // Only run cleanup every 10 seconds
    if let Some(last) = *last_cleanup {
        if now.duration_since(last) < Duration::from_secs(10) {
            return;
        }
    }

    let login_cleaned = login_corr.cleanup_stale();
    let char_cleaned = char_corr.cleanup_stale();
    let server_cleaned = server_corr.cleanup_stale();

    let total_cleaned = login_cleaned + char_cleaned + server_cleaned;
    if total_cleaned > 0 {
        warn!(
            "Cleaned up {} stale correlation entries (login: {}, character: {}, server: {})",
            total_cleaned, login_cleaned, char_cleaned, server_cleaned
        );
    }

    *last_cleanup = Some(now);
}
