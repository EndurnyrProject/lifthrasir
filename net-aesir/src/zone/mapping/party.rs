use crate::proto::aesir::net;
use net_contract::dto::{PartyErrorKind, PartyMemberInfo};
use net_contract::events::{
    PartyActionResulted, PartyDisbanded, PartyInfoReceived, PartyInviteNotified,
};

pub fn party_info(p: net::PartyInfo) -> PartyInfoReceived {
    PartyInfoReceived {
        party_id: p.party_id,
        name: p.name,
        leader_char_id: p.leader_char_id,
        exp_share: p.exp_share,
        members: p.members.into_iter().map(party_member).collect(),
    }
}

fn party_member(m: net::PartyMember) -> PartyMemberInfo {
    PartyMemberInfo {
        char_id: m.char_id,
        name: m.name,
        base_level: m.base_level,
        online: m.online,
        map: m.map,
    }
}

pub fn party_invite_notify(n: net::PartyInviteNotify) -> PartyInviteNotified {
    PartyInviteNotified {
        party_id: n.party_id,
        party_name: n.party_name,
        inviter_name: n.inviter_name,
    }
}

pub fn party_action_result(r: net::PartyActionResult) -> PartyActionResulted {
    PartyActionResulted {
        action: r.action,
        success: r.success,
        error: party_error(r.error),
    }
}

pub fn party_disbanded(d: net::PartyDisbanded) -> PartyDisbanded {
    PartyDisbanded {
        party_id: d.party_id,
        reason: d.reason,
    }
}

fn party_error(v: i32) -> PartyErrorKind {
    match net::PartyError::try_from(v) {
        Ok(net::PartyError::None) => PartyErrorKind::None,
        Ok(net::PartyError::NameTaken) => PartyErrorKind::NameTaken,
        Ok(net::PartyError::AlreadyInParty) => PartyErrorKind::AlreadyInParty,
        Ok(net::PartyError::PartyFull) => PartyErrorKind::PartyFull,
        Ok(net::PartyError::NotLeader) => PartyErrorKind::NotLeader,
        Ok(net::PartyError::LevelRange) => PartyErrorKind::LevelRange,
        Ok(net::PartyError::SameAccount) => PartyErrorKind::SameAccount,
        Ok(net::PartyError::TargetOffline) => PartyErrorKind::TargetOffline,
        Ok(net::PartyError::NotMember) => PartyErrorKind::NotMember,
        Ok(net::PartyError::NotSameMap) => PartyErrorKind::NotSameMap,
        Err(_) => PartyErrorKind::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_error_maps_each_known_variant() {
        assert_eq!(
            party_error(net::PartyError::None as i32),
            PartyErrorKind::None
        );
        assert_eq!(
            party_error(net::PartyError::NameTaken as i32),
            PartyErrorKind::NameTaken
        );
        assert_eq!(
            party_error(net::PartyError::AlreadyInParty as i32),
            PartyErrorKind::AlreadyInParty
        );
        assert_eq!(
            party_error(net::PartyError::PartyFull as i32),
            PartyErrorKind::PartyFull
        );
        assert_eq!(
            party_error(net::PartyError::NotLeader as i32),
            PartyErrorKind::NotLeader
        );
        assert_eq!(
            party_error(net::PartyError::LevelRange as i32),
            PartyErrorKind::LevelRange
        );
        assert_eq!(
            party_error(net::PartyError::SameAccount as i32),
            PartyErrorKind::SameAccount
        );
        assert_eq!(
            party_error(net::PartyError::TargetOffline as i32),
            PartyErrorKind::TargetOffline
        );
        assert_eq!(
            party_error(net::PartyError::NotMember as i32),
            PartyErrorKind::NotMember
        );
        assert_eq!(
            party_error(net::PartyError::NotSameMap as i32),
            PartyErrorKind::NotSameMap
        );
    }

    #[test]
    fn party_error_out_of_range_falls_back_to_none() {
        assert_eq!(party_error(999), PartyErrorKind::None);
    }

    #[test]
    fn party_info_maps_members_and_fields() {
        let info = net::PartyInfo {
            party_id: 7,
            name: "Vikings".into(),
            leader_char_id: 42,
            exp_share: true,
            members: vec![
                net::PartyMember {
                    char_id: 42,
                    name: "Leader".into(),
                    base_level: 99,
                    online: true,
                    map: "prontera".into(),
                },
                net::PartyMember {
                    char_id: 43,
                    name: "Follower".into(),
                    base_level: 50,
                    online: false,
                    map: "geffen".into(),
                },
            ],
        };

        let received = party_info(info);

        assert_eq!(received.party_id, 7);
        assert_eq!(received.leader_char_id, 42);
        assert!(received.exp_share);
        assert_eq!(received.members.len(), 2);
        assert_eq!(received.members[0].char_id, 42);
        assert_eq!(received.members[0].name, "Leader");
        assert_eq!(received.members[0].base_level, 99);
        assert!(received.members[0].online);
        assert_eq!(received.members[0].map, "prontera");
        assert_eq!(received.members[1].char_id, 43);
        assert_eq!(received.members[1].name, "Follower");
        assert_eq!(received.members[1].base_level, 50);
        assert!(!received.members[1].online);
        assert_eq!(received.members[1].map, "geffen");
    }
}
