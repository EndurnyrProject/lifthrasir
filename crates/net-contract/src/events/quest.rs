//! Ordered quest log ingress from the active adapter.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::dto::{QuestEntry, QuestState};

/// One quest log payload. A single stream keeps the order the server sent them in,
/// which matters when a quest is replaced: the old one is removed, then the new
/// one added.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub enum QuestEvent {
    /// The whole log, sent on map load.
    List(Vec<QuestEntry>),
    Added(QuestEntry),
    Removed {
        quest_id: u32,
    },
    StateChanged {
        quest_id: u32,
        state: QuestState,
    },
    /// `count` and `needed` are the new current value and the cap.
    HuntProgress {
        quest_id: u32,
        objective_index: usize,
        count: u32,
        needed: u32,
    },
}
