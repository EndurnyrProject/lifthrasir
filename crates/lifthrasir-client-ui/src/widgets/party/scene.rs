//! The party window scenes: the frame [`window`] spawns once and the three regions
//! `rebuild_party_ui` projects from their view-models. Sizes follow the `Party`
//! artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Button;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, HoverColors, INPUT_SHADOW, WINDOW_SHADOW, close_button, diamond, display_text,
    inset, segment, tab_text, window_title_bar, window_titled,
};

use super::view::{
    BodyView, CardView, FormView, HeadView, LeaveView, LevelsView, MemberReadout, ReadoutPart,
    SHARE_GAP, SelectionView, SharingView,
};
use super::{
    PARTY_MAX, PartyAction, PartyHost, PartyNameField, PartyTitlebar, PartyWindowRoot,
    on_party_button,
};
use crate::screens::shared::{
    DANGER_TINT, colored_lines, glow, overline, row_button, search_field, usable, wide_button,
};

const WINDOW_LEFT: f32 = 160.0;
const WINDOW_TOP: f32 = 40.0;
/// Width of the window's shape.
const WINDOW_WIDTH: f32 = 924.0;
/// Width of the roster column: two cards and the gap between them.
const ROSTER_WIDTH: f32 = 536.0;
const CARD_GAP: f32 = 8.0;
const CARD_WIDTH: f32 = (ROSTER_WIDTH - CARD_GAP) / 2.0;
const CARD_HEIGHT: f32 = 92.0;

/// The frame: the title bar, the heading beside the name field, and the body.
pub fn window() -> impl Scene {
    bsn! {
        PartyWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        Pickable
        Children [
            PartyTitlebar
            @window_title_bar("Party", bsn! {
                @close_button()
                PartyAction::CloseWindow
                on(on_party_button)
            })
            Pickable
            on(drag_window::<PartyTitlebar, PartyWindowRoot>)
            --
            Node {
                align_items: AlignItems::FlexStart,
                column_gap: px(14),
                margin: {UiRect::top(px(10))},
            }
            @ignore_picking()
            Children [
                PartyHost::Head
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                @ignore_picking()
                --
                PartyHost::Form
                Node { flex_shrink: 0.0 }
                @ignore_picking()
            ]
            --
            PartyHost::Body
            Node { margin: {UiRect::new(px(0), px(0), px(10), px(8))} }
            @ignore_picking()
        ]
    }
}

pub(super) fn head(view: HeadView) -> impl Scene {
    let line = match view.count {
        Some((counts, leader)) => vec![(ro_dark::MUTED, counts), (ro_dark::RARE_RING, leader)],
        None => vec![(ro_dark::MUTED, "Name a party to found one.".to_string())],
    };
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @display_text(view.title, 22.0, ro_dark::HERO)
            --
            @colored_lines(vec![line])
        ]
    }
}

/// The name field, its button and the rule under them.
pub(super) fn form(view: FormView) -> impl Scene {
    let (hint, label, width, enabled) = match view {
        FormView::Create => ("Party name", "Create Party", 112.0, true),
        FormView::Invite { leader } => ("Invite a hero by name", "Invite", 72.0, leader),
    };
    let note = match view {
        FormView::Create => format!("You become the leader \u{b7} Up to {PARTY_MAX} members"),
        FormView::Invite { leader: true } => format!(
            "Heroes must be online and not in another party \u{b7} Up to {PARTY_MAX} members"
        ),
        FormView::Invite { leader: false } => "Only the leader can invite".to_string(),
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(8),
        }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(14) }
            @ignore_picking()
            Children [
                @search_field::<PartyNameField>(hint)
                Node {
                    width: px(220.0 + 2.0 * INPUT_SHADOW),
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                }
                --
                @wide_button(ButtonArt::PRIMARY, label, width)
                @usable(enabled, Color::WHITE)
                PartyAction::Submit
                on(on_party_button)
            ]
            --
            @chrome_text(note, 11.0, ro_dark::MUTED)
        ]
    }
}

