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

/// Server -> client, show a `progressbar` over the local player's head that fills
/// over `seconds`. `color` is `0xRRGGBB` (rendered verbatim; `0` is a black bar).
/// `npc_id` owns the interaction, so the client can address the Progress/Cancel
/// ack to the exact NPC even when no dialogue window is open.
#[derive(Message, Debug, Clone, Copy)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct ProgressBarStarted {
    pub seconds: u32,
    pub color: u32,
    pub npc_id: u32,
}
