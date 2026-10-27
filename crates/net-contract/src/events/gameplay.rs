//! Item-granted casts and server-authoritative gameplay feedback.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::dto::{GameplayNoticeKind, ItemAddFailure, ItemSkillTargetKind};

/// One pending item-granted cast, not a learned skill.
#[derive(Message, Debug, Clone, Copy)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ItemSkillOffered {
    pub skill_id: u32,
    pub level: u32,
    pub target_kind: ItemSkillTargetKind,
}

#[derive(Message, Debug, Clone, Copy)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct InventoryAddRejected {
    pub reason: ItemAddFailure,
}

#[derive(Message, Debug, Clone, Copy)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct GameplayNoticeReceived {
    pub kind: GameplayNoticeKind,
}