/// The roster with its selection bar, beside the sharing options, the level track and
/// Leave Party.
pub(super) fn body(view: BodyView) -> impl Scene {
    let open = (0..view.open_slots).map(|_| Box::new(open_slot()) as Box<dyn Scene>);
    let cards: Vec<Box<dyn Scene>> = view
        .cards
        .into_iter()
        .map(|card_view| Box::new(card(card_view)) as Box<dyn Scene>)
        .chain(open)
        .collect();
    let selection = selection_bar(view.selection);
    let leave = leave_controls(view.leave);
    bsn! {
        Node { column_gap: px(20), align_items: AlignItems::FlexStart }
        @ignore_picking()
        Children [
            Node {
                width: px(ROSTER_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                @overline("MEMBERS".into(), ro_dark::ACCENT)
                --
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(CARD_GAP),
                    row_gap: px(CARD_GAP),
                    margin: {UiRect::top(px(6))},
                }
                @ignore_picking()
                Children [ {cards} ]
                --
                @inset()
                Node {
                    height: px(52),
                    margin: {UiRect::top(px(10))},
                    padding: {UiRect::horizontal(px(12))},
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                }
                @ignore_picking()
                Children [ {selection} ]
            ]
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
            }
            @ignore_picking()
            Children [
                @sharing(view.sharing)
                --
                @levels(view.levels)
                --
                Node {
                    height: px(32),
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                }
                @ignore_picking()
                Children [ {leave} ]
            ]
        ]
    }
}

/// A node that follows one live value of the member `char_id`.
pub(super) fn readout(char_id: u32, part: ReadoutPart) -> MemberReadout {
    MemberReadout { char_id, part }
}

/// The gold mark beside the leader's name.
pub(super) fn crown(size: f32) -> impl Scene {
    diamond(size, ro_dark::RARE_RING, ro_dark::RARE_RING)
}

fn card(view: CardView) -> impl Scene {
    let (fill, stroke, border) = match (view.selected, view.online) {
        (true, _) => (ro_dark::SLOT, ro_dark::SELECT_RING, 2.0),
        (false, true) => (ro_dark::SLOT, ro_dark::SLOT_STROKE, 1.0),
        (false, false) => (ro_dark::SLOT_DISABLED, ro_dark::INSET_RULE, 1.0),
    };
    let halo = glow(if view.selected {
        ro_dark::FROST_GLOW.with_alpha(0.45)
    } else {
        Color::NONE
    });
    let colors = HoverColors {
        rest: (fill, stroke),
        hover: if view.selected {
            (fill, stroke)
        } else {
            (ro_dark::SLOT_HOVER, ro_dark::BUTTON_STROKE)
        },
    };
    let (dot, ink) = match (view.online, view.me) {
        (false, _) => (ro_dark::SLOT_STROKE, ro_dark::MUTED),
        (true, true) => (ro_dark::JOB_EXP, ro_dark::FROST),
        (true, false) => (ro_dark::JOB_EXP, ro_dark::STRONG),
    };
    let char_id = view.char_id;
    let crown = view.leader.then(|| EntityScene(crown(9.0)));
    let you = view
        .me
        .then(|| EntityScene(chrome_text("You".into(), 11.0, ro_dark::FROST)));
    let place = readout(char_id, ReadoutPart::Where);
    let place = view.online.then(|| {
        EntityScene(bsn! {
            place
            @chrome_text(String::new(), 11.0, ro_dark::MUTED)
            TextLayout { linebreak: LineBreak::NoWrap }
        })
    });
    let offline =
        (!view.online).then(|| EntityScene(chrome_text("Offline".into(), 11.0, ro_dark::MUTED)));
    let bars = view.online.then(|| {
        EntityScene(bsn! {
            Node { column_gap: px(6) }
            @ignore_picking()
            Children [
                @bar(char_id, (ReadoutPart::HpFill, ReadoutPart::HpText), ro_dark::HP, 3.0)
                --
                @bar(char_id, (ReadoutPart::SpFill, ReadoutPart::SpText), ro_dark::SP, 2.0)
            ]
        })
    });
    bsn! {
        Button
        Hovered
        colors
        PartyAction::Select(char_id)
        Node {
            width: px(CARD_WIDTH),
            height: px(CARD_HEIGHT),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::axes(px(12.0 - border), px(10.0 - border))},
            border: px(border),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        halo
        Pickable
        on(on_party_button)
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [
                Node { width: px(8), height: px(8), flex_shrink: 0.0, border_radius: BorderRadius::MAX }
                BackgroundColor(dot)
                @ignore_picking()
                --
                {crown}
                --
                @chrome_text(view.name, 13.0, ink)
                --
                {you}
                --
                Node { flex_grow: 1.0 }
                @ignore_picking()
                --
                {place}
                --
                {offline}
            ]
            --
            @chrome_text(view.detail, 11.0, ro_dark::MUTED)
            --
            {bars}
        ]
    }
}

