use bevy::prelude::*;
use game_engine::domain::character::events::CharacterInfoWithJobName;
use game_engine::domain::entities::character::components::CharacterInfo;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

use super::app_bridge::{HairstyleInfo, SessionData};

const CORRELATION_TIMEOUT: Duration = Duration::from_secs(30);

struct CorrelationEntry<T> {
    sender: oneshot::Sender<T>,
    created_at: Instant,
}

impl<T> CorrelationEntry<T> {
    fn new(sender: oneshot::Sender<T>) -> Self {
        Self {
            sender,
            created_at: Instant::now(),
        }
    }

    fn is_stale(&self) -> bool {
        self.created_at.elapsed() > CORRELATION_TIMEOUT
    }
}

#[derive(Resource, Default)]
pub struct LoginCorrelation {
    entries: HashMap<String, CorrelationEntry<Result<SessionData, String>>>,
}

impl LoginCorrelation {
    pub fn insert(
        &mut self,
        username: String,
        sender: oneshot::Sender<Result<SessionData, String>>,
    ) {
        self.entries.insert(username, CorrelationEntry::new(sender));
    }

    pub fn remove(
        &mut self,
        username: &str,
    ) -> Option<oneshot::Sender<Result<SessionData, String>>> {
        self.entries.remove(username).map(|e| e.sender)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.entries.len();
        self.entries.retain(|_, e| !e.is_stale());
        initial - self.entries.len()
    }
}

#[derive(Resource, Default)]
pub struct CharacterCorrelation {
    selection_entries: HashMap<u8, CorrelationEntry<Result<(), String>>>,
    creation_entries: HashMap<u8, CorrelationEntry<Result<CharacterInfo, String>>>,
    deletion_entries: HashMap<u32, CorrelationEntry<Result<(), String>>>,
}

impl CharacterCorrelation {
    pub fn insert_selection(&mut self, slot: u8, sender: oneshot::Sender<Result<(), String>>) {
        self.selection_entries
            .insert(slot, CorrelationEntry::new(sender));
    }

    pub fn remove_selection(&mut self, slot: &u8) -> Option<oneshot::Sender<Result<(), String>>> {
        self.selection_entries.remove(slot).map(|e| e.sender)
    }

    pub fn insert_creation(
        &mut self,
        slot: u8,
        sender: oneshot::Sender<Result<CharacterInfo, String>>,
    ) {
        self.creation_entries
            .insert(slot, CorrelationEntry::new(sender));
    }

    pub fn remove_creation(
        &mut self,
        slot: &u8,
    ) -> Option<oneshot::Sender<Result<CharacterInfo, String>>> {
        self.creation_entries.remove(slot).map(|e| e.sender)
    }

    pub fn insert_deletion(&mut self, char_id: u32, sender: oneshot::Sender<Result<(), String>>) {
        self.deletion_entries
            .insert(char_id, CorrelationEntry::new(sender));
    }

    pub fn remove_deletion(
        &mut self,
        char_id: &u32,
    ) -> Option<oneshot::Sender<Result<(), String>>> {
        self.deletion_entries.remove(char_id).map(|e| e.sender)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.selection_entries.len()
            + self.creation_entries.len()
            + self.deletion_entries.len();
        self.selection_entries.retain(|_, e| !e.is_stale());
        self.creation_entries.retain(|_, e| !e.is_stale());
        self.deletion_entries.retain(|_, e| !e.is_stale());
        let final_count = self.selection_entries.len()
            + self.creation_entries.len()
            + self.deletion_entries.len();
        initial - final_count
    }
}

#[derive(Resource, Default)]
pub struct ServerCorrelation {
    entries: HashMap<usize, CorrelationEntry<Result<(), String>>>,
}

impl ServerCorrelation {
    pub fn insert(&mut self, index: usize, sender: oneshot::Sender<Result<(), String>>) {
        self.entries.insert(index, CorrelationEntry::new(sender));
    }

    pub fn remove(&mut self, index: &usize) -> Option<oneshot::Sender<Result<(), String>>> {
        self.entries.remove(index).map(|e| e.sender)
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.entries.len();
        self.entries.retain(|_, e| !e.is_stale());
        initial - self.entries.len()
    }
}

type CharacterListSender = oneshot::Sender<Result<Vec<CharacterInfoWithJobName>, String>>;

#[derive(Resource, Default)]
pub struct PendingCharacterListSenders {
    senders: Vec<(Instant, CharacterListSender)>,
}

impl PendingCharacterListSenders {
    pub fn push(&mut self, sender: CharacterListSender) {
        self.senders.push((Instant::now(), sender));
    }

    pub fn pop_oldest(&mut self) -> Option<CharacterListSender> {
        if self.senders.is_empty() {
            None
        } else {
            Some(self.senders.remove(0).1)
        }
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.senders.len();
        self.senders
            .retain(|(created, _)| created.elapsed() < CORRELATION_TIMEOUT);
        initial - self.senders.len()
    }
}

type HairstyleSender = oneshot::Sender<Result<Vec<HairstyleInfo>, String>>;

#[derive(Resource, Default)]
pub struct PendingHairstyleSenders {
    senders: Vec<(Instant, HairstyleSender)>,
}

impl PendingHairstyleSenders {
    pub fn push(&mut self, sender: HairstyleSender) {
        self.senders.push((Instant::now(), sender));
    }

    pub fn pop_oldest(&mut self) -> Option<HairstyleSender> {
        if self.senders.is_empty() {
            None
        } else {
            Some(self.senders.remove(0).1)
        }
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.senders.len();
        self.senders
            .retain(|(created, _)| created.elapsed() < CORRELATION_TIMEOUT);
        initial - self.senders.len()
    }
}

type CharacterStatusSender = oneshot::Sender<Result<super::app_bridge::CharacterStatusPayload, String>>;

#[derive(Resource, Default)]
pub struct PendingCharacterStatusSenders {
    senders: Vec<(Instant, CharacterStatusSender)>,
}

impl PendingCharacterStatusSenders {
    pub fn push(&mut self, sender: CharacterStatusSender) {
        self.senders.push((Instant::now(), sender));
    }

    pub fn pop_oldest(&mut self) -> Option<CharacterStatusSender> {
        if self.senders.is_empty() {
            None
        } else {
            Some(self.senders.remove(0).1)
        }
    }

    pub fn cleanup_stale(&mut self) -> usize {
        let initial = self.senders.len();
        self.senders
            .retain(|(created, _)| created.elapsed() < CORRELATION_TIMEOUT);
        initial - self.senders.len()
    }
}

pub fn cleanup_stale_correlations(
    mut login: ResMut<LoginCorrelation>,
    mut character: ResMut<CharacterCorrelation>,
    mut server: ResMut<ServerCorrelation>,
    mut char_list: ResMut<PendingCharacterListSenders>,
    mut hairstyles: ResMut<PendingHairstyleSenders>,
    mut char_status: ResMut<PendingCharacterStatusSenders>,
    mut last_cleanup: Local<Option<Instant>>,
) {
    let now = Instant::now();
    if let Some(last) = *last_cleanup {
        if now.duration_since(last) < Duration::from_secs(10) {
            return;
        }
    }

    let total = login.cleanup_stale()
        + character.cleanup_stale()
        + server.cleanup_stale()
        + char_list.cleanup_stale()
        + hairstyles.cleanup_stale()
        + char_status.cleanup_stale();
    if total > 0 {
        warn!("Cleaned up {} stale correlation entries", total);
    }
    *last_cleanup = Some(now);
}
