use crate::envelope::Body;
use crate::proto::aesir::net;
use net_contract::dto::{
    GuildActionResult, GuildErrorKind, GuildInfo, GuildInviteInfo, GuildMemberInfo,
    GuildPositionInfo,
};
use net_contract::events::GuildIngressPayload;

fn guild_error(value: i32) -> GuildErrorKind {
    match net::GuildError::try_from(value) {
        Ok(net::GuildError::GuildErrNone) => GuildErrorKind::None,
        Ok(net::GuildError::GuildErrNameTaken) => GuildErrorKind::NameTaken,
        Ok(net::GuildError::GuildErrAlreadyInGuild) => GuildErrorKind::AlreadyInGuild,
        Ok(net::GuildError::GuildErrGuildFull) => GuildErrorKind::GuildFull,
        Ok(net::GuildError::GuildErrNoPermission) => GuildErrorKind::NoPermission,
        Ok(net::GuildError::GuildErrNotMember) => GuildErrorKind::NotMember,
        Ok(net::GuildError::GuildErrTargetOffline) => GuildErrorKind::TargetOffline,
        Ok(net::GuildError::GuildErrNoEmperium) => GuildErrorKind::NoEmperium,
        Ok(net::GuildError::GuildErrInvalidEmblem) => GuildErrorKind::InvalidEmblem,
        Ok(net::GuildError::GuildErrCannotTargetMaster) => GuildErrorKind::CannotTargetMaster,
        Ok(net::GuildError::GuildErrInvalidPosition) => GuildErrorKind::InvalidPosition,
        Err(_) => GuildErrorKind::Unknown(value),
    }
}

fn guild_position(position: net::GuildPosition) -> GuildPositionInfo {
    GuildPositionInfo {
        index: position.index,
        name: position.name,
        can_invite: position.can_invite,
        can_expel: position.can_expel,
        can_storage: position.can_storage,
        tax: position.tax,
    }
}

fn guild_member(member: net::GuildMember) -> GuildMemberInfo {
    GuildMemberInfo {
        char_id: member.char_id,
        name: member.name,
        job_id: member.job_id,
        base_level: member.base_level,
        online: member.online,
        map: member.map,
        position_index: member.position_index,
        hp: member.hp,
        max_hp: member.max_hp,
        sp: member.sp,
        max_sp: member.max_sp,
        ap: member.ap,
        max_ap: member.max_ap,
    }
}

fn guild_info(info: net::GuildInfo) -> GuildInfo {
    GuildInfo {
        guild_id: info.guild_id,
        name: info.name,
        master_char_id: info.master_char_id,
        emblem_id: info.emblem_id,
        notice_subject: info.notice_subject,
        notice_body: info.notice_body,
        positions: info.positions.into_iter().map(guild_position).collect(),
        members: info.members.into_iter().map(guild_member).collect(),
    }
}

pub(crate) fn guild_scope_id(body: &Body) -> Option<u32> {
    match body {
        Body::GuildInviteNotify(invite) => Some(invite.guild_id),
        Body::GuildInfo(info) => Some(info.guild_id),
        Body::GuildMemberUpdate(update) => Some(update.guild_id),
        Body::GuildEmblemChanged(emblem) => Some(emblem.guild_id),
        Body::GuildEmblemData(emblem) => Some(emblem.guild_id),
        Body::GuildDisbanded(disbanded) => Some(disbanded.guild_id),
        _ => None,
    }
}

