//! View-models of the party window, the HUD frames and the invitation.
//!
//! The views hold only what changes rarely (who is in the party, who leads, what is
//! selected), so rebuilding on a changed view never respawns a control mid-click.
//! What changes all the time (HP, SP, distance) is a [`MemberReadout`] that
//! [`readout`] refreshes in place.

use bevy::prelude::*;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::domain::party::PartyState;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::PartyMemberInfo;
use net_contract::events::PartyInviteNotified;

use super::{PARTY_MAX, PartyUi};
use crate::screens::shared::thousands;

/// NOTE: aesir's default `party_share_level`. The protocol does not carry the server's
/// setting; read it from the party snapshot once it does.
pub const SHARE_GAP: u32 = 15;
/// HP percentage under which a member is flagged.
const LOW_HP: f32 = 35.0;
/// Slots the roster grid always shows; open ones fill the rest.
const SHOWN_SLOTS: usize = 8;

/// The party as the local player `me` sees it.
#[derive(Clone, Copy)]
pub struct Roster<'a> {
    pub party: &'a PartyState,
    pub me: u32,
    pub jobs: Option<&'a JobSpriteRegistry>,
}

impl Roster<'_> {
    fn member(&self, char_id: u32) -> Option<&PartyMemberInfo> {
        self.party.members.iter().find(|m| m.char_id == char_id)
    }

    /// The map the local player is on, as the party reports it.
    pub fn my_map(&self) -> &str {
        self.member(self.me).map_or("", |m| m.map.as_str())
    }

    fn online(&self) -> usize {
        self.party.members.iter().filter(|m| m.online).count()
    }

    fn low(member: &PartyMemberInfo) -> bool {
        member.online && percent(member.hp, member.max_hp) < LOW_HP
    }
}

/// A job's name, or a stable fallback while the registry is loading or the id is new.
fn job_name(jobs: Option<&JobSpriteRegistry>, job_id: u32) -> String {
    jobs.and_then(|jobs| jobs.try_display_name(job_id))
        .unwrap_or("Unknown Job")
        .to_string()
}

/// `current` over `max` in percent, capped at 100.
fn percent(current: u64, max: u64) -> f32 {
    if max == 0 {
        return 0.0;
    }
    (current as f64 * 100.0 / max as f64).min(100.0) as f32
}

/// Lowest and highest base level among online members.
fn level_range(levels: impl Iterator<Item = u32> + Clone) -> Option<(u32, u32)> {
    Some((levels.clone().min()?, levels.max()?))
}

fn online_levels(members: &[PartyMemberInfo]) -> impl Iterator<Item = u32> + Clone {
    members.iter().filter(|m| m.online).map(|m| m.base_level)
}

/// The window's heading: the party's name and its head count.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadView {
    pub title: String,
    /// The member counts, then the leader's name.
    pub count: Option<(String, String)>,
}

pub fn head(roster: Roster) -> HeadView {
    let party = roster.party;
    if !party.in_party() {
        return HeadView {
            title: "No Party".into(),
            count: None,
        };
    }
    let leader = roster
        .member(party.leader_char_id)
        .map_or_else(String::new, |m| m.name.clone());
    HeadView {
        title: party.name.clone(),
        count: Some((
            format!(
                "{} / {PARTY_MAX} members \u{b7} {} online \u{b7} Led by ",
                party.members.len(),
                roster.online()
            ),
            leader,
        )),
    }
}

/// What the name field beside the heading does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormView {
    /// Founds a party with the typed name.
    Create,
    /// Invites the typed hero; only the leader may.
    Invite { leader: bool },
}

pub fn form(roster: Roster) -> FormView {
    if !roster.party.in_party() {
        return FormView::Create;
    }
    FormView::Invite {
        leader: roster.party.is_leader(roster.me),
    }
}

/// One roster card. HP, SP and position are [`MemberReadout`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct CardView {
    pub char_id: u32,
    pub name: String,
    pub me: bool,
    pub leader: bool,
    pub online: bool,
    pub selected: bool,
    /// Job, level and a low-HP flag.
    pub detail: String,
}

/// The bar under the roster acting on the selected member.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionView {
    pub char_id: u32,
    pub name: String,
    pub title: String,
    pub meta: String,
    /// The expel prompt is up.
    pub ask_expel: bool,
    pub can_trade: bool,
    pub can_lead: bool,
    pub can_expel: bool,
}

