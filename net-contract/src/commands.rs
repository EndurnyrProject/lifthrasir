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
