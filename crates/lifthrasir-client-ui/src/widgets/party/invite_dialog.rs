//! The party invitation: who invites, the party they invite to, and thirty seconds to
//! answer, after the `Party Invitation` artboard.
//!
//! An inbound [`PartyInviteNotified`] is projected once into [`PendingPartyInvite`];
//! `rebuild_party_ui` shows the dialog for as long as it is pending. Join Party,
//! Decline, `/paccept`, `/pdecline` and the 30 s expiry all end it by clearing the
//! resource. The dialog does not block the game behind it.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::guild::GuildState;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, WINDOW_SHADOW, diamond, inset, window_title, window_titled,
};
use net_contract::events::PartyInviteNotified;

use super::view::{self, InviteMember, InviteView};
use super::{PartyAction, PartyData, PartyHost, on_party_button};
#[cfg(test)]
use crate::screens::shared::InviteClock;
use crate::screens::shared::{
    InviteClockNode, about_line, colored_lines, disabled_checkbox, invite_clock, overline,
    tick_invite_clock, wide_button,
};
use crate::widgets::system_dialog;

/// The server declines for the invitee after this; the client mirrors it so a dead
/// invitation never lingers.
const INVITE_TTL_SECS: f32 = 30.0;
/// Under the death and system dialogs, so a disconnect notice stays on top.
const DIALOG_Z: i32 = i32::MAX - 4;

const _: () = assert!(
    DIALOG_Z < system_dialog::DIALOG_Z,
    "the party invitation must stack under the system dialog"
);

const WIDTH: f32 = 524.0;
/// Width of one of the roster's two columns.
const ROSTER_COLUMN: f32 = 217.0;

/// The invitation waiting for an answer.
#[derive(Resource, Default)]
pub struct PendingPartyInvite {
    /// `0` while nothing is pending.
    pub party_id: u32,
    view: Option<InviteView>,
    timer: Timer,
}

impl PendingPartyInvite {
    pub fn is_pending(&self) -> bool {
        self.party_id != 0
    }

    pub(super) fn view(&self) -> Option<&InviteView> {
        self.view.as_ref()
    }

    fn set(&mut self, party_id: u32, view: InviteView) {
        self.party_id = party_id;
        self.view = Some(view);
        self.timer = Timer::from_seconds(INVITE_TTL_SECS, TimerMode::Once);
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Records the latest invitation. The server keeps one live invitation per player, so
/// a newer one replaces whatever was pending.
pub fn receive_invite(
    mut invites: MessageReader<PartyInviteNotified>,
    mut pending: ResMut<PendingPartyInvite>,
    data: PartyData,
    guild: Res<GuildState>,
    statuses: Query<&CharacterStatus, With<LocalPlayer>>,
) {
    let Some(invite) = invites.read().last() else {
        return;
    };
    let view = view::invite(
        invite,
        data.jobs.as_deref(),
        statuses.single().ok().map(|status| status.base_level),
        guild.member(invite.inviter_char_id).is_some(),
        data.cells(invite.inviter_char_id),
    );
    pending.set(invite.party_id, view);
}

/// Drops the invitation once its time is up.
pub fn expire_pending_invite(time: Res<Time>, mut pending: ResMut<PendingPartyInvite>) {
    if !pending.is_pending() {
        return;
    }
    if pending.timer.tick(time.delta()).just_finished() {
        pending.clear();
    }
}

/// Marks the party invitation's countdown, apart from any other invitation on screen.
#[derive(Component, Default, Clone)]
pub struct PartyClock;

/// Drains the countdown bar and counts the seconds down beside it.
pub fn sync_invite_clock(
    pending: Res<PendingPartyInvite>,
    mut clocks: Query<InviteClockNode, With<PartyClock>>,
) {
    if pending.is_pending() {
        tick_invite_clock(&pending.timer, &mut clocks);
    }
}

fn clock() -> impl Scene {
    invite_clock::<PartyClock>()
}

/// The screen-wide region the dialog is centred in. It lets every click through.
pub(super) fn host() -> impl Scene {
    bsn! {
        PartyHost::Invite
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::FlexStart,
            padding: {UiRect::top(px(96))},
        }
        GlobalZIndex({DIALOG_Z})
        @ignore_picking()
    }
}

pub(super) fn dialog(view: InviteView) -> impl Scene {
    let about = about_line(view.job, view.about);
    let roster = (!view.members.is_empty()).then(|| EntityScene(roster(view.count, view.members)));
    let warning = view
        .warning
        .map(|warning| EntityScene(gap_warning(warning)));
    bsn! {
        @window_titled()
        Node {
            width: px(WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Pickable
        Children [
            @window_title("Party Invitation")
            --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                margin: {UiRect::new(px(0), px(0), px(10), px(6))},
            }
            @ignore_picking()
            Children [
                @sentence(view.inviter, view.party)
                --
                {about}
                --
                {roster}
                --
                Node { column_gap: px(12) }
                @ignore_picking()
                Children [
                    @setting("EXP", view.exp)
                    --
                    @setting("Item pickup", view.pickup)
                ]
                --
                {warning}
                --
                @clock()
                --
                @friends_only()
                --
                Node {
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(12),
                    margin: {UiRect::top(px(6))},
                }
                @ignore_picking()
                Children [
                    @wide_button(ButtonArt::NORMAL, "Decline", 96.0)
                    PartyAction::AnswerInvite(false)
                    on(on_party_button)
                    --
                    @wide_button(ButtonArt::PRIMARY, "Join Party", 110.0)
                    PartyAction::AnswerInvite(true)
                    on(on_party_button)
                ]
            ]
        ]
    }
}

/// "`inviter` invites you to join `party`.", with the party's name in the title font.
fn sentence(inviter: String, party: String) -> impl Scene {
    bsn! {
        Text(inviter)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(14.0)},
        }
        TextColor(ro_dark::HERO)
        @ignore_picking()
        Children [
            @run(" invites you to join ".into(), theme::FONT_BODY, ro_dark::BODY)
            --
            @run(party, theme::FONT_TITLE, ro_dark::HERO)
            --
            @run(".".into(), theme::FONT_BODY, ro_dark::BODY)
        ]
    }
}

