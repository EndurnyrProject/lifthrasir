use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Server-driven quest mark for an NPC.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub enum QuestMarkChanged {
    /// Show or replace the NPC's mark at the given cell.
    Show {
        npc_id: u32,
        x: u16,
        y: u16,
        icon: u32,
        color: u32,
    },
    /// Remove the NPC's mark.
    Clear { npc_id: u32 },
}
