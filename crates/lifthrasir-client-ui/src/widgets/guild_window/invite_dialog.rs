//! The guild invitation: who invites, the guild they invite to, and thirty seconds to
//! answer, after the `Guild Invitation` artboard.
//!
//! An inbound invitation is projected once into [`PendingGuildInvite`];
//! `rebuild_guild_ui` shows the dialog for as long as it is pending. Join Guild,
//! Decline and the 30 s expiry all end it. An accepted one turns into the welcome
//! once the guild's snapshot arrives. The dialog does not block the game behind it.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, WINDOW_SHADOW, close_button, diamond, display_text, inset, window_title,
    window_title_bar, window_titled,
};
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use super::view::{self, InviteView, WelcomeView};
use super::{GuildAction, GuildHost, GuildUiSession, on_guild_button, scene};
use crate::screens::shared::{
    InviteClockNode, about_line, colored_lines, disabled_checkbox, invite_clock, overline,
    tick_invite_clock, wide_button,
};
use crate::widgets::system_dialog;

/// The server drops the invitation after this; the client mirrors it so a dead
/// invitation never lingers.
const INVITE_TTL_SECS: f32 = 30.0;
/// Under the death and system dialogs, so a disconnect notice stays on top.
const DIALOG_Z: i32 = i32::MAX - 4;

const _: () = assert!(
    DIALOG_Z < system_dialog::DIALOG_Z,
    "the guild invitation must stack under the system dialog"
);

const WIDTH: f32 = 544.0;

/// The invitation waiting for an answer.
#[derive(Resource, Default)]
pub struct PendingGuildInvite {
    /// `0` while nothing is pending.
    pub guild_id: u32,
    view: Option<InviteView>,
    timer: Timer,
    /// The guild whose invitation was accepted, until the welcome is dismissed.
    joined: u32,
}

impl PendingGuildInvite {
    pub fn is_pending(&self) -> bool {
        self.guild_id != 0
    }

    pub(super) fn view(&self) -> Option<&InviteView> {
        self.view.as_ref()
    }

    /// The guild the player accepted into and has not been welcomed to yet.
    pub(super) fn joined(&self) -> u32 {
        self.joined
    }

    fn set(&mut self, guild_id: u32, view: InviteView) {
        *self = Self {
            guild_id,
            view: Some(view),
            timer: Timer::from_seconds(INVITE_TTL_SECS, TimerMode::Once),
            joined: 0,
        };
    }

    /// Ends the invitation as accepted, so the welcome can follow.
    pub(super) fn accept(&mut self) {
        *self = Self {
            joined: self.guild_id,
            ..default()
        };
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Marks the guild invitation's countdown, apart from any other invitation on screen.
#[derive(Component, Default, Clone)]
pub struct GuildClock;

/// Records the latest invitation of this session. The server keeps one live
/// invitation per player, so a newer one replaces whatever was pending.
pub(super) fn receive_invite(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    session: Res<GuildUiSession>,
    jobs: Option<Res<JobSpriteRegistry>>,
    mut pending: ResMut<PendingGuildInvite>,
) {
    if session.blocked {
        ingress.clear();
        return;
    }
    let newest = ingress
        .read()
        .filter(|event| event.generation == *generation)
        .filter_map(|event| match &event.payload {
            GuildIngressPayload::InviteNotified(invite) => Some(invite),
            _ => None,
        })
        .last();
    if let Some(invite) = newest {
        pending.set(invite.guild_id, view::invite(invite, jobs.as_deref()));
    }
}

/// Drops the invitation once its time is up.
pub(super) fn expire_pending_invite(time: Res<Time>, mut pending: ResMut<PendingGuildInvite>) {
    if !pending.is_pending() {
        return;
    }
    if pending.timer.tick(time.delta()).just_finished() {
        pending.clear();
    }
}

pub(super) fn clear_pending_invite(mut pending: ResMut<PendingGuildInvite>) {
    pending.clear();
}

/// Drains the countdown bar and counts the seconds down beside it.
pub(super) fn sync_invite_clock(
    pending: Res<PendingGuildInvite>,
    mut clocks: Query<InviteClockNode, With<GuildClock>>,
) {
    if pending.is_pending() {
        tick_invite_clock(&pending.timer, &mut clocks);
    }
}

/// The screen-wide region the dialog is centred in. It lets every click through.
pub(super) fn host() -> impl Scene {
    bsn! {
        GuildHost::Invite
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

/// The dialog's frame; the caller adds the title row and the content.
fn frame() -> impl Scene {
    bsn! {
        @window_titled()
        Node {
            width: px(WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Pickable
    }
}

fn content() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            margin: {UiRect::new(px(0), px(0), px(10), px(6))},
        }
        @ignore_picking()
    }
}

pub(super) fn dialog(view: InviteView) -> impl Scene {
    let about = about_line(view.job, view.about);
    let (castle, castle_tone) = match view.castle {
        Some(castle) => (castle, ro_dark::BRONZE_LIGHT),
        None => ("None".to_string(), ro_dark::MUTED),
    };
    let sub = vec![vec![
        (
            ro_dark::MUTED,
            format!("Guild Lv {} \u{b7} Master ", view.level),
        ),
        (ro_dark::BRONZE_LIGHT, view.master),
    ]];
    bsn! {
        @frame()
        Children [
            @window_title("Guild Invitation")
            --
            @content()
            Children [
                @sentence(view.inviter)
                --
                {about}
                --
                @inset()
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: {UiRect::axes(px(14), px(12))},
                }
                @ignore_picking()
                Children [
                    @scene::emblem(view.guild_id, view.emblem_id, 72.0)
                    --
                    Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                    }
                    @ignore_picking()
                    Children [
                        @display_text(view.guild, 20.0, ro_dark::HERO)
                        --
                        @colored_lines(sub)
                        --
                        Node { column_gap: px(12), margin: {UiRect::top(px(4))} }
                        @ignore_picking()
                        Children [
                            @fact("Members", view.members, ro_dark::STRONG)
                            --
                            @fact("Online now", view.online, ro_dark::STATUS_ONLINE)
                            --
                            @fact("Castle", castle, castle_tone)
                        ]
                    ]
                ]
                --
                Node { column_gap: px(16) }
                @ignore_picking()
                Children [
                    @term("You\u{2019}d join as", view.join_as, ro_dark::STRONG)
                    --
                    @term("EXP tax", view.tax, ro_dark::BRONZE_LIGHT)
                ]
                --
                @term("Allies", view.allies, ro_dark::FROST)
                --
                @term("Antagonists", view.antagonists, ro_dark::STATUS_FULL)
                --
                @invite_clock::<GuildClock>()
                --
                @strangers()
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(12),
                    margin: {UiRect::top(px(6))},
                }
                @ignore_picking()
                Children [
                    Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                    @ignore_picking()
                    Children [
                        @chrome_text(
                            "You\u{2019}re not in a guild, so nothing else changes.".into(),
                            11.0,
                            ro_dark::MUTED,
                        )
                    ]
                    --
                    @wide_button(ButtonArt::NORMAL, "Decline", 96.0)
                    GuildAction::AnswerInvite(false)
                    on(on_guild_button)
                    --
                    @wide_button(ButtonArt::PRIMARY, "Join Guild", 110.0)
                    GuildAction::AnswerInvite(true)
                    on(on_guild_button)
                ]
            ]
        ]
    }
}