fn run(text: String, font: &'static str, color: Color) -> impl Scene {
    bsn! {
        TextSpan(text)
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: {FontSize::Px(14.0)},
        }
        TextColor(color)
    }
}

fn roster(count: String, members: Vec<InviteMember>) -> impl Scene {
    let rows: Vec<_> = members.into_iter().map(roster_row).collect();
    bsn! {
        @inset()
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::axes(px(12), px(10))},
        }
        @ignore_picking()
        Children [
            Node {
                align_items: AlignItems::Baseline,
                justify_content: JustifyContent::SpaceBetween,
            }
            @ignore_picking()
            Children [
                @overline("MEMBERS".into(), ro_dark::ACCENT)
                --
                @chrome_text(count, 11.0, ro_dark::MUTED)
            ]
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(16), row_gap: px(2) }
            @ignore_picking()
            Children [ {rows} ]
        ]
    }
}

fn roster_row(member: InviteMember) -> impl Scene {
    let crown = member
        .leader
        .then(|| EntityScene(diamond(8.0, ro_dark::RARE_RING, ro_dark::RARE_RING)));
    let (dot, ink) = if member.online {
        (ro_dark::JOB_EXP, ro_dark::STRONG)
    } else {
        (ro_dark::SLOT_STROKE, ro_dark::MUTED)
    };
    bsn! {
        Node {
            width: px(ROSTER_COLUMN),
            height: px(20),
            align_items: AlignItems::Center,
            column_gap: px(7),
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [
            Node { width: px(7), height: px(7), flex_shrink: 0.0, border_radius: BorderRadius::MAX }
            BackgroundColor(dot)
            @ignore_picking()
            --
            {crown}
            --
            @chrome_text(member.name, 12.0, ink)
            TextLayout { linebreak: LineBreak::NoWrap }
            Node { flex_shrink: 0.0 }
            --
            @chrome_text(member.meta, 12.0, ro_dark::MUTED)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    }
}

/// One of the party's sharing options: its name over its current choice.
fn setting(label: &'static str, value: &'static str) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: px(0) }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
            --
            @chrome_text(value.into(), 13.0, ro_dark::STRONG)
        ]
    }
}

