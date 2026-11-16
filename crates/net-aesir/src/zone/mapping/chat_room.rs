use crate::envelope::Body;
use crate::proto::aesir::net;
use net_contract::dto::{ChatRoomJoinError, ChatRoomMember, ChatRoomSign};
use net_contract::events::ChatRoomEvent;

/// The chat room event a body stands for, or `None` for a body that says nothing
/// about chat rooms.
pub fn chat_room_event(body: Body) -> Option<ChatRoomEvent> {
    Some(match body {
        Body::WaitingRoomInfo(info) => ChatRoomEvent::SignShown(sign(info)),
        Body::WaitingRoomRemoved(r) => ChatRoomEvent::SignRemoved { room_id: r.room_id },
        Body::WaitingRoomCreateResult(r) if r.result == 0 => ChatRoomEvent::Created,
        Body::WaitingRoomCreateResult(_) => ChatRoomEvent::CreateRefused,
        Body::WaitingRoomJoinResult(r) if r.result == 0 => ChatRoomEvent::Joined {
            room_id: r.room_id,
            owner_gid: r.owner_gid,
            members: r.members.into_iter().map(member).collect(),
        },
        Body::WaitingRoomJoinResult(r) => ChatRoomEvent::JoinRefused {
            room_id: r.room_id,
            reason: join_error(r.result),
        },
        Body::WaitingRoomMemberUpdate(u) => {
            let member = ChatRoomMember {
                char_id: u.char_id,
                name: u.name,
            };
            if u.joined {
                ChatRoomEvent::MemberJoined {
                    room_id: u.room_id,
                    member,
                }
            } else {
                ChatRoomEvent::MemberLeft {
                    room_id: u.room_id,
                    member,
                    kicked: u.kicked,
                }
            }
        }
        // The server also names the member who stopped owning the room; the new owner
        // alone says who owns it now.
        Body::WaitingRoomRoleChanged(r) if r.owner => ChatRoomEvent::OwnerChanged {
            room_id: r.room_id,
            char_id: r.char_id,
        },
        Body::WaitingRoomChat(c) => ChatRoomEvent::Said {
            room_id: c.room_id,
            char_id: c.char_id,
            name: c.name,
            text: c.message,
        },
        Body::UnitDespawn(d) => ChatRoomEvent::UnitLeft { gid: d.gid },
        Body::MapMove(_) => ChatRoomEvent::MapChanging,
        _ => return None,
    })
}

/// An owner NPC takes one place of the wire limit but is not a member, so the seats
/// players can fill are one fewer.
fn sign(info: net::WaitingRoomInfo) -> ChatRoomSign {
    ChatRoomSign {
        room_id: info.room_id,
        title: info.title,
        member_count: info.member_count,
        seats: info.limit.saturating_sub(u32::from(info.owner_is_npc)),
        public: info.public,
        owner_gid: info.owner_gid,
        owner_is_npc: info.owner_is_npc,
    }
}

fn member(m: net::WaitingRoomMember) -> ChatRoomMember {
    ChatRoomMember {
        char_id: m.char_id,
        name: m.name,
    }
}

fn join_error(code: u32) -> ChatRoomJoinError {
    match code {
        1 => ChatRoomJoinError::Full,
        2 => ChatRoomJoinError::WrongPassword,
        3 => ChatRoomJoinError::LevelTooLow,
        4 => ChatRoomJoinError::LevelTooHigh,
        5 => ChatRoomJoinError::NotEnoughZeny,
        6 => ChatRoomJoinError::Kicked,
        other => ChatRoomJoinError::Unknown(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(owner_is_npc: bool) -> Body {
        Body::WaitingRoomInfo(net::WaitingRoomInfo {
            room_id: 9,
            title: "Arena".into(),
            member_count: 3,
            limit: 10,
            public: true,
            owner_gid: 77,
            owner_is_npc,
        })
    }

    #[test]
    fn an_npc_room_offers_one_seat_fewer_than_its_limit() {
        let Some(ChatRoomEvent::SignShown(npc)) = chat_room_event(info(true)) else {
            panic!("expected a sign");
        };
        assert_eq!((npc.seats, npc.member_count, npc.owner_gid), (9, 3, 77));

        let Some(ChatRoomEvent::SignShown(player)) = chat_room_event(info(false)) else {
            panic!("expected a sign");
        };
        assert_eq!(player.seats, 10);
    }

    #[test]
    fn a_join_result_is_a_roster_or_a_reason() {
        let result = |result, members| {
            chat_room_event(Body::WaitingRoomJoinResult(net::WaitingRoomJoinResult {
                room_id: 9,
                result,
                members,
                owner_gid: 77,
            }))
        };
        let roster = vec![net::WaitingRoomMember {
            char_id: 5,
            name: "Freyja".into(),
        }];
        assert_eq!(
            result(0, roster),
            Some(ChatRoomEvent::Joined {
                room_id: 9,
                owner_gid: 77,
                members: vec![ChatRoomMember {
                    char_id: 5,
                    name: "Freyja".into()
                }],
            })
        );
        let reason = |code| match result(code, vec![]) {
            Some(ChatRoomEvent::JoinRefused { reason, .. }) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        };
        assert_eq!(reason(1), ChatRoomJoinError::Full);
        assert_eq!(reason(2), ChatRoomJoinError::WrongPassword);
        assert_eq!(reason(6), ChatRoomJoinError::Kicked);
        assert_eq!(reason(42), ChatRoomJoinError::Unknown(42));
    }

    #[test]
    fn a_member_update_is_a_join_a_leave_or_a_kick() {
        let update = |joined, kicked| {
            chat_room_event(Body::WaitingRoomMemberUpdate(
                net::WaitingRoomMemberUpdate {
                    room_id: 9,
                    joined,
                    kicked,
                    char_id: 5,
                    name: "Freyja".into(),
                },
            ))
        };
        assert!(matches!(
            update(true, false),
            Some(ChatRoomEvent::MemberJoined { .. })
        ));
        assert!(matches!(
            update(false, false),
            Some(ChatRoomEvent::MemberLeft { kicked: false, .. })
        ));
        assert!(matches!(
            update(false, true),
            Some(ChatRoomEvent::MemberLeft { kicked: true, .. })
        ));
    }

    #[test]
    fn only_the_new_owner_changes_the_owner() {
        let role = |owner| {
            chat_room_event(Body::WaitingRoomRoleChanged(net::WaitingRoomRoleChanged {
                room_id: 9,
                char_id: 5,
                owner,
            }))
        };
        assert_eq!(
            role(true),
            Some(ChatRoomEvent::OwnerChanged {
                room_id: 9,
                char_id: 5
            })
        );
        assert_eq!(role(false), None);
    }

    #[test]
    fn a_create_result_opens_or_refuses() {
        let created = |result| {
            chat_room_event(Body::WaitingRoomCreateResult(
                net::WaitingRoomCreateResult { result },
            ))
        };
        assert_eq!(created(0), Some(ChatRoomEvent::Created));
        assert_eq!(created(1), Some(ChatRoomEvent::CreateRefused));
    }
}
