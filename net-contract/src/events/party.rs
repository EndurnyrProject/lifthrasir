use crate::dto::{PartyErrorKind, PartyMemberInfo};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Full party roster, replacing any prior `PartyState`.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyInfoReceived {
    pub party_id: u32,
    pub name: String,
    pub leader_char_id: u32,
    pub exp_share: bool,
    pub members: Vec<PartyMemberInfo>,
}

/// An incoming invite to join a party.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyInviteNotified {
    pub party_id: u32,
    pub party_name: String,
    pub inviter_name: String,
}

/// Result of an outbound party action (create, invite, respond, leave).
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyActionResulted {
    pub action: String,
    pub success: bool,
    pub error: PartyErrorKind,
}

/// The local player's party was disbanded.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PartyDisbanded {
    pub party_id: u32,
    pub reason: String,
}
