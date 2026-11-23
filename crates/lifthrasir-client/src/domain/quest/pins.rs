//! The quests the player keeps on screen.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use net_contract::dto::QuestState;

use super::QuestLog;

/// Quests the tracker shows at once.
pub const MAX_PINS: usize = 3;

/// Why a quest could not be pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinRefused {
    Full,
    NotActive,
}

fn is_active(log: &QuestLog, quest_id: u32) -> bool {
    log.get(quest_id)
        .is_some_and(|entry| entry.state == QuestState::Active)
}

/// The quests pinned to the tracker, in the order they were pinned.
#[derive(Resource, Default, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct QuestPins {
    ids: Vec<u32>,
}

impl QuestPins {
    /// Unpins a pinned quest, or pins an active one while there is room.
    pub fn toggle(&mut self, quest_id: u32, log: &QuestLog) -> Result<(), PinRefused> {
        if let Some(at) = self.ids.iter().position(|id| *id == quest_id) {
            self.ids.remove(at);
            return Ok(());
        }
        if !is_active(log, quest_id) {
            return Err(PinRefused::NotActive);
        }
        if self.ids.len() >= MAX_PINS {
            return Err(PinRefused::Full);
        }
        self.ids.push(quest_id);
        Ok(())
    }

    /// Drops the pins of quests that are gone or no longer active, and any beyond
    /// [`MAX_PINS`] that a hand-edited save file added. Does nothing until the log
    /// has seen the server's list, since an empty log would take every pin.
    pub fn prune(&mut self, log: &QuestLog) {
        if !log.synced() {
            return;
        }
        self.ids.retain(|id| is_active(log, *id));
        self.ids.truncate(MAX_PINS);
    }

    pub fn contains(&self, quest_id: u32) -> bool {
        self.ids.contains(&quest_id)
    }

    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
}