/// The sharing options, which only the leader may change.
#[derive(Clone, Debug, PartialEq)]
pub struct SharingView {
    pub leader: bool,
    pub exp_share: bool,
    pub pickup_share: bool,
    /// Online levels are close enough for Even Share.
    pub even_ok: bool,
    pub exp_note: String,
    pub exp_warn: bool,
}

/// Online members' levels on a track, with the span between the extremes.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelsView {
    pub lo: u32,
    pub hi: u32,
    /// Left edge and width of the span, in percent of the track.
    pub band: (f32, f32),
    pub even_ok: bool,
    /// Each online member's place in percent, and whether it is the local player.
    pub ticks: Vec<(f32, bool)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeaveView {
    pub ask: bool,
    pub prompt: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyView {
    pub cards: Vec<CardView>,
    pub open_slots: usize,
    pub selection: Option<SelectionView>,
    pub sharing: SharingView,
    pub levels: LevelsView,
    pub leave: LeaveView,
}

/// `on_screen` tells whether a member's character is in view, and `trade_free` whether
/// the local player may start a trade.
pub fn body(
    roster: Roster,
    ui: &PartyUi,
    on_screen: impl Fn(u32) -> bool,
    trade_free: bool,
) -> BodyView {
    let party = roster.party;
    let leader = party.is_leader(roster.me);
    let cards: Vec<_> = party
        .members
        .iter()
        .map(|member| CardView {
            char_id: member.char_id,
            name: member.name.clone(),
            me: member.char_id == roster.me,
            leader: member.char_id == party.leader_char_id,
            online: member.online,
            selected: ui.selected == Some(member.char_id),
            detail: format!(
                "{} \u{b7} Base Lv {}{}",
                job_name(roster.jobs, member.job_id),
                member.base_level,
                if Roster::low(member) {
                    " \u{b7} Low HP"
                } else {
                    ""
                }
            ),
        })
        .collect();
    let selection = ui
        .selected
        .and_then(|char_id| roster.member(char_id))
        .map(|member| {
            let me = member.char_id == roster.me;
            let is_leader = member.char_id == party.leader_char_id;
            let place = if member.online {
                member.map.as_str()
            } else {
                "Offline"
            };
            SelectionView {
                char_id: member.char_id,
                name: member.name.clone(),
                title: format!(
                    "{}{}{}",
                    member.name,
                    if me { " (you)" } else { "" },
                    if is_leader { " \u{b7} Leader" } else { "" }
                ),
                meta: format!(
                    "{} \u{b7} Base Lv {} \u{b7} {place}",
                    job_name(roster.jobs, member.job_id),
                    member.base_level
                ),
                ask_expel: ui.ask_expel,
                can_trade: !me && member.online && on_screen(member.char_id) && trade_free,
                can_lead: !me && leader && member.online,
                can_expel: !me && leader,
            }
        });
    let (min, max) = level_range(online_levels(&party.members)).unwrap_or((1, 1));
    let even_ok = max - min <= SHARE_GAP;
    let (exp_note, exp_warn) = match (party.exp_share, even_ok) {
        (true, _) => (
            "Everyone online on the same map gets an equal cut.".to_string(),
            false,
        ),
        (false, false) => (
            format!(
                "Even Share is unavailable: levels are {} apart (max {SHARE_GAP}).",
                max - min
            ),
            true,
        ),
        (false, true) => ("Each hero keeps the EXP they earn.".to_string(), false),
    };
    BodyView {
        open_slots: SHOWN_SLOTS.saturating_sub(cards.len()),
        cards,
        selection,
        sharing: SharingView {
            leader,
            exp_share: party.exp_share,
            pickup_share: party.item_pickup_share,
            even_ok,
            exp_note,
            exp_warn,
        },
        levels: levels(roster, min, max, even_ok),
        leave: LeaveView {
            ask: ui.ask_leave,
            prompt: if leader {
                format!("Leaving disbands {}.", party.name)
            } else {
                format!("Leave {}?", party.name)
            },
        },
    }
}

/// The track runs one share gap beyond both extremes, so the span reads against the
/// widest gap Even Share allows.
fn levels(roster: Roster, min: u32, max: u32, even_ok: bool) -> LevelsView {
    let lo = min.saturating_sub(SHARE_GAP).max(1);
    let hi = max + SHARE_GAP;
    let at = |level: u32| (level - lo) as f32 * 100.0 / (hi - lo) as f32;
    LevelsView {
        lo,
        hi,
        band: (at(min), at(max) - at(min)),
        even_ok,
        ticks: roster
            .party
            .members
            .iter()
            .filter(|m| m.online)
            .map(|m| (at(m.base_level), m.char_id == roster.me))
            .collect(),
    }
}

/// One HUD frame. Its bars, tag and position are [`MemberReadout`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameView {
    pub char_id: u32,
    pub name: String,
    pub job: String,
    pub me: bool,
    pub leader: bool,
    pub online: bool,
    /// Online, but on another map.
    pub far: bool,
    pub low: bool,
}

