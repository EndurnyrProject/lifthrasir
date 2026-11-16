//! Chat rooms: the signs NPCs and players open over their heads, and the room the
//! local player sits in.
//!
//! The server is the authority. [`ChatRooms`] only replays its event stream, with one
//! exception: the server tells a player nothing when they leave a room, so a sent
//! [`ChatRoomCommand::Leave`] takes the player out on its own.

use std::collections::HashMap;
use std::time::SystemTime;

use bevy::prelude::*;
use net_contract::commands::ChatRoomCommand;
use net_contract::dto::{ChatRoomMember, ChatRoomSign};
use net_contract::events::ChatRoomEvent;
use net_contract::state::ZoneSession;

use crate::core::state::GameState;
use crate::domain::entities::character::components::core::CharacterData;
use crate::domain::entities::components::EntityName;
use crate::domain::entities::markers::LocalPlayer;

/// Lines a room keeps; older ones fall off the top.
const LINES_KEPT: usize = 100;

/// One line of a room's log and when it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomLine {
    pub at: SystemTime,
    pub what: RoomLineKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomLineKind {
    Said {
        char_id: u32,
        name: String,
        text: String,
    },
    Joined(String),
    Left(String),
    Kicked(String),
    NewOwner(String),
    /// The owner edited the room. `Some` when who can join changed, `true` for anyone.
    SettingsChanged(Option<bool>),
}

/// The room the local player is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedRoom {
    /// The room as last announced. Its `member_count` can trail `members`.
    pub sign: ChatRoomSign,
    /// In join order, which is also the order the room is inherited in.
    pub members: Vec<ChatRoomMember>,
    pub lines: Vec<RoomLine>,
}

impl JoinedRoom {
    pub fn is_owner(&self, char_id: u32) -> bool {
        !self.sign.owner_is_npc && self.sign.owner_gid == char_id
    }

    fn log(&mut self, at: SystemTime, what: RoomLineKind) {
        self.lines.push(RoomLine { at, what });
        let extra = self.lines.len().saturating_sub(LINES_KEPT);
        self.lines.drain(..extra);
    }

    /// Takes a newer announcement of this room, noting an edit of its settings.
    fn announce(&mut self, sign: &ChatRoomSign, at: SystemTime) {
        let old = &self.sign;
        let edited = (&old.title, old.seats, old.public) != (&sign.title, sign.seats, sign.public);
        let access = (old.public != sign.public).then_some(sign.public);
        self.sign = sign.clone();
        if edited {
            self.log(at, RoomLineKind::SettingsChanged(access));
        }
    }
}

/// Every chat room the local player can see, and the one they are in.
#[derive(Resource, Default, Debug)]
pub struct ChatRooms {
    signs: HashMap<u32, ChatRoomSign>,
    joined: Option<JoinedRoom>,
    /// Set between the server accepting a new room and that room's first sign.
    opening: bool,
}

impl ChatRooms {
    /// The rooms in view by room id.
    pub fn signs(&self) -> &HashMap<u32, ChatRoomSign> {
        &self.signs
    }

    pub fn joined(&self) -> Option<&JoinedRoom> {
        self.joined.as_ref()
    }

    /// Takes the local player out of their room.
    pub fn leave(&mut self) {
        self.joined = None;
    }

    fn room(&mut self, room_id: u32) -> Option<&mut JoinedRoom> {
        self.joined
            .as_mut()
            .filter(|room| room.sign.room_id == room_id)
    }

