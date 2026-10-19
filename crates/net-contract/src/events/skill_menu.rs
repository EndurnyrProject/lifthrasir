use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// What the ids in a [`SkillMenuOffered`] name, and therefore how the client
/// resolves them to labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMenuKind {
    /// Skill ids (e.g. the spells `SA_AUTOSPELL` offers).
    Skills,
    /// Item ids (e.g. the products a forge or brew recipe yields).
    Items,
    /// Inventory slot indices (e.g. the items `MC_IDENTIFY` can identify).
    InventorySlots,
}

/// The server offers a list of choices on behalf of a skill; the client renders
/// them and answers with exactly one [`AnswerSkillMenu`](crate::commands::AnswerSkillMenu).
/// A newer offer replaces any older one — the server parks a single pending menu.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillMenuOffered {
    pub src_skill_id: u32,
    pub kind: SkillMenuKind,
    pub entry_ids: Vec<u32>,
}