/// The party frames on the HUD.
#[derive(Clone, Debug, PartialEq)]
pub struct FramesView {
    pub name: String,
    pub online: String,
    pub even_share: bool,
    pub collapsed: bool,
    pub frames: Vec<FrameView>,
}

pub fn frames(roster: Roster, ui: &PartyUi) -> FramesView {
    let party = roster.party;
    let my_map = roster.my_map();
    FramesView {
        name: party.name.clone(),
        online: format!("{} / {} online", roster.online(), party.members.len()),
        even_share: party.exp_share,
        collapsed: ui.frames_collapsed,
        frames: party
            .members
            .iter()
            .map(|member| FrameView {
                char_id: member.char_id,
                name: member.name.clone(),
                job: job_name(roster.jobs, member.job_id),
                me: member.char_id == roster.me,
                leader: member.char_id == party.leader_char_id,
                online: member.online,
                far: member.online && member.map != my_map,
                low: Roster::low(member),
            })
            .collect(),
    }
}

/// Which live value of a member a node shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadoutPart {
    #[default]
    HpFill,
    SpFill,
    HpText,
    SpText,
    /// A card's place line: the map, and the distance while in view.
    Where,
    /// A frame's place line.
    FrameWhere,
    /// A frame's status tag.
    Tag,
}

/// A node that follows one live value of a party member.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemberReadout {
    pub char_id: u32,
    pub part: ReadoutPart,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReadoutValue {
    /// A bar's fill, in percent.
    Fill(f32),
    Text(String, Color),
}

/// The current value of `part` for `member`. `cells` is the distance to the member
/// while its character is in view.
pub fn readout(
    part: ReadoutPart,
    member: &PartyMemberInfo,
    roster: Roster,
    cells: Option<u32>,
) -> ReadoutValue {
    let me = member.char_id == roster.me;
    let here = member.map == roster.my_map();
    let amount = |current, max| format!("{} / {}", thousands(current), thousands(max));
    match part {
        ReadoutPart::HpFill => ReadoutValue::Fill(percent(member.hp, member.max_hp)),
        ReadoutPart::SpFill => ReadoutValue::Fill(percent(member.sp, member.max_sp)),
        ReadoutPart::HpText => ReadoutValue::Text(amount(member.hp, member.max_hp), ro_dark::VALUE),
        ReadoutPart::SpText => ReadoutValue::Text(amount(member.sp, member.max_sp), ro_dark::VALUE),
        ReadoutPart::Where => match (me, here, cells) {
            (true, ..) | (_, true, None) => {
                ReadoutValue::Text(member.map.clone(), ro_dark::STATUS_ONLINE)
            }
            (_, true, Some(cells)) => {
                ReadoutValue::Text(format!("Here \u{b7} {cells} cells"), ro_dark::STATUS_ONLINE)
            }
            (_, false, _) => ReadoutValue::Text(member.map.clone(), ro_dark::STATUS_BUSY),
        },
        ReadoutPart::FrameWhere => {
            let place = match cells.filter(|_| !me) {
                Some(cells) => format!("{cells} cells"),
                None => member.map.clone(),
            };
            ReadoutValue::Text(place, ro_dark::MUTED)
        }
        ReadoutPart::Tag if Roster::low(member) => {
            ReadoutValue::Text("Low HP".into(), ro_dark::STATUS_FULL)
        }
        ReadoutPart::Tag if !here => ReadoutValue::Text("Away".into(), ro_dark::STATUS_BUSY),
        ReadoutPart::Tag if me => ReadoutValue::Text("You".into(), ro_dark::FROST),
        ReadoutPart::Tag => ReadoutValue::Text(
            format!("{:.0}%", percent(member.hp, member.max_hp)),
            ro_dark::LABEL,
        ),
    }
}

/// One line of the invitation's roster.
#[derive(Clone, Debug, PartialEq)]
pub struct InviteMember {
    pub name: String,
    /// Job and level.
    pub meta: String,
    pub online: bool,
    pub leader: bool,
}