    /// Replays one server event. `me` is the local player and `now` stamps the log.
    pub fn apply(&mut self, event: &ChatRoomEvent, me: &ChatRoomMember, now: SystemTime) {
        match event {
            ChatRoomEvent::SignShown(sign) => {
                self.signs.insert(sign.room_id, sign.clone());
                let own = !sign.owner_is_npc && sign.owner_gid == me.char_id;
                if self.opening && own {
                    self.opening = false;
                    self.joined = Some(JoinedRoom {
                        sign: sign.clone(),
                        members: vec![me.clone()],
                        lines: Vec::new(),
                    });
                    return;
                }
                if let Some(room) = self.room(sign.room_id) {
                    room.announce(sign, now);
                }
            }
            ChatRoomEvent::SignRemoved { room_id } => {
                self.signs.remove(room_id);
            }
            ChatRoomEvent::UnitLeft { gid } => self.signs.retain(|_, sign| sign.owner_gid != *gid),
            ChatRoomEvent::Created => self.opening = true,
            ChatRoomEvent::CreateRefused => self.opening = false,
            ChatRoomEvent::Joined {
                room_id,
                owner_gid,
                members,
            } => {
                let sign = self.signs.get(room_id).cloned().unwrap_or(ChatRoomSign {
                    room_id: *room_id,
                    public: true,
                    ..default()
                });
                self.joined = Some(JoinedRoom {
                    sign: ChatRoomSign {
                        owner_gid: *owner_gid,
                        ..sign
                    },
                    members: members.clone(),
                    lines: Vec::new(),
                });
            }
            ChatRoomEvent::JoinRefused { .. } => {}
            ChatRoomEvent::MemberJoined { room_id, member } => {
                let Some(room) = self.room(*room_id) else {
                    return;
                };
                room.members.retain(|known| known.char_id != member.char_id);
                room.members.push(member.clone());
                room.log(now, RoomLineKind::Joined(member.name.clone()));
            }
            ChatRoomEvent::MemberLeft {
                room_id,
                member,
                kicked,
            } => {
                let Some(room) = self.room(*room_id) else {
                    return;
                };
                if member.char_id == me.char_id {
                    self.joined = None;
                    return;
                }
                room.members.retain(|known| known.char_id != member.char_id);
                let name = member.name.clone();
                let what = if *kicked {
                    RoomLineKind::Kicked(name)
                } else {
                    RoomLineKind::Left(name)
                };
                room.log(now, what);
            }
            ChatRoomEvent::OwnerChanged { room_id, char_id } => {
                let Some(room) = self.room(*room_id) else {
                    return;
                };
                room.sign.owner_gid = *char_id;
                room.sign.owner_is_npc = false;
                let owner = room.members.iter().find(|m| m.char_id == *char_id);
                if let Some(name) = owner.map(|owner| owner.name.clone()) {
                    room.log(now, RoomLineKind::NewOwner(name));
                }
            }
            ChatRoomEvent::Said {
                room_id,
                char_id,
                name,
                text,
            } => {
                let Some(room) = self.room(*room_id) else {
                    return;
                };
                let what = RoomLineKind::Said {
                    char_id: *char_id,
                    name: name.clone(),
                    text: text.clone(),
                };
                room.log(now, what);
            }
            ChatRoomEvent::MapChanging => *self = Self::default(),
        }
    }
}

pub struct ChatRoomPlugin;

impl Plugin for ChatRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatRooms>()
            .add_systems(Update, replay_chat_rooms)
            .add_systems(OnEnter(GameState::CharacterSelection), reset_chat_rooms);
    }
}

type Hero<'a> = (&'a CharacterData, Option<&'a EntityName>);

/// Replays the server's events, then the leaves the player sent this frame: an event
/// still in flight when the player left must not put them back.
fn replay_chat_rooms(
    mut events: MessageReader<ChatRoomEvent>,
    mut sent: MessageReader<ChatRoomCommand>,
    session: Res<ZoneSession>,
    hero: Query<Hero, With<LocalPlayer>>,
    mut rooms: ResMut<ChatRooms>,
) {
    let left = sent
        .read()
        .any(|command| *command == ChatRoomCommand::Leave);
    if events.is_empty() && !left {
        return;
    }
    let name = hero.single().map_or_else(
        |_| String::new(),
        |(data, name)| name.map_or_else(|| data.name.clone(), |name| name.name.clone()),
    );
    let me = ChatRoomMember {
        char_id: session.char_id,
        name,
    };
    let now = SystemTime::now();
    for event in events.read() {
        rooms.apply(event, &me, now);
    }
    if left {
        rooms.leave();
    }
}

