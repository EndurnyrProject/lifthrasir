//! View-models of the chat room UI: plain values the scenes draw, computed from the
//! domain's rooms and the player's picks. Nothing here touches the ECS.

use std::time::SystemTime;

use bevy::prelude::{Alpha, Color};
use chrono::Timelike;
use lifthrasir_client::domain::chat_room::{JoinedRoom, RoomLine, RoomLineKind};
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{ChatRoomJoinError, ChatRoomSettings, ChatRoomSign};

/// Longest room title the form takes; the server cuts at 60 bytes.
pub(super) const TITLE_MAX: usize = 36;
/// Longest password, the server's cap.
pub(super) const PASSWORD_MAX: usize = 8;
/// Longest line said in a room.
pub(super) const MESSAGE_MAX: usize = 70;
const SEATS_MIN: u32 = 2;
/// The server's cap on a room.
const SEATS_MAX: u32 = 20;

/// The ring of an NPC's room and of the notes about it.
const NPC_TONE: Color = ro_dark::FROST;

/// What kind of room a sign stands for, which picks its ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignTone {
    Open,
    Private,
    Full,
    Npc,
    /// The local player's own room.
    Own,
}

impl SignTone {
    /// The sign's ring, then the fill and ink of its head count.
    pub(crate) fn colors(self) -> (Color, Color, Color) {
        let tinted = |tone: Color| (tone, tone.with_alpha(0.18), tone);
        match self {
            Self::Open => (
                ro_dark::BRONZE,
                Color::WHITE.with_alpha(0.08),
                ro_dark::LABEL,
            ),
            Self::Private => tinted(ro_dark::TONE_WHISPER),
            Self::Full => tinted(ro_dark::TONE_ERROR),
            Self::Npc => tinted(NPC_TONE),
            Self::Own => tinted(ro_dark::RARE_RING),
        }
    }
}

/// A room's sign: its title, how full it is and the marks before the title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignView {
    pub title: String,
    pub count: String,
    pub private: bool,
    pub npc: bool,
    pub tone: SignTone,
}

pub fn sign(sign: &ChatRoomSign, own: bool) -> SignView {
    let tone = match (own, sign.owner_is_npc, sign.public, sign.is_full()) {
        (true, ..) => SignTone::Own,
        (_, true, ..) => SignTone::Npc,
        (_, _, false, _) => SignTone::Private,
        (_, _, _, true) => SignTone::Full,
        _ => SignTone::Open,
    };
    SignView {
        title: sign.title.clone(),
        count: format!("{}/{}", sign.member_count, sign.seats),
        private: !sign.public,
        npc: sign.owner_is_npc,
        tone,
    }
}

/// The top of a sign's card: what the room is and who runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardHeadView {
    pub title: String,
    pub count: String,
    pub full: bool,
    pub meta: String,
}

/// `owner` is the name of the unit under the sign, while it is known.
pub fn card_head(sign: &ChatRoomSign, owner: Option<&str>) -> CardHeadView {
    let kind = match (sign.owner_is_npc, sign.public) {
        (true, _) => "Event waiting room",
        (false, true) => "Open room",
        (false, false) => "Password room",
    };
    let meta = match owner.filter(|_| !sign.owner_is_npc) {
        Some(owner) => format!("{kind} \u{b7} by {owner}"),
        None => kind.to_string(),
    };
    CardHeadView {
        title: sign.title.clone(),
        count: format!("{} / {}", sign.member_count, sign.seats),
        full: sign.is_full(),
        meta,
    }
}

/// The bottom of a sign's card: why joining failed or cannot work, and the button.
#[derive(Debug, Clone, PartialEq)]
pub struct CardFootView {
    pub note: Option<(String, Color)>,
    pub join: &'static str,
    pub enabled: bool,
}

