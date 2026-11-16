//! Ordered chat room ingress from the active adapter.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::dto::{ChatRoomJoinError, ChatRoomMember, ChatRoomSign};

/// One chat room payload. A single stream keeps the order the server sent them in,
/// which matters when a room changes owner: its sign is removed and shown again.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub enum ChatRoomEvent {
    /// A room appeared near the player or its sign changed.
    SignShown(ChatRoomSign),
    SignRemoved {
        room_id: u32,
    },
    /// A unit left view. Any sign it carried goes with it; the server shows the sign
    /// again when the unit returns.
    UnitLeft {
        gid: u32,
    },
    /// The local player's own room was opened; its sign follows.
    Created,
    CreateRefused,
    /// The local player entered a room and received its roster.
    Joined {
        room_id: u32,
        owner_gid: u32,
        members: Vec<ChatRoomMember>,
    },
    JoinRefused {
        room_id: u32,
        reason: ChatRoomJoinError,
    },
    MemberJoined {
        room_id: u32,
        member: ChatRoomMember,
    },
    /// A member left. The local player receives their own only when `kicked`.
    MemberLeft {
        room_id: u32,
        member: ChatRoomMember,
        kicked: bool,
    },
    /// `char_id` owns the room now.
    OwnerChanged {
        room_id: u32,
        char_id: u32,
    },
    Said {
        room_id: u32,
        char_id: u32,
        name: String,
        text: String,
    },
    /// The player is changing maps, which leaves every room behind.
    MapChanging,
}
