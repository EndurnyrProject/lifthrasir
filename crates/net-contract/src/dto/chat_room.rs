//! Chat rooms: the signs NPCs and players open over their heads to gather people.

/// A chat room as its sign shows it to everyone near the owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatRoomSign {
    pub room_id: u32,
    pub title: String,
    /// Players in the room; an owner NPC is not one of them.
    pub member_count: u32,
    /// Players the room holds, the owning player included.
    pub seats: u32,
    /// `false` when joining needs the password.
    pub public: bool,
    /// The unit the sign hangs over: an NPC or the owning player.
    pub owner_gid: u32,
    pub owner_is_npc: bool,
}

impl ChatRoomSign {
    pub fn is_full(&self) -> bool {
        self.member_count >= self.seats
    }
}

/// A player in a chat room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRoomMember {
    pub char_id: u32,
    pub name: String,
}

/// What the owner sets when opening or editing a room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRoomSettings {
    pub title: String,
    /// Ignored while `public`.
    pub password: String,
    /// Players the room holds, the owner included.
    pub seats: u32,
    pub public: bool,
}

/// Why the server refused a join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRoomJoinError {
    Full,
    WrongPassword,
    LevelTooLow,
    LevelTooHigh,
    NotEnoughZeny,
    /// The owner removed this player from the room earlier.
    Kicked,
    Unknown(u32),
}
