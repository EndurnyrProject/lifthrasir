//! The quest log: a mirror of what the server says the player has accepted.
//!
//! The server is the authority. [`QuestLog`] only replays [`QuestEvent`], and the
//! tracker pins ([`QuestPins`]) follow whatever the log still holds.

mod persistence;
mod pins;

use std::collections::BTreeMap;

use bevy::prelude::*;
use net_contract::dto::{QuestEntry, QuestState};
use net_contract::events::QuestEvent;

pub use pins::{MAX_PINS, PinRefused, QuestPins};

use crate::core::state::GameState;

/// Every quest the server has told the client about, by quest id.
#[derive(Resource, Default, Debug)]
pub struct QuestLog {
    quests: BTreeMap<u32, QuestEntry>,
    synced: bool,
}

/// One hunt target reached its count; `index` is its position in the quest's objectives.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveCompleted {
    pub quest_id: u32,
    pub index: usize,
}

impl QuestLog {
    pub fn get(&self, quest_id: u32) -> Option<&QuestEntry> {
        self.quests.get(&quest_id)
    }

    /// Whether the server's list has arrived. Until it has, an empty log means
    /// "not told yet", not "no quests".
    pub fn synced(&self) -> bool {
        self.synced
    }

    /// The quests in quest id order.
    pub fn iter(&self) -> impl Iterator<Item = &QuestEntry> {
        self.quests.values()
    }

    /// Replays one server event. Returns the objective the event just completed, if any.
    pub fn apply(&mut self, event: &QuestEvent) -> Option<ObjectiveCompleted> {
        match event {
            QuestEvent::List(entries) => {
                self.quests = entries
                    .iter()
                    .map(|entry| (entry.quest_id, entry.clone()))
                    .collect();
                self.synced = true;
            }
            QuestEvent::Added(entry) => {
                self.quests.insert(entry.quest_id, entry.clone());
            }
            QuestEvent::Removed { quest_id } => {
                self.quests.remove(quest_id);
            }
            QuestEvent::StateChanged { quest_id, state } => match self.quests.get_mut(quest_id) {
                Some(entry) => entry.state = *state,
                None => warn!("quest {quest_id}: state change for a quest not in the log"),
            },
            QuestEvent::HuntProgress {
                quest_id,
                objective_index,
                count,
                needed,
            } => return self.progress(*quest_id, *objective_index, *count, *needed),
        }
        None
    }

    fn progress(
        &mut self,
        quest_id: u32,
        index: usize,
        count: u32,
        needed: u32,
    ) -> Option<ObjectiveCompleted> {
        let Some(objective) = self
            .quests
            .get_mut(&quest_id)
            .and_then(|entry| entry.objectives.get_mut(index))
        else {
            warn!("quest {quest_id}: hunt progress for unknown objective {index}");
            return None;
        };
        let was_met = objective.current >= objective.needed;
        objective.current = count;
        objective.needed = needed;
        (!was_met && count >= needed).then_some(ObjectiveCompleted { quest_id, index })
    }
}

/// Whether the quest can be turned in: active, with hunts, every one of them met.
pub fn is_ready(entry: &QuestEntry) -> bool {
    entry.state == QuestState::Active
        && !entry.objectives.is_empty()
        && entry
            .objectives
            .iter()
            .all(|objective| objective.current >= objective.needed)
}

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestLog>()
            .init_resource::<QuestPins>()
            .init_resource::<persistence::PinsPersistState>()
            .add_message::<ObjectiveCompleted>()
            .add_systems(
                Update,
                (
                    replay_quests,
                    persistence::load_quest_pins.run_if(in_state(GameState::InGame)),
                    persistence::persist_quest_pins.run_if(in_state(GameState::InGame)),
                )
                    .chain(),
            )
            .add_systems(
                OnExit(GameState::InGame),
                persistence::save_quest_pins_on_exit,
            )
            .add_systems(OnEnter(GameState::CharacterSelection), reset_quests);
    }
}

/// Replays the server's events, announces the hunts they finished, then drops the
/// pins of quests the log no longer holds as active.
fn replay_quests(
    mut events: MessageReader<QuestEvent>,
    mut log: ResMut<QuestLog>,
    mut pins: ResMut<QuestPins>,
    mut completed: MessageWriter<ObjectiveCompleted>,
) {
    if events.is_empty() {
        return;
    }
    for event in events.read() {
        if let Some(done) = log.apply(event) {
            completed.write(done);
        }
    }
    pins.prune(&log);
}

fn reset_quests(mut log: ResMut<QuestLog>, mut pins: ResMut<QuestPins>) {
    *log = QuestLog::default();
    *pins = QuestPins::default();
}

#[cfg(test)]
mod tests;