/// "`inviter` invites you to join their guild."
fn sentence(inviter: String) -> impl Scene {
    bsn! {
        Text(inviter)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(14.0)},
        }
        TextColor(ro_dark::HERO)
        @ignore_picking()
        Children [
            TextSpan(" invites you to join their guild.")
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_BODY),
                font_size: {FontSize::Px(14.0)},
            }
            TextColor(ro_dark::BODY)
        ]
    }
}

/// One figure of the guild card: its name over its value.
fn fact(label: &'static str, value: String, tone: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
            --
            @chrome_text(value, 13.0, tone)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    }
}

/// A term on the left of its row and its value on the right.
fn term(label: &'static str, value: String, tone: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(12),
        }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(value, 12.0, tone)
        ]
    }
}

/// NOTE: drawn disabled, because the client has no friends list and the server no such
/// filter. Make it an `ArtCheckbox` once invitations can be filtered.
fn strangers() -> impl Scene {
    disabled_checkbox("Don\u{2019}t show guild invites from heroes I don\u{2019}t know")
}

/// What the dialog becomes once the player is in the guild: a greeting, the guild's
/// notice and the way into the guild window.
pub(super) fn welcome(view: WelcomeView) -> impl Scene {
    let notice = view.notice;
    let subject = if notice.subject.is_empty() && notice.body.is_empty() {
        "No guild notice has been posted.".to_string()
    } else {
        notice.subject
    };
    let body = (!notice.body.is_empty())
        .then(|| EntityScene(chrome_text(notice.body, 12.0, ro_dark::BODY)));
    let by =
        (!notice.by.is_empty()).then(|| EntityScene(chrome_text(notice.by, 11.0, ro_dark::MUTED)));
    bsn! {
        @frame()
        Children [
            @window_title_bar("Guild Invitation", bsn! {
                @close_button()
                GuildAction::DismissInvite
                on(on_guild_button)
            })
            --
            @content()
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(10) }
                @ignore_picking()
                Children [
                    @diamond(10.0, ro_dark::STATUS_ONLINE, ro_dark::STATUS_ONLINE)
                    --
                    @display_text(format!("Welcome to {}.", view.guild), 20.0, ro_dark::HERO)
                ]
                --
                @chrome_text(
                    format!("You joined as {}. The guild has been told.", view.position),
                    13.0,
                    ro_dark::BODY,
                )
                --
                @inset()
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    padding: {UiRect::axes(px(14), px(12))},
                }
                @ignore_picking()
                Children [
                    @overline("GUILD NOTICE".into(), ro_dark::ACCENT)
                    --
                    @chrome_text(subject, 14.0, ro_dark::HERO)
                    --
                    {body}
                    --
                    {by}
                ]
                --
                Node { justify_content: JustifyContent::FlexEnd, margin: {UiRect::top(px(6))} }
                @ignore_picking()
                Children [
                    @wide_button(ButtonArt::PRIMARY, "Open Guild", 110.0)
                    GuildAction::OpenGuild
                    on(on_guild_button)
                ]
            ]
        ]
    }
}