/// A party invitation as its dialog shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct InviteView {
    pub inviter: String,
    pub party: String,
    /// The inviter's job, when the roster names them.
    pub job: Option<String>,
    /// What else is known about the inviter, joined by dots.
    pub about: String,
    pub count: String,
    pub members: Vec<InviteMember>,
    pub exp: &'static str,
    pub pickup: &'static str,
    /// The headline and the consequence of a level gap that turns Even Share off.
    pub warning: Option<(String, String)>,
}

/// `my_level` is the local player's base level, `guild_mate` whether the inviter shares
/// their guild and `cells` the distance to the inviter while in view.
pub fn invite(
    invite: &PartyInviteNotified,
    jobs: Option<&JobSpriteRegistry>,
    my_level: Option<u32>,
    guild_mate: bool,
    cells: Option<u32>,
) -> InviteView {
    let inviter = invite
        .members
        .iter()
        .find(|m| m.char_id == invite.inviter_char_id);
    let mut about = Vec::new();
    if let Some(inviter) = inviter {
        about.push(format!("Base Lv {}", inviter.base_level));
    }
    if invite.inviter_char_id != 0 && invite.inviter_char_id == invite.leader_char_id {
        about.push("Party leader".into());
    }
    if guild_mate {
        about.push("Guild mate".into());
    }
    if let Some(cells) = cells {
        about.push(format!("{cells} cells away"));
    }
    let online = invite.members.iter().filter(|m| m.online).count();
    InviteView {
        inviter: invite.inviter_name.clone(),
        party: invite.party_name.clone(),
        job: inviter.map(|m| job_name(jobs, m.job_id)),
        about: about.join(" \u{b7} "),
        count: format!(
            "{} / {PARTY_MAX} \u{b7} {online} online",
            invite.members.len()
        ),
        members: invite
            .members
            .iter()
            .map(|member| InviteMember {
                name: member.name.clone(),
                meta: format!(
                    "{} {}{}",
                    job_name(jobs, member.job_id),
                    member.base_level,
                    if member.online { "" } else { " \u{b7} Offline" }
                ),
                online: member.online,
                leader: member.char_id == invite.leader_char_id,
            })
            .collect(),
        exp: if invite.exp_share {
            "Even Share"
        } else {
            "Each Take"
        },
        pickup: if invite.item_pickup_share {
            "Party Share"
        } else {
            "Each Take"
        },
        warning: my_level
            .filter(|_| invite.exp_share)
            .and_then(|level| gap_warning(level, &invite.members)),
    }
}

