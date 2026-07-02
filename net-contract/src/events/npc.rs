use crate::dto::NpcDialogExpect;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// One frame of an NPC dialogue; `options` is populated only when `expect == Menu`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct NpcDialogReceived {
    pub npc_id: u32,
    pub text: String,
    pub expect: NpcDialogExpect,
    pub options: Vec<String>,
}
