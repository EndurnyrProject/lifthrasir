//! Protocol-neutral party types.

/// One roster entry in a `PartyInfoReceived` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberInfo {
    pub char_id: u32,
    pub name: String,
    pub base_level: u32,
    pub online: bool,
    pub map: String,
    pub job_id: u32,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub ap: u32,
    pub max_ap: u32,
}

/// Mirrors the proto `PartyError` enum; `None` means no error / not applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PartyErrorKind {
    #[default]
    None,
    NameTaken,
    AlreadyInParty,
    PartyFull,
    NotLeader,
    LevelRange,
    SameAccount,
    TargetOffline,
    NotMember,
    NotSameMap,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_error_kind_defaults_to_none() {
        assert_eq!(PartyErrorKind::default(), PartyErrorKind::None);
    }

    #[test]
    fn party_member_info_constructs() {
        let member = PartyMemberInfo {
            char_id: 1,
            name: "Test".into(),
            base_level: 99,
            online: true,
            map: "prontera".into(),
            job_id: 7,
            hp: u32::MAX as u64 + 1,
            max_hp: u32::MAX as u64 + 2,
            sp: u32::MAX as u64 + 3,
            max_sp: u32::MAX as u64 + 4,
            ap: 5,
            max_ap: 6,
        };

        assert_eq!(member.char_id, 1);
        assert_eq!(member.name, "Test");
        assert_eq!(member.base_level, 99);
        assert!(member.online);
        assert_eq!(member.map, "prontera");
        assert_eq!(member.job_id, 7);
        assert_eq!(member.hp, u32::MAX as u64 + 1);
        assert_eq!(member.max_hp, u32::MAX as u64 + 2);
        assert_eq!(member.sp, u32::MAX as u64 + 3);
        assert_eq!(member.max_sp, u32::MAX as u64 + 4);
        assert_eq!(member.ap, 5);
        assert_eq!(member.max_ap, 6);
    }
}