pub(crate) fn guild_payload(body: Body) -> Option<GuildIngressPayload> {
    if guild_scope_id(&body) == Some(0) {
        return None;
    }
    match body {
        Body::GuildActionResult(result) => {
            Some(GuildIngressPayload::ActionResult(GuildActionResult {
                action: result.action,
                success: result.success,
                error: guild_error(result.error),
            }))
        }
        Body::GuildInviteNotify(invite) => {
            Some(GuildIngressPayload::InviteNotified(GuildInviteInfo {
                guild_id: invite.guild_id,
                guild_name: invite.guild_name,
                inviter_name: invite.inviter_name,
            }))
        }
        Body::GuildInfo(info) => Some(GuildIngressPayload::Info(guild_info(info))),
        Body::GuildMemberUpdate(update) => {
            update
                .member
                .map(guild_member)
                .map(|member| GuildIngressPayload::MemberUpdated {
                    guild_id: update.guild_id,
                    member,
                })
        }
        Body::GuildEmblemChanged(emblem) => Some(GuildIngressPayload::EmblemChanged {
            guild_id: emblem.guild_id,
            emblem_id: emblem.emblem_id,
        }),
        Body::GuildEmblemData(emblem) => Some(GuildIngressPayload::EmblemData {
            guild_id: emblem.guild_id,
            emblem_id: emblem.emblem_id,
            data: emblem.data,
        }),
        Body::GuildDisbanded(disbanded) => Some(GuildIngressPayload::Disbanded {
            guild_id: disbanded.guild_id,
            reason: disbanded.reason,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_error_maps_every_known_variant_and_preserves_unknown_values() {
        let known = [
            (net::GuildError::GuildErrNone, GuildErrorKind::None),
            (
                net::GuildError::GuildErrNameTaken,
                GuildErrorKind::NameTaken,
            ),
            (
                net::GuildError::GuildErrAlreadyInGuild,
                GuildErrorKind::AlreadyInGuild,
            ),
            (
                net::GuildError::GuildErrGuildFull,
                GuildErrorKind::GuildFull,
            ),
            (
                net::GuildError::GuildErrNoPermission,
                GuildErrorKind::NoPermission,
            ),
            (
                net::GuildError::GuildErrNotMember,
                GuildErrorKind::NotMember,
            ),
            (
                net::GuildError::GuildErrTargetOffline,
                GuildErrorKind::TargetOffline,
            ),
            (
                net::GuildError::GuildErrNoEmperium,
                GuildErrorKind::NoEmperium,
            ),
            (
                net::GuildError::GuildErrInvalidEmblem,
                GuildErrorKind::InvalidEmblem,
            ),
            (
                net::GuildError::GuildErrCannotTargetMaster,
                GuildErrorKind::CannotTargetMaster,
            ),
            (
                net::GuildError::GuildErrInvalidPosition,
                GuildErrorKind::InvalidPosition,
            ),
        ];

        for (wire, expected) in known {
            assert_eq!(guild_error(wire as i32), expected);
        }
        assert_eq!(guild_error(77), GuildErrorKind::Unknown(77));
    }

    #[test]
    fn guild_info_maps_the_complete_authoritative_snapshot() {
        let payload = guild_payload(Body::GuildInfo(net::GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 3,
            notice_subject: "Welcome".into(),
            notice_body: "Be kind".into(),
            positions: vec![net::GuildPosition {
                index: 1,
                name: "Officer".into(),
                can_invite: true,
                can_expel: true,
                can_storage: false,
                tax: 12,
            }],
            members: vec![net::GuildMember {
                char_id: 42,
                name: "Odin".into(),
                job_id: 4001,
                base_level: 99,
                online: true,
                map: "prontera".into(),
                position_index: 1,
                hp: u32::MAX as u64 + 1,
                max_hp: u32::MAX as u64 + 2,
                sp: u32::MAX as u64 + 3,
                max_sp: u32::MAX as u64 + 4,
                ap: 5,
                max_ap: 6,
            }],
        }))
        .expect("guild info should map");

        let GuildIngressPayload::Info(info) = payload else {
            panic!("expected guild info payload");
        };
        assert_eq!(info.guild_id, 7);
        assert_eq!(info.name, "Vikings");
        assert_eq!(info.master_char_id, 42);
        assert_eq!(info.emblem_id, 3);
        assert_eq!(info.notice_subject, "Welcome");
        assert_eq!(info.notice_body, "Be kind");
        assert_eq!(info.positions.len(), 1);
        assert_eq!(info.positions[0].index, 1);
        assert_eq!(info.positions[0].name, "Officer");
        assert!(info.positions[0].can_invite);
        assert!(info.positions[0].can_expel);
        assert!(!info.positions[0].can_storage);
        assert_eq!(info.positions[0].tax, 12);
        assert_eq!(info.members.len(), 1);
        assert_eq!(info.members[0].char_id, 42);
        assert_eq!(info.members[0].name, "Odin");
        assert_eq!(info.members[0].job_id, 4001);
        assert_eq!(info.members[0].base_level, 99);
        assert!(info.members[0].online);
        assert_eq!(info.members[0].map, "prontera");
        assert_eq!(info.members[0].position_index, 1);
        assert_eq!(info.members[0].hp, u32::MAX as u64 + 1);
        assert_eq!(info.members[0].max_hp, u32::MAX as u64 + 2);
        assert_eq!(info.members[0].sp, u32::MAX as u64 + 3);
        assert_eq!(info.members[0].max_sp, u32::MAX as u64 + 4);
        assert_eq!(info.members[0].ap, 5);
        assert_eq!(info.members[0].max_ap, 6);
    }

    #[test]
    fn guild_payload_maps_each_delta_and_notification_variant() {
        let cases = [
            (
                Body::GuildActionResult(net::GuildActionResult {
                    action: "invite".into(),
                    success: false,
                    error: net::GuildError::GuildErrNoPermission as i32,
                }),
                GuildIngressPayload::ActionResult(net_contract::dto::GuildActionResult {
                    action: "invite".into(),
                    success: false,
                    error: GuildErrorKind::NoPermission,
                }),
            ),
            (
                Body::GuildInviteNotify(net::GuildInviteNotify {
                    guild_id: 7,
                    guild_name: "Vikings".into(),
                    inviter_name: "Odin".into(),
                }),
                GuildIngressPayload::InviteNotified(net_contract::dto::GuildInviteInfo {
                    guild_id: 7,
                    guild_name: "Vikings".into(),
                    inviter_name: "Odin".into(),
                }),
            ),
            (
                Body::GuildMemberUpdate(net::GuildMemberUpdate {
                    guild_id: 7,
                    member: Some(net::GuildMember {
                        char_id: 43,
                        name: "Thor".into(),
                        job_id: 7,
                        base_level: 50,
                        online: false,
                        map: "geffen".into(),
                        position_index: 2,
                        hp: 8,
                        max_hp: 9,
                        sp: 10,
                        max_sp: 11,
                        ap: 12,
                        max_ap: 13,
                    }),
                }),
                GuildIngressPayload::MemberUpdated {
                    guild_id: 7,
                    member: net_contract::dto::GuildMemberInfo {
                        char_id: 43,
                        name: "Thor".into(),
                        job_id: 7,
                        base_level: 50,
                        online: false,
                        map: "geffen".into(),
                        position_index: 2,
                        hp: 8,
                        max_hp: 9,
                        sp: 10,
                        max_sp: 11,
                        ap: 12,
                        max_ap: 13,
                    },
                },
            ),
            (
                Body::GuildEmblemChanged(net::GuildEmblemChanged {
                    guild_id: 7,
                    emblem_id: 3,
                }),
                GuildIngressPayload::EmblemChanged {
                    guild_id: 7,
                    emblem_id: 3,
                },
            ),
            (
                Body::GuildEmblemData(net::GuildEmblemData {
                    guild_id: 7,
                    emblem_id: 3,
                    data: vec![0x42, 0x4d],
                }),
                GuildIngressPayload::EmblemData {
                    guild_id: 7,
                    emblem_id: 3,
                    data: vec![0x42, 0x4d],
                },
            ),
            (
                Body::GuildDisbanded(net::GuildDisbanded {
                    guild_id: 7,
                    reason: "master left".into(),
                }),
                GuildIngressPayload::Disbanded {
                    guild_id: 7,
                    reason: "master left".into(),
                },
            ),
        ];

        for (body, expected) in cases {
            assert_eq!(guild_payload(body), Some(expected));
        }
    }

    #[test]
    fn guild_scoped_payloads_reject_zero_guild_ids() {
        let bodies = [
            Body::GuildInviteNotify(net::GuildInviteNotify {
                guild_id: 0,
                guild_name: "Invalid".into(),
                inviter_name: "Odin".into(),
            }),
            Body::GuildInfo(net::GuildInfo {
                guild_id: 0,
                name: "Invalid".into(),
                master_char_id: 42,
                emblem_id: 1,
                notice_subject: String::new(),
                notice_body: String::new(),
                positions: vec![],
                members: vec![],
            }),
            Body::GuildMemberUpdate(net::GuildMemberUpdate {
                guild_id: 0,
                member: Some(net::GuildMember {
                    char_id: 42,
                    name: "Odin".into(),
                    job_id: 1,
                    base_level: 1,
                    online: true,
                    map: "prontera".into(),
                    position_index: 0,
                    hp: 1,
                    max_hp: 1,
                    sp: 1,
                    max_sp: 1,
                    ap: 0,
                    max_ap: 0,
                }),
            }),
            Body::GuildEmblemChanged(net::GuildEmblemChanged {
                guild_id: 0,
                emblem_id: 1,
            }),
            Body::GuildEmblemData(net::GuildEmblemData {
                guild_id: 0,
                emblem_id: 1,
                data: vec![0x42, 0x4d],
            }),
            Body::GuildDisbanded(net::GuildDisbanded {
                guild_id: 0,
                reason: "invalid".into(),
            }),
        ];

        for body in bodies {
            assert_eq!(guild_payload(body), None);
        }
    }
}
