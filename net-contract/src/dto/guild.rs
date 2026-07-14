//! Protocol-neutral guild types.

/// Complete authoritative guild snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInfo {
    pub guild_id: u32,
    pub name: String,
    pub master_char_id: u32,
    pub emblem_id: u32,
    pub notice_subject: String,
    pub notice_body: String,
    pub positions: Vec<GuildPositionInfo>,
    pub members: Vec<GuildMemberInfo>,
}

/// One fixed guild position slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildPositionInfo {
    pub index: u32,
    pub name: String,
    pub can_invite: bool,
    pub can_expel: bool,
    pub can_storage: bool,
    pub tax: u32,
}

/// One complete guild roster entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMemberInfo {
    pub char_id: u32,
    pub name: String,
    pub job_id: u32,
    pub base_level: u32,
    pub online: bool,
    pub map: String,
    pub position_index: u32,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub ap: u32,
    pub max_ap: u32,
}

/// An incoming guild invitation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInviteInfo {
    pub guild_id: u32,
    pub guild_name: String,
    pub inviter_name: String,
}

/// Result of an outbound guild action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildActionResult {
    pub action: String,
    pub success: bool,
    pub error: GuildErrorKind,
}

/// Protocol-neutral guild operation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuildErrorKind {
    #[default]
    None,
    NameTaken,
    AlreadyInGuild,
    GuildFull,
    NoPermission,
    NotMember,
    TargetOffline,
    NoEmperium,
    InvalidEmblem,
    CannotTargetMaster,
    InvalidPosition,
    Unknown(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_error_preserves_its_wire_value() {
        assert_eq!(GuildErrorKind::Unknown(77), GuildErrorKind::Unknown(77));
        assert_ne!(GuildErrorKind::Unknown(77), GuildErrorKind::None);
    }
}
