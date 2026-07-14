//! Ordered guild ingress from the active network adapter.

use crate::{
    dto::{GuildActionResult, GuildInfo, GuildInviteInfo, GuildMemberInfo},
    state::ZoneSessionGeneration,
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// One guild payload stamped with the zone session that received it.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct GuildIngress {
    pub generation: ZoneSessionGeneration,
    pub payload: GuildIngressPayload,
}

/// Guild payloads share one message stream so their wire order is preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildIngressPayload {
    ActionResult(GuildActionResult),
    InviteNotified(GuildInviteInfo),
    Info(GuildInfo),
    MemberUpdated {
        guild_id: u32,
        member: GuildMemberInfo,
    },
    EmblemChanged {
        guild_id: u32,
        emblem_id: u32,
    },
    EmblemData {
        guild_id: u32,
        emblem_id: u32,
        data: Vec<u8>,
    },
    Disbanded {
        guild_id: u32,
        reason: String,
    },
}
