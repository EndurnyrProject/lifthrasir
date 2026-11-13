//! Protocol-neutral guild types.

/// Complete authoritative guild snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildInfo {
    pub guild_id: u32,
    pub name: String,
    pub master_char_id: u32,
    pub emblem_id: u32,
    pub notice_subject: String,
    pub notice_body: String,
    pub positions: Vec<GuildPositionInfo>,
    pub members: Vec<GuildMemberInfo>,
    pub level: u32,
    pub exp: u64,
    pub next_exp: u64,
    pub skill_points: u32,
    pub skills: Vec<GuildSkillInfo>,
    pub relations: Vec<GuildRelationInfo>,
    /// Member capacity.
    pub max_members: u32,
    /// Name of the member who last edited the notice; empty when never edited.
    pub notice_author: String,
    /// Unix seconds of the last notice edit; `0` when never edited.
    pub notice_posted_at: u64,
    /// Display names of the castles the guild holds.
    pub castles: Vec<String>,
    /// Most recent activity, newest first.
    pub activity: Vec<GuildActivityInfo>,
}

/// One learned or learnable guild skill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildSkillInfo {
    pub skill_id: u32,
    pub level: u32,
    pub max_level: u32,
}

/// One authoritative guild relation. Everything past `kind` describes the
/// other guild as it was when the snapshot was built.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildRelationInfo {
    pub guild_id: u32,
    pub name: String,
    pub kind: GuildRelationKind,
    pub level: u32,
    pub member_count: u32,
    pub master_name: String,
    /// The other guild's emblem version; `0` means it has no emblem.
    pub emblem_id: u32,
    pub castles: Vec<String>,
}

/// Protocol-neutral guild relation kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GuildRelationKind {
    #[default]
    Ally,
    Antagonist,
    Unknown(i32),
}

/// One entry of a guild's activity log. Each kind fills only the fields it
/// documents; the rest stay at their defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildActivityInfo {
    pub kind: GuildActivityKind,
    /// Who or what it happened to: a member, another guild or a castle.
    pub subject: String,
    /// The member who did it; empty when nobody did.
    pub actor: String,
    /// Free text the kind needs (the new position's name).
    pub detail: String,
    pub skill_id: u32,
    pub level: u32,
    /// Unix seconds the entry was recorded.
    pub at: u64,
}

/// What a [`GuildActivityInfo`] records.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GuildActivityKind {
    /// The guild was founded by `actor`.
    #[default]
    Created,
    /// `subject` joined, invited by `actor`.
    Joined,
    /// `subject` left on their own.
    Left,
    /// `subject` was expelled by `actor`.
    Expelled,
    /// `subject` was moved to the position named `detail`.
    PositionChanged,
    /// `actor` raised guild skill `skill_id` to `level`.
    SkillLearned,
    /// The guild reached guild level `level`.
    LevelUp,
    /// An alliance with the guild `subject` was formed.
    AllianceFormed,
    /// The alliance with the guild `subject` ended.
    AllianceBroken,
    /// The guild `subject` was declared an antagonist.
    AntagonistDeclared,
    /// The guild `subject` is no longer an antagonist.
    AntagonistRemoved,
    /// The guild took the castle `subject`.
    CastleTaken,
    /// The guild lost the castle `subject`.
    CastleLost,
    Unknown(i32),
}

/// One incoming alliance request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildAllianceInviteInfo {
    pub guild_id: u32,
    pub guild_name: String,
    pub requester_name: String,
}

/// One fixed guild position slot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildPositionInfo {
    pub index: u32,
    pub name: String,
    pub can_invite: bool,
    pub can_expel: bool,
    pub can_storage: bool,
    pub tax: u32,
    /// May edit the guild notice.
    pub can_notice: bool,
    /// May manage alliances and antagonists.
    pub can_relations: bool,
}

/// One complete guild roster entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// Base EXP this member has paid to the guild through the position tax.
    pub exp: u64,
    /// Unix seconds of the member's last login; `0` when unknown. Meaningful
    /// only while `online` is false.
    pub last_seen: u64,
}

/// An incoming guild invitation, with the guild as its members see it so the
/// invitee can judge it before answering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildInviteInfo {
    pub guild_id: u32,
    pub guild_name: String,
    pub inviter_name: String,
    /// One of `guild.members`.
    pub inviter_char_id: u32,
    pub guild: GuildInfo,
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
    NoSkillPoints,
    SkillRequirement,
    SkillMaxed,
    AllyLimit,
    AntagonistLimit,
    AlreadyAllied,
    AlreadyAntagonist,
    NotRelated,
    SameGuild,
    SiegeActive,
    RequestPending,
    AllianceDeclined,
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