/// Names the online member furthest from `level` when joining would stretch the party
/// past [`SHARE_GAP`].
fn gap_warning(level: u32, members: &[PartyMemberInfo]) -> Option<(String, String)> {
    let far = members
        .iter()
        .filter(|m| m.online)
        .max_by_key(|m| m.base_level.abs_diff(level))?;
    let (min, max) = level_range(online_levels(members).chain([level]))?;
    if max - min <= SHARE_GAP {
        return None;
    }
    let side = if far.base_level > level {
        "below"
    } else {
        "above"
    };
    Some((
        format!(
            "You\u{2019}re Lv {level}, {} {side} {} ({}).",
            far.base_level.abs_diff(level),
            far.name,
            far.base_level
        ),
        format!(
            "Even Share allows a {SHARE_GAP}-level gap, so the party will switch to Each Take when you join."
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(char_id: u32, name: &str, level: u32) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: name.into(),
            base_level: level,
            online: true,
            map: "prontera".into(),
            job_id: 0,
            hp: 80,
            max_hp: 100,
            sp: 30,
            max_sp: 60,
            ap: 0,
            max_ap: 0,
        }
    }

    fn party(members: Vec<PartyMemberInfo>) -> PartyState {
        PartyState {
            party_id: 7,
            name: "Wolfpack".into(),
            leader_char_id: 1,
            members,
            ..default()
        }
    }

    fn roster(party: &PartyState, me: u32) -> Roster<'_> {
        Roster {
            party,
            me,
            jobs: None,
        }
    }

    #[test]
    fn head_and_form_follow_membership_and_leadership() {
        let none = PartyState::default();
        assert_eq!(head(roster(&none, 1)).count, None);
        assert_eq!(form(roster(&none, 1)), FormView::Create);

        let mut offline = member(2, "Freyja", 50);
        offline.online = false;
        let party = party(vec![member(1, "Ygor", 50), offline]);
        let head = head(roster(&party, 2));
        assert_eq!(head.title, "Wolfpack");
        assert_eq!(
            head.count,
            Some((
                "2 / 12 members \u{b7} 1 online \u{b7} Led by ".into(),
                "Ygor".into()
            ))
        );
        assert_eq!(form(roster(&party, 1)), FormView::Invite { leader: true });
        assert_eq!(form(roster(&party, 2)), FormView::Invite { leader: false });
    }

    #[test]
    fn selection_actions_follow_leadership_presence_and_self() {
        let mut offline = member(3, "Ingrid", 50);
        offline.online = false;
        let party = party(vec![
            member(1, "Ygor", 50),
            member(2, "Freyja", 50),
            offline,
        ]);
        let select = |me, selected, in_view: bool, trade_free| {
            let ui = PartyUi {
                selected: Some(selected),
                ..default()
            };
            body(roster(&party, me), &ui, |_| in_view, trade_free)
                .selection
                .unwrap()
        };

        let other = select(1, 2, true, true);
        assert_eq!(other.title, "Freyja");
        assert!(other.can_trade && other.can_lead && other.can_expel);
        assert!(!select(1, 2, false, true).can_trade, "out of view");
        assert!(!select(1, 2, true, false).can_trade, "already trading");

        let myself = select(1, 1, true, true);
        assert_eq!(myself.title, "Ygor (you) \u{b7} Leader");
        assert!(!myself.can_trade && !myself.can_lead && !myself.can_expel);

        let gone = select(1, 3, true, true);
        assert!(gone.meta.ends_with("Offline"));
        assert!(!gone.can_trade && !gone.can_lead && gone.can_expel);

        let as_member = select(2, 1, true, true);
        assert!(as_member.can_trade && !as_member.can_lead && !as_member.can_expel);
    }

    #[test]
    fn a_selection_that_left_the_party_shows_nothing() {
        let party = party(vec![member(1, "Ygor", 50)]);
        let ui = PartyUi {
            selected: Some(9),
            ..default()
        };
        let view = body(roster(&party, 1), &ui, |_| true, true);
        assert_eq!(view.selection, None);
        assert_eq!(view.open_slots, 7);
    }

    #[test]
    fn even_share_needs_online_levels_within_the_gap() {
        let mut far = member(2, "Orm", 30);
        let mut state = party(vec![member(1, "Ygor", 50), far.clone()]);
        let view = body(roster(&state, 1), &PartyUi::default(), |_| true, true);
        assert!(!view.sharing.even_ok);
        assert!(view.sharing.exp_warn);
        assert!(view.sharing.exp_note.contains("20 apart"));
        assert_eq!((view.levels.lo, view.levels.hi), (15, 65));
        assert_eq!(view.levels.ticks, vec![(70.0, true), (30.0, false)]);
        assert_eq!(view.levels.band, (30.0, 40.0));

        // An offline member's level does not count.
        far.online = false;
        state.members[1] = far;
        let view = body(roster(&state, 1), &PartyUi::default(), |_| true, true);
        assert!(view.sharing.even_ok);
        assert!(!view.sharing.exp_warn);
    }

    #[test]
    fn leaving_as_leader_warns_about_the_disband() {
        let party = party(vec![member(1, "Ygor", 50), member(2, "Freyja", 50)]);
        let prompt = |me| {
            body(roster(&party, me), &PartyUi::default(), |_| true, true)
                .leave
                .prompt
        };
        assert_eq!(prompt(1), "Leaving disbands Wolfpack.");
        assert_eq!(prompt(2), "Leave Wolfpack?");
    }

    #[test]
    fn frames_flag_far_low_and_offline_members() {
        let mut away = member(2, "Leif", 50);
        away.map = "glast_01".into();
        let mut hurt = member(3, "Sigrun", 50);
        hurt.hp = 29;
        let mut offline = member(4, "Ingrid", 50);
        offline.online = false;
        offline.hp = 0;
        let party = party(vec![member(1, "Ygor", 50), away, hurt, offline]);
        let view = frames(roster(&party, 1), &PartyUi::default());

        assert_eq!(view.online, "3 / 4 online");
        let flags: Vec<_> = view.frames.iter().map(|f| (f.far, f.low)).collect();
        assert_eq!(
            flags,
            vec![(false, false), (true, false), (false, true), (false, false)]
        );
    }

    #[test]
    fn readouts_follow_vitals_and_position() {
        let mut away = member(2, "Leif", 50);
        away.map = "glast_01".into();
        let mut hurt = member(3, "Sigrun", 50);
        hurt.hp = 2_900;
        hurt.max_hp = 9_800;
        let party = party(vec![
            member(1, "Ygor", 50),
            away,
            hurt,
            member(4, "Bjorn", 50),
        ]);
        let roster = roster(&party, 1);
        let text =
            |part, index: usize, cells| match readout(part, &party.members[index], roster, cells) {
                ReadoutValue::Text(text, _) => text,
                ReadoutValue::Fill(fill) => panic!("expected text, got fill {fill}"),
            };

        assert_eq!(
            readout(ReadoutPart::HpFill, &party.members[0], roster, None),
            ReadoutValue::Fill(80.0)
        );
        assert_eq!(text(ReadoutPart::HpText, 2, None), "2,900 / 9,800");
        assert_eq!(text(ReadoutPart::Where, 0, Some(0)), "prontera");
        assert_eq!(text(ReadoutPart::Where, 3, Some(5)), "Here \u{b7} 5 cells");
        assert_eq!(text(ReadoutPart::Where, 3, None), "prontera");
        assert_eq!(text(ReadoutPart::Where, 1, None), "glast_01");
        assert_eq!(text(ReadoutPart::FrameWhere, 3, Some(5)), "5 cells");
        assert_eq!(text(ReadoutPart::FrameWhere, 1, None), "glast_01");
        assert_eq!(text(ReadoutPart::Tag, 0, None), "You");
        assert_eq!(text(ReadoutPart::Tag, 1, None), "Away");
        assert_eq!(text(ReadoutPart::Tag, 2, None), "Low HP");
        assert_eq!(text(ReadoutPart::Tag, 3, None), "80%");
    }

    #[test]
    fn a_bar_with_no_maximum_is_empty_and_an_overfull_one_is_capped() {
        assert_eq!(percent(10, 0), 0.0);
        assert_eq!(percent(150, 100), 100.0);
        let large = u64::from(u32::MAX) + 1;
        assert_eq!(percent(large, large * 2), 50.0);
    }

    fn invitation(members: Vec<PartyMemberInfo>) -> PartyInviteNotified {
        PartyInviteNotified {
            party_id: 7,
            party_name: "Wolfpack".into(),
            inviter_name: "Ygor".into(),
            inviter_char_id: 1,
            leader_char_id: 1,
            exp_share: true,
            item_pickup_share: false,
            members,
        }
    }

    #[test]
    fn invite_describes_the_inviter_and_the_party() {
        let mut offline = member(2, "Ingrid", 190);
        offline.online = false;
        let invite_msg = invitation(vec![member(1, "Ygor", 215), offline]);
        let view = invite(&invite_msg, None, Some(210), true, Some(7));

        assert_eq!(view.job.as_deref(), Some("Unknown Job"));
        assert_eq!(
            view.about,
            "Base Lv 215 \u{b7} Party leader \u{b7} Guild mate \u{b7} 7 cells away"
        );
        assert_eq!(view.count, "2 / 12 \u{b7} 1 online");
        assert!(view.members[0].leader && view.members[0].online);
        assert_eq!(view.members[1].meta, "Unknown Job 190 \u{b7} Offline");
        assert_eq!((view.exp, view.pickup), ("Even Share", "Each Take"));
        assert_eq!(view.warning, None, "5 levels from the only online member");
    }

    #[test]
    fn invite_warns_when_joining_breaks_even_share() {
        let members = vec![member(1, "Ygor", 215), member(2, "Bjorn", 220)];
        let warning = |level, exp_share| {
            let invite_msg = PartyInviteNotified {
                exp_share,
                ..invitation(members.clone())
            };
            invite(&invite_msg, None, level, false, None).warning
        };

        let (headline, _) = warning(Some(184), true).unwrap();
        assert_eq!(headline, "You\u{2019}re Lv 184, 36 below Bjorn (220).");
        let (headline, _) = warning(Some(240), true).unwrap();
        assert_eq!(headline, "You\u{2019}re Lv 240, 25 above Ygor (215).");
        assert_eq!(warning(Some(210), true), None);
        assert_eq!(warning(Some(184), false), None, "nothing to lose");
        assert_eq!(warning(None, true), None, "own level unknown");
    }

    #[test]
    fn an_invite_without_a_roster_still_names_the_party() {
        let view = invite(&invitation(Vec::new()), None, Some(50), false, None);
        assert_eq!(view.job, None);
        assert_eq!(view.about, "Party leader");
        assert_eq!(view.warning, None);
    }
}