fn reset_chat_rooms(mut rooms: ResMut<ChatRooms>) {
    *rooms = ChatRooms::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: u32 = 42;
    const ROOM: u32 = 9;

    fn member(char_id: u32, name: &str) -> ChatRoomMember {
        ChatRoomMember {
            char_id,
            name: name.into(),
        }
    }

    fn sign(owner_gid: u32) -> ChatRoomSign {
        ChatRoomSign {
            room_id: ROOM,
            title: "Glast Heim run".into(),
            member_count: 1,
            seats: 6,
            public: true,
            owner_gid,
            owner_is_npc: false,
        }
    }

    fn replay(rooms: &mut ChatRooms, events: impl IntoIterator<Item = ChatRoomEvent>) {
        for event in events {
            rooms.apply(&event, &member(ME, "Ygor"), SystemTime::UNIX_EPOCH);
        }
    }

    /// A room joined as a member, owned by Freyja (5), with Orm (7) in it.
    fn joined() -> ChatRooms {
        let mut rooms = ChatRooms::default();
        replay(
            &mut rooms,
            [
                ChatRoomEvent::SignShown(sign(5)),
                ChatRoomEvent::Joined {
                    room_id: ROOM,
                    owner_gid: 5,
                    members: vec![member(5, "Freyja"), member(7, "Orm"), member(ME, "Ygor")],
                },
            ],
        );
        rooms
    }

    fn kinds(rooms: &ChatRooms) -> Vec<RoomLineKind> {
        let room = rooms.joined().expect("in a room");
        room.lines.iter().map(|line| line.what.clone()).collect()
    }

    #[test]
    fn an_accepted_create_makes_the_player_owner_of_their_first_sign() {
        let mut rooms = ChatRooms::default();
        replay(&mut rooms, [ChatRoomEvent::SignShown(sign(ME))]);
        assert!(rooms.joined().is_none(), "a sign alone opens nothing");

        replay(
            &mut rooms,
            [ChatRoomEvent::Created, ChatRoomEvent::SignShown(sign(ME))],
        );
        let room = rooms.joined().expect("the room is open");
        assert!(room.is_owner(ME));
        assert_eq!(room.members, [member(ME, "Ygor")]);
    }

    #[test]
    fn a_sign_in_flight_does_not_bring_back_a_room_the_player_left() {
        let mut rooms = ChatRooms::default();
        replay(
            &mut rooms,
            [ChatRoomEvent::Created, ChatRoomEvent::SignShown(sign(ME))],
        );
        rooms.leave();
        replay(&mut rooms, [ChatRoomEvent::SignShown(sign(ME))]);
        assert!(rooms.joined().is_none());
    }

    #[test]
    fn joining_takes_the_roster_and_the_sign_of_the_room() {
        let rooms = joined();
        let room = rooms.joined().expect("in the room");
        assert_eq!(room.sign.title, "Glast Heim run");
        assert_eq!(room.members.len(), 3);
        assert!(!room.is_owner(ME) && room.is_owner(5));
    }

    #[test]
    fn members_come_and_go_in_the_log() {
        let mut rooms = joined();
        replay(
            &mut rooms,
            [
                ChatRoomEvent::MemberJoined {
                    room_id: ROOM,
                    member: member(8, "Eir"),
                },
                ChatRoomEvent::Said {
                    room_id: ROOM,
                    char_id: 8,
                    name: "Eir".into(),
                    text: "hello".into(),
                },
                ChatRoomEvent::MemberLeft {
                    room_id: ROOM,
                    member: member(7, "Orm"),
                    kicked: true,
                },
                ChatRoomEvent::MemberLeft {
                    room_id: ROOM,
                    member: member(8, "Eir"),
                    kicked: false,
                },
                // Another room's traffic is not this room's.
                ChatRoomEvent::MemberJoined {
                    room_id: ROOM + 1,
                    member: member(3, "Bjorn"),
                },
            ],
        );
        let names: Vec<_> = rooms.joined().unwrap().members.clone();
        assert_eq!(names, [member(5, "Freyja"), member(ME, "Ygor")]);
        assert_eq!(
            kinds(&rooms),
            [
                RoomLineKind::Joined("Eir".into()),
                RoomLineKind::Said {
                    char_id: 8,
                    name: "Eir".into(),
                    text: "hello".into()
                },
                RoomLineKind::Kicked("Orm".into()),
                RoomLineKind::Left("Eir".into()),
            ]
        );
    }

    #[test]
    fn being_removed_ends_the_room() {
        let mut rooms = joined();
        replay(
            &mut rooms,
            [ChatRoomEvent::MemberLeft {
                room_id: ROOM,
                member: member(ME, "Ygor"),
                kicked: true,
            }],
        );
        assert!(rooms.joined().is_none());
        assert!(rooms.signs().contains_key(&ROOM), "the room itself stays");
    }

    #[test]
    fn an_owner_leaving_hands_the_room_and_its_sign_over() {
        let mut rooms = joined();
        replay(
            &mut rooms,
            [
                ChatRoomEvent::MemberLeft {
                    room_id: ROOM,
                    member: member(5, "Freyja"),
                    kicked: false,
                },
                ChatRoomEvent::OwnerChanged {
                    room_id: ROOM,
                    char_id: 7,
                },
                ChatRoomEvent::SignRemoved { room_id: ROOM },
                ChatRoomEvent::SignShown(sign(7)),
            ],
        );
        let room = rooms.joined().expect("still in the room");
        assert!(room.is_owner(7));
        assert_eq!(rooms.signs()[&ROOM].owner_gid, 7);
        assert_eq!(
            kinds(&rooms),
            [
                RoomLineKind::Left("Freyja".into()),
                RoomLineKind::NewOwner("Orm".into()),
            ]
        );
    }

    #[test]
    fn an_edit_is_logged_but_a_head_count_is_not() {
        let mut rooms = joined();
        let counted = ChatRoomSign {
            member_count: 4,
            ..sign(5)
        };
        replay(&mut rooms, [ChatRoomEvent::SignShown(counted)]);
        assert!(kinds(&rooms).is_empty());

        let locked = ChatRoomSign {
            public: false,
            title: "Wolfpack".into(),
            ..sign(5)
        };
        replay(&mut rooms, [ChatRoomEvent::SignShown(locked)]);
        assert_eq!(kinds(&rooms), [RoomLineKind::SettingsChanged(Some(false))]);
        assert_eq!(rooms.joined().unwrap().sign.title, "Wolfpack");
    }

    #[test]
    fn a_sign_goes_with_its_owner_and_everything_goes_with_the_map() {
        let mut rooms = joined();
        replay(&mut rooms, [ChatRoomEvent::UnitLeft { gid: 3 }]);
        assert_eq!(rooms.signs().len(), 1, "another unit takes no sign");

        replay(&mut rooms, [ChatRoomEvent::UnitLeft { gid: 5 }]);
        assert!(rooms.signs().is_empty());
        assert!(rooms.joined().is_some(), "membership is not the sign");

        replay(&mut rooms, [ChatRoomEvent::MapChanging]);
        assert!(rooms.joined().is_none());
    }

    #[test]
    fn a_sent_leave_takes_the_player_out_after_the_frames_events() {
        let mut app = App::new();
        app.add_message::<ChatRoomEvent>()
            .add_message::<ChatRoomCommand>()
            .init_resource::<ZoneSession>()
            .insert_resource(joined())
            .add_systems(Update, replay_chat_rooms);
        app.world_mut().resource_mut::<ZoneSession>().char_id = ME;
        app.world_mut().write_message(ChatRoomCommand::Leave);
        app.world_mut().write_message(ChatRoomEvent::MemberJoined {
            room_id: ROOM,
            member: member(8, "Eir"),
        });

        app.update();

        assert!(app.world().resource::<ChatRooms>().joined().is_none());
    }
}
