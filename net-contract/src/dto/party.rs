//! Protocol-neutral party types.

/// One roster entry in a `PartyInfoReceived` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberInfo {
    pub char_id: u32,
    pub name: String,
    pub base_level: u32,
    pub online: bool,
    pub map: String,
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
        };

        assert_eq!(member.char_id, 1);
    }
}
