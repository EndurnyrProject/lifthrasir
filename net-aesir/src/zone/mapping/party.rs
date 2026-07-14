use crate::proto::aesir::net;
use net_contract::dto::{PartyErrorKind, PartyMemberInfo};
use net_contract::events::{
    PartyActionResulted, PartyDisbanded, PartyInfoReceived, PartyInviteNotified, PartyMemberUpdated,
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
        job_id: m.job_id,
        hp: m.hp,
        max_hp: m.max_hp,
        sp: m.sp,
        max_sp: m.max_sp,
        ap: m.ap,
        max_ap: m.max_ap,
    }
}

pub fn party_member_update(u: net::PartyMemberUpdate) -> Option<PartyMemberUpdated> {
    Some(PartyMemberUpdated {
        party_id: u.party_id,
        member: party_member(u.member?),
    })
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
                    job_id: 4001,
                    hp: u32::MAX as u64 + 1,
                    max_hp: u32::MAX as u64 + 2,
                    sp: u32::MAX as u64 + 3,
                    max_sp: u32::MAX as u64 + 4,
                    ap: 5,
                    max_ap: 6,
                },
                net::PartyMember {
                    char_id: 43,
                    name: "Follower".into(),
                    base_level: 50,
                    online: false,
                    map: "geffen".into(),
                    job_id: 7,
                    hp: 8,
                    max_hp: 9,
                    sp: 10,
                    max_sp: 11,
                    ap: 12,
                    max_ap: 13,
                },
            ],
        };

        let received = party_info(info);

        assert_eq!(received.party_id, 7);
        assert_eq!(received.name, "Vikings");
        assert_eq!(received.leader_char_id, 42);
        assert!(received.exp_share);
        assert_eq!(received.members.len(), 2);
        assert_eq!(received.members[0].char_id, 42);
        assert_eq!(received.members[0].name, "Leader");
        assert_eq!(received.members[0].base_level, 99);
        assert!(received.members[0].online);
        assert_eq!(received.members[0].map, "prontera");
        assert_eq!(received.members[0].job_id, 4001);
        assert_eq!(received.members[0].hp, u32::MAX as u64 + 1);
        assert_eq!(received.members[0].max_hp, u32::MAX as u64 + 2);
        assert_eq!(received.members[0].sp, u32::MAX as u64 + 3);
        assert_eq!(received.members[0].max_sp, u32::MAX as u64 + 4);
        assert_eq!(received.members[0].ap, 5);
        assert_eq!(received.members[0].max_ap, 6);
        assert_eq!(received.members[1].char_id, 43);
        assert_eq!(received.members[1].name, "Follower");
        assert_eq!(received.members[1].base_level, 50);
        assert!(!received.members[1].online);
        assert_eq!(received.members[1].map, "geffen");
        assert_eq!(received.members[1].job_id, 7);
        assert_eq!(received.members[1].hp, 8);
        assert_eq!(received.members[1].max_hp, 9);
        assert_eq!(received.members[1].sp, 10);
        assert_eq!(received.members[1].max_sp, 11);
        assert_eq!(received.members[1].ap, 12);
        assert_eq!(received.members[1].max_ap, 13);
    }

    #[test]
    fn party_member_update_maps_complete_snapshot() {
        let updated = party_member_update(net::PartyMemberUpdate {
            party_id: 7,
            member: Some(net::PartyMember {
                char_id: 42,
                name: "Leader".into(),
                base_level: 99,
                online: true,
                map: "prontera".into(),
                job_id: 4001,
                hp: u32::MAX as u64 + 1,
                max_hp: u32::MAX as u64 + 2,
                sp: u32::MAX as u64 + 3,
                max_sp: u32::MAX as u64 + 4,
                ap: 5,
                max_ap: 6,
            }),
        })
        .expect("member snapshot should map");

        assert_eq!(updated.party_id, 7);
        assert_eq!(updated.member.char_id, 42);
        assert_eq!(updated.member.name, "Leader");
        assert_eq!(updated.member.base_level, 99);
        assert!(updated.member.online);
        assert_eq!(updated.member.map, "prontera");
        assert_eq!(updated.member.job_id, 4001);
        assert_eq!(updated.member.hp, u32::MAX as u64 + 1);
        assert_eq!(updated.member.max_hp, u32::MAX as u64 + 2);
        assert_eq!(updated.member.sp, u32::MAX as u64 + 3);
        assert_eq!(updated.member.max_sp, u32::MAX as u64 + 4);
        assert_eq!(updated.member.ap, 5);
        assert_eq!(updated.member.max_ap, 6);
    }
}
