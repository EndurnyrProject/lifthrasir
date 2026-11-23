//! The quest log: what the player has accepted, how far each hunt has come, and
//! who gave it out.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestState {
    /// Shelved: kept in the log, and its hunts still count.
    Inactive,
    Active,
    Complete,
}

/// The NPC whose script set a quest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGiver {
    pub name: String,
    pub map: String,
    pub x: u16,
    pub y: u16,
}

/// What else a kill must satisfy to count. A `0` bound or an empty text means the
/// filter is absent; the texts keep the spelling of the server's quest database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HuntFilter {
    pub min_level: u32,
    pub max_level: u32,
    pub race: String,
    pub size: String,
    pub element: String,
    pub map: String,
}

/// One hunt target. `mob_id` is `0` for a target that takes any monster passing
/// `filter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestObjective {
    pub mob_id: u32,
    pub mob_name: String,
    pub needed: u32,
    pub current: u32,
    pub filter: HuntFilter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestEntry {
    pub quest_id: u32,
    pub state: QuestState,
    /// The quest database title; empty when its definition no longer resolves.
    pub title: String,
    pub objectives: Vec<QuestObjective>,
    /// Unix seconds the quest expires; `None` for no time limit.
    pub deadline: Option<u64>,
    /// `None` when the quest was set outside an NPC script.
    pub giver: Option<QuestGiver>,
}
