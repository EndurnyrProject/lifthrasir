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
