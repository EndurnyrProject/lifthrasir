//! Outbound command Messages (client to server).

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Request to move the local player to a destination cell.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct MoveRequested {
    pub dest_x: u16,
    pub dest_y: u16,
}

/// Request to equip the inventory item at `index` to its worn `location` mask.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct EquipRequested {
    pub index: u16,
    pub location: u32,
}

/// Request to unequip the inventory item at `index`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UnequipRequested {
    pub index: u16,
}

/// Request to use (consume) the inventory item at `index`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct UseRequested {
    pub index: u32,
}

/// Request to send a chat line; `message` is the wire-ready, formatted string.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ChatSent {
    pub message: String,
}

/// Request to cast a single-target skill (`skill_id` at `level`) at `target_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SkillCastRequested {
    pub skill_id: u32,
    pub level: u32,
    pub target_id: u32,
}

/// Request to cast a ground-targeted skill (`skill_id` at `level`) on cell (`x`, `y`).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct GroundSkillCastRequested {
    pub skill_id: u32,
    pub level: u32,
    pub x: u32,
    pub y: u32,
}

/// Request a basic attack against the entity identified by `target_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct AttackRequested {
    pub target_id: u32,
}

/// Request to sit (`sit == true`) or stand (`sit == false`) the local player.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct SitToggled {
    pub sit: bool,
}

/// Request to allocate `amount` points into the stat identified by `stat_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StatRaiseRequested {
    pub stat_id: u32,
    pub amount: u32,
}

/// Request to learn (raise a level of) the skill identified by `skill_id`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct LearnSkillRequested {
    pub skill_id: u32,
}