/// `seated` is true while the player is in a room, which rules out a second one.
pub fn card_foot(
    sign: &ChatRoomSign,
    refusal: Option<ChatRoomJoinError>,
    seated: bool,
) -> CardFootView {
    let full = sign.is_full();
    let note = if seated {
        Some(("Leave your room to join this one.".into(), ro_dark::LABEL))
    } else if let Some(reason) = refusal {
        Some((refusal_text(reason), ro_dark::TONE_ERROR))
    } else if full {
        Some((
            "This room is full. Try again later.".into(),
            ro_dark::TONE_ERROR,
        ))
    } else {
        None
    };
    let join = match (full, sign.owner_is_npc) {
        (true, _) => "Full",
        (false, true) => "Take a Place",
        (false, false) => "Join",
    };
    CardFootView {
        note,
        join,
        enabled: !full && !seated,
    }
}

pub fn refusal_text(reason: ChatRoomJoinError) -> String {
    match reason {
        ChatRoomJoinError::Full => "This room is full. Try again later.".into(),
        ChatRoomJoinError::WrongPassword => "That\u{2019}s not the password".into(),
        ChatRoomJoinError::LevelTooLow => "Your level is too low for this room.".into(),
        ChatRoomJoinError::LevelTooHigh => "Your level is too high for this room.".into(),
        ChatRoomJoinError::NotEnoughZeny => "You do not have the zeny this room asks for.".into(),
        ChatRoomJoinError::Kicked => "The owner removed you from this room.".into(),
        ChatRoomJoinError::Unknown(code) => format!("You cannot join this room ({code})."),
    }
}

/// The room window's title bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadView {
    pub title: String,
    pub private: bool,
    pub count: String,
    pub full: bool,
    /// The local player owns the room and may edit it.
    pub owner: bool,
    pub editing: bool,
}

pub fn head(room: &JoinedRoom, me: u32, editing: bool) -> HeadView {
    let people = room.members.len() as u32;
    HeadView {
        title: room.sign.title.clone(),
        private: !room.sign.public,
        count: format!("{people} / {}", room.sign.seats),
        full: people >= room.sign.seats,
        owner: room.is_owner(me),
        editing,
    }
}

/// One line of the room's log.
#[derive(Debug, Clone, PartialEq)]
pub enum LineView {
    /// Something that happened to the room, centred in its `tone`.
    Notice { text: String, tone: Color },
    Said {
        time: String,
        name: String,
        me: bool,
        /// The speaker owns the room now.
        crown: bool,
        text: String,
    },
}

pub fn log(room: &JoinedRoom, me: u32) -> Vec<LineView> {
    room.lines
        .iter()
        .map(|line| line_view(room, line, me))
        .collect()
}

fn line_view(room: &JoinedRoom, line: &RoomLine, me: u32) -> LineView {
    let notice = |text: String, tone: Color| LineView::Notice { text, tone };
    match &line.what {
        RoomLineKind::Said {
            char_id,
            name,
            text,
        } => LineView::Said {
            time: clock(line.at),
            name: name.clone(),
            me: *char_id == me,
            crown: room.is_owner(*char_id),
            text: text.clone(),
        },
        RoomLineKind::Joined(name) => notice(format!("{name} joined"), ro_dark::TONE_OK),
        RoomLineKind::Left(name) => notice(format!("{name} left"), ro_dark::MUTED),
        RoomLineKind::Kicked(name) => notice(
            format!("{name} was removed from the room"),
            ro_dark::TONE_ERROR,
        ),
        RoomLineKind::NewOwner(name) => {
            notice(format!("{name} is now the room owner"), ro_dark::RARE_RING)
        }
        RoomLineKind::SettingsChanged(access) => {
            let access = match access {
                Some(true) => " \u{b7} now open to anyone",
                Some(false) => " \u{b7} now needs a password",
                None => "",
            };
            notice(format!("Room settings changed{access}"), ro_dark::FROST)
        }
    }
}

/// The local wall-clock time of `at`, as `HH:MM`.
fn clock(at: SystemTime) -> String {
    let at = chrono::DateTime::<chrono::Local>::from(at);
    format!("{:02}:{:02}", at.hour(), at.minute())
}

/// One row of the people list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonView {
    pub char_id: u32,
    pub label: String,
    pub initial: String,
    pub me: bool,
    pub owner: bool,
    /// The local player owns the room and may hand it to, or remove, this member.
    pub manage: bool,
}