/// A resource bar with its amount written over it. `grow` shares the row between bars.
fn bar(
    char_id: u32,
    (fill, text): (ReadoutPart, ReadoutPart),
    color: Color,
    grow: f32,
) -> impl Scene {
    let fill = readout(char_id, fill);
    let text = readout(char_id, text);
    bsn! {
        Node {
            flex_grow: grow,
            flex_basis: px(0),
            height: px(12),
            padding: px(1),
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::BAR_TROUGH)
        BorderColor::all(ro_dark::BAR_FRAME)
        @ignore_picking()
        Children [
            fill
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(4)),
            }
            BackgroundColor(color)
            @ignore_picking()
            --
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
            Children [
                text
                @chrome_text(String::new(), 9.5, ro_dark::VALUE)
                TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
            ]
        ]
    }
}

fn open_slot() -> impl Scene {
    bsn! {
        Node {
            width: px(CARD_WIDTH),
            height: px(CARD_HEIGHT),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [ @chrome_text("Open slot".into(), 11.0, ro_dark::FAINT) ]
    }
}

/// The selected member with what can be done to them, or the expel prompt.
fn selection_bar(view: Option<SelectionView>) -> Vec<Box<dyn Scene>> {
    let Some(view) = view else {
        return vec![Box::new(bsn! {
            Node { flex_grow: 1.0, justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text("Select a member".into(), 12.0, ro_dark::MUTED) ]
        })];
    };
    let char_id = view.char_id;
    let mut bar: Vec<Box<dyn Scene>> = vec![Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [
            @chrome_text(view.title, 13.0, ro_dark::STRONG)
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            @chrome_text(view.meta, 11.0, ro_dark::MUTED)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    })];
    if view.ask_expel {
        bar.push(Box::new(chrome_text(
            format!("Expel {}?", view.name),
            12.0,
            ro_dark::DANGER_GLOW,
        )));
        bar.push(Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Keep", 48.0)
            PartyAction::KeepMember
            on(on_party_button)
        }));
        bar.push(Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Expel", 52.0)
            @usable(true, DANGER_TINT)
            PartyAction::Expel(char_id)
            on(on_party_button)
        }));
        return bar;
    }
    // NOTE: disabled, because the client has no whisper. Point it at the chat's
    // whisper target once that exists.
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Whisper", 64.0)
        @usable(false, Color::WHITE)
    }));
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Trade", 52.0)
        @usable(view.can_trade, Color::WHITE)
        PartyAction::Trade(char_id)
        on(on_party_button)
    }));
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Make Leader", 92.0)
        @usable(view.can_lead, Color::WHITE)
        PartyAction::MakeLeader(char_id)
        on(on_party_button)
    }));
    bar.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Expel", 52.0)
        @usable(view.can_expel, DANGER_TINT)
        PartyAction::AskExpel
        on(on_party_button)
    }));
    bar
}

fn sharing(view: SharingView) -> impl Scene {
    let who = if view.leader {
        "You lead \u{b7} You can change these"
    } else {
        "Only the leader can change these"
    };
    let act = |action: PartyAction| view.leader.then_some(action);
    let exp_tone = if view.exp_warn {
        ro_dark::STATUS_BUSY
    } else {
        ro_dark::MUTED
    };
    let pickup_note = if view.pickup_share {
        "Anyone in the party can pick up any drop."
    } else {
        "Whoever picks it up keeps it."
    };
    bsn! {
        @inset()
        Node { flex_direction: FlexDirection::Column, row_gap: px(12), padding: px(14) }
        @ignore_picking()
        Children [
            Node {
                align_items: AlignItems::Baseline,
                justify_content: JustifyContent::SpaceBetween,
            }
            @ignore_picking()
            Children [
                @overline("SHARING".into(), ro_dark::ACCENT)
                --
                @chrome_text(who.into(), 11.0, ro_dark::MUTED)
            ]
            --
            @option(
                "EXP",
                [
                    ("Each Take", !view.exp_share, act(PartyAction::ExpShare(false))),
                    (
                        "Even Share",
                        view.exp_share,
                        act(PartyAction::ExpShare(true)).filter(|_| view.even_ok),
                    ),
                ],
                (view.exp_note, exp_tone),
            )
            --
            @option(
                "Item Pickup",
                [
                    ("Each Take", !view.pickup_share, act(PartyAction::PickupShare(false))),
                    ("Party Share", view.pickup_share, act(PartyAction::PickupShare(true))),
                ],
                (pickup_note.into(), ro_dark::MUTED),
            )
            --
            // NOTE: disabled, because the protocol has no item distribution option.
            // Wire both choices once `PartyInfo` carries one.
            @option(
                "Item Sharing",
                [("Individual", false, None), ("Random", false, None)],
                ("Not supported by the server yet.".into(), ro_dark::MUTED),
            )
        ]
    }
}