/// The note that joining will turn Even Share off.
fn gap_warning((headline, consequence): (String, String)) -> impl Scene {
    let fill = ro_dark::STATUS_BUSY.with_alpha(0.1);
    let stroke = ro_dark::STATUS_BUSY.with_alpha(0.45);
    let line = vec![vec![(
        ro_dark::BRONZE_LIGHT,
        format!("{headline} {consequence}"),
    )]];
    bsn! {
        Node {
            align_items: AlignItems::FlexStart,
            column_gap: px(10),
            padding: {UiRect::axes(px(12), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        @ignore_picking()
        Children [
            @diamond(8.0, ro_dark::STATUS_BUSY, ro_dark::STATUS_BUSY)
            Node { margin: {UiRect::top(px(4))} }
            --
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            @ignore_picking()
            Children [ @colored_lines(line) ]
        ]
    }
}

/// NOTE: drawn disabled, because the client has no friends list and the server no such
/// filter. Make it an `ArtCheckbox` once invitations can be filtered.
fn friends_only() -> impl Scene {
    disabled_checkbox("Only accept party invites from friends and guild mates")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::entities::EntityRegistry;
    use lifthrasir_client::domain::party::PartyState;
    use net_contract::state::ZoneSession;
    use std::time::Duration;

    fn invite(party_id: u32, party_name: &str) -> PartyInviteNotified {
        PartyInviteNotified {
            party_id,
            party_name: party_name.into(),
            inviter_name: "Odin".into(),
            ..default()
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<PartyInviteNotified>()
            .init_resource::<Time>()
            .init_resource::<PendingPartyInvite>()
            .init_resource::<PartyState>()
            .init_resource::<ZoneSession>()
            .init_resource::<EntityRegistry>()
            .init_resource::<GuildState>()
            .add_systems(Update, (receive_invite, expire_pending_invite).chain());
        app
    }

    fn send(app: &mut App, invite: PartyInviteNotified) {
        app.world_mut()
            .resource_mut::<Messages<PartyInviteNotified>>()
            .write(invite);
    }

    #[test]
    fn an_invite_becomes_pending_and_a_newer_one_replaces_it() {
        let mut app = app();
        send(&mut app, invite(42, "Wolfpack"));
        app.update();

        let pending = app.world().resource::<PendingPartyInvite>();
        assert_eq!(pending.party_id, 42);
        assert_eq!(pending.view().unwrap().party, "Wolfpack");
        assert_eq!(pending.view().unwrap().inviter, "Odin");

        send(&mut app, invite(43, "Ravens"));
        app.update();

        let pending = app.world().resource::<PendingPartyInvite>();
        assert_eq!(pending.party_id, 43);
        assert_eq!(pending.view().unwrap().party, "Ravens");
    }

    #[test]
    fn a_pending_invite_expires_after_its_time() {
        let mut app = app();
        send(&mut app, invite(42, "Wolfpack"));
        app.update();
        assert!(app.world().resource::<PendingPartyInvite>().is_pending());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));
        app.update();

        let pending = app.world().resource::<PendingPartyInvite>();
        assert!(!pending.is_pending());
        assert!(pending.view().is_none());
    }

    #[test]
    fn the_clock_counts_down_and_turns_red_at_the_end() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<PendingPartyInvite>();
        app.add_systems(Update, sync_invite_clock);
        app.world_mut().spawn_scene(clock()).unwrap();
        app.world_mut().resource_mut::<PendingPartyInvite>().set(
            42,
            view::invite(&invite(42, "Wolfpack"), None, None, false, None),
        );
        let label = |app: &mut App| {
            let world = app.world_mut();
            let (clock_text, color) = world
                .query::<(&InviteClock, &Text, &TextColor)>()
                .iter(world)
                .find(|(clock, ..)| **clock == InviteClock::Label)
                .map(|(_, text, color)| (text.0.clone(), color.0))
                .unwrap();
            (clock_text, color)
        };

        app.update();
        assert_eq!(label(&mut app), ("Expires in 30 s".into(), ro_dark::MUTED));

        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .timer
            .tick(Duration::from_secs_f32(26.5));
        app.update();
        assert_eq!(
            label(&mut app),
            ("Expires in 4 s".into(), ro_dark::STATUS_FULL)
        );
    }

    #[test]
    fn the_dialog_shows_the_roster_and_the_level_warning() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        let view = InviteView {
            inviter: "Ygor".into(),
            party: "Wolfpack".into(),
            job: Some("Dragon Knight".into()),
            about: "Base Lv 215".into(),
            count: "1 / 12 \u{b7} 1 online".into(),
            members: vec![InviteMember {
                name: "Ygor".into(),
                meta: "Dragon Knight 215".into(),
                online: true,
                leader: true,
            }],
            exp: "Even Share",
            pickup: "Each Take",
            warning: Some(("Too far apart.".into(), "EXP will switch.".into())),
        };
        app.world_mut().spawn_scene(dialog(view)).unwrap();

        let world = app.world_mut();
        let texts: Vec<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        for expected in [
            "Dragon Knight 215",
            "Even Share",
            "Too far apart. EXP will switch.",
        ] {
            assert!(texts.iter().any(|text| text == expected), "{expected}");
        }
        let answers: Vec<_> = world.query::<&PartyAction>().iter(world).copied().collect();
        assert_eq!(
            answers,
            vec![
                PartyAction::AnswerInvite(false),
                PartyAction::AnswerInvite(true)
            ]
        );
    }
}