/// The people in the room and the label of the button that leaves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeopleView {
    pub count: String,
    pub rows: Vec<PersonView>,
    pub leave: String,
}

pub fn people(room: &JoinedRoom, me: u32) -> PeopleView {
    let mine = room.is_owner(me);
    let rows = room
        .members
        .iter()
        .map(|member| {
            let own = member.char_id == me;
            PersonView {
                char_id: member.char_id,
                label: if own {
                    format!("{} (you)", member.name)
                } else {
                    member.name.clone()
                },
                initial: member.name.chars().take(1).collect(),
                me: own,
                owner: room.is_owner(member.char_id),
                manage: mine && !own,
            }
        })
        .collect();
    // The room passes to whoever joined first after the owner.
    let heir = room.members.iter().find(|member| member.char_id != me);
    let leave = match (mine, heir) {
        (true, Some(heir)) => format!("Leave \u{b7} {} takes over", heir.name),
        (true, None) => "Close Room".into(),
        (false, _) => "Leave Room".into(),
    };
    PeopleView {
        count: format!("{} / {}", room.members.len(), room.sign.seats),
        rows,
        leave,
    }
}

/// The room a player is setting up or editing. `title` and `password` mirror what is
/// typed in the form's fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomForm {
    pub title: String,
    pub password: String,
    pub seats: u32,
    pub private: bool,
}

impl Default for RoomForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            password: String::new(),
            seats: 6,
            private: false,
        }
    }
}

impl RoomForm {
    /// The form of a room that exists, for its owner to edit. The server never sends
    /// a password back, so `password` is the one this player set, if they did.
    pub fn of(sign: &ChatRoomSign, password: &str) -> Self {
        Self {
            title: sign.title.clone(),
            password: password.into(),
            seats: sign.seats,
            private: !sign.public,
        }
    }

    /// A room needs a title, and a password when it asks for one.
    pub fn is_valid(&self) -> bool {
        !self.title.trim().is_empty() && (!self.private || !self.password.is_empty())
    }

    /// Steps the seats by `delta`, keeping room for the `seated` people already in.
    pub fn step_seats(&mut self, delta: i32, seated: u32) {
        let floor = seated.max(SEATS_MIN);
        self.seats = self
            .seats
            .saturating_add_signed(delta)
            .clamp(floor, SEATS_MAX);
    }

    pub fn settings(&self) -> ChatRoomSettings {
        ChatRoomSettings {
            title: self.title.trim().into(),
            password: if self.private {
                self.password.clone()
            } else {
                String::new()
            },
            seats: self.seats,
            public: !self.private,
        }
    }
}

/// Which form is on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    /// The New Chat Room dialog.
    Create,
    /// The owner's settings panel, in a room of `seated` people.
    Edit { seated: u32 },
}

impl FormMode {
    pub fn seated(self) -> u32 {
        match self {
            Self::Create => 0,
            Self::Edit { seated } => seated,
        }
    }
}

/// The form's seat stepper and who-can-join choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionsView {
    pub mode: FormMode,
    pub seats: u32,
    pub fewer: bool,
    pub more: bool,
    pub private: bool,
}

pub fn options(form: &RoomForm, mode: FormMode) -> OptionsView {
    OptionsView {
        mode,
        seats: form.seats,
        fewer: form.seats > mode.seated().max(SEATS_MIN),
        more: form.seats < SEATS_MAX,
        private: form.private,
    }
}

/// The form's buttons and, for a new room, the sign it would hang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootView {
    pub preview: Option<SignView>,
    pub submit: &'static str,
    pub enabled: bool,
}

pub fn foot(form: &RoomForm, mode: FormMode) -> FootView {
    let title = form.title.trim();
    let preview = (mode == FormMode::Create).then(|| SignView {
        title: if title.is_empty() {
            "Your room title".into()
        } else {
            title.into()
        },
        count: format!("1/{}", form.seats),
        private: form.private,
        npc: false,
        tone: SignTone::Own,
    });
    FootView {
        preview,
        submit: match mode {
            FormMode::Create => "Open Room",
            FormMode::Edit { .. } => "Save",
        },
        enabled: form.is_valid(),
    }
}