/// One sharing option: its name, its two choices and a note on the current one.
fn option(
    label: &'static str,
    choices: [(&'static str, bool, Option<PartyAction>); 2],
    (note, tone): (String, Color),
) -> impl Scene {
    let choices: Vec<_> = choices
        .into_iter()
        .map(|(label, on, action)| choice(label, on, action))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::LABEL)
            --
            Node { column_gap: px(4) }
            @ignore_picking()
            Children [ {choices} ]
            --
            @chrome_text(note, 11.0, tone)
        ]
    }
}

/// A choice, `lit` while it is the current one. Without an `action` it is dimmed and
/// cannot be picked.
fn choice(label: &'static str, lit: bool, action: Option<PartyAction>) -> impl Scene {
    let (ink, _) = tab_text(lit);
    let ink = if action.is_some() {
        ink
    } else {
        ink.with_alpha(0.55)
    };
    let off = action.is_none().then(ignore_picking);
    let wired = action.map(|action| {
        bsn! {
            action
            on(on_party_button)
        }
    });
    bsn! {
        Button
        @segment(lit)
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            height: px(30),
            justify_content: JustifyContent::Center,
        }
        @{wired}
        @{off}
        Children [ @chrome_text(label.into(), 12.0, ink) ]
    }
}

fn levels(view: LevelsView) -> impl Scene {
    let band = if view.even_ok {
        ro_dark::STATUS_ONLINE.with_alpha(0.55)
    } else {
        ro_dark::STATUS_BUSY.with_alpha(0.6)
    };
    let (band_left, band_width) = view.band;
    let ticks: Vec<_> = view.ticks.into_iter().map(tick).collect();
    let rule =
        format!("Online members only \u{b7} Even Share needs a gap of {SHARE_GAP} levels or less");
    bsn! {
        @inset()
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::axes(px(14), px(12))},
        }
        @ignore_picking()
        Children [
            @overline("LEVELS".into(), ro_dark::ACCENT)
            --
            Node { height: px(34), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: px(13),
                    height: px(6),
                    border: px(1),
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor(ro_dark::BAR_TROUGH)
                BorderColor::all(ro_dark::INSET_RULE)
                @ignore_picking()
                --
                Node {
                    position_type: PositionType::Absolute,
                    left: {Val::Percent(band_left)},
                    width: {Val::Percent(band_width)},
                    top: px(13),
                    height: px(6),
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor(band)
                @ignore_picking()
                --
                {ticks}
                --
                @chrome_text(format!("Lv {}", view.lo), 10.0, ro_dark::MUTED)
                Node { position_type: PositionType::Absolute, left: px(0), top: px(22) }
                --
                @chrome_text(format!("Lv {}", view.hi), 10.0, ro_dark::MUTED)
                Node { position_type: PositionType::Absolute, right: px(0), top: px(22) }
            ]
            --
            @chrome_text(rule, 11.0, ro_dark::MUTED)
        ]
    }
}

/// One member's place on the level track; frost for the local player.
fn tick((at, me): (f32, bool)) -> impl Scene {
    let fill = if me {
        ro_dark::FROST_GLOW
    } else {
        ro_dark::BRONZE_LIGHT
    };
    bsn! {
        @diamond(10.0, fill, ro_dark::BAR_TROUGH)
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Percent(at)},
            top: px(10),
            margin: {UiRect::left(px(-5))},
        }
    }
}

/// Leave Party, or the prompt it raises.
fn leave_controls(view: LeaveView) -> Vec<Box<dyn Scene>> {
    if !view.ask {
        return vec![Box::new(bsn! {
            @wide_button(ButtonArt::NORMAL, "Leave Party", 112.0)
            @usable(true, DANGER_TINT)
            PartyAction::AskLeave
            on(on_party_button)
        })];
    }
    vec![
        Box::new(chrome_text(view.prompt, 12.0, ro_dark::DANGER_GLOW)),
        Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Stay", 48.0)
            PartyAction::Stay
            on(on_party_button)
        }),
        Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Leave Party", 92.0)
            @usable(true, DANGER_TINT)
            PartyAction::Leave
            on(on_party_button)
        }),
    ]
}
