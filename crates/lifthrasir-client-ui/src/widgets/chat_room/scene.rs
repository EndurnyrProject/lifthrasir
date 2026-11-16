//! The chat room scenes: the frames spawned once and the regions
//! `rebuild_chat_room_ui` projects from their view-models. Sizes follow the
//! `Chat Room Window` and `Chat Room Signs (World)` artboards.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{Button, ScrollArea};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, CONTENT_INSET, WINDOW_SHADOW, close_button, display_text, inset, segment, tab_text,
    window_title_bar, window_titled,
};

use super::view::{
    CardFootView, CardHeadView, FootView, FormMode, HeadView, LineView, MESSAGE_MAX, OptionsView,
    PASSWORD_MAX, PeopleView, PersonView, RoomForm, SignTone, SignView, TITLE_MAX,
};
use super::{
    ChatRoomAction, ChatRoomHost, ChatRoomTitlebar, ChatRoomWindow, PasswordRow, PersonActions,
    PersonRow, RoomField, TitleCount, on_room_button,
};
use crate::screens::shared::{
    DANGER_TINT, chip, colored_text, modal_layer, overline, row_button, sign_title, small_button,
    text_field, usable, wide_button,
};
use crate::worldspace::{WORLD_OVERLAY_Z, hung_root};

const WINDOW_LEFT: f32 = 230.0;
const WINDOW_TOP: f32 = 80.0;
/// Width of the window's shape.
const WINDOW_WIDTH: f32 = 820.0;
const SIDE_WIDTH: f32 = 236.0;
const LOG_HEIGHT: f32 = 380.0;
const PEOPLE_HEIGHT: f32 = 402.0;
/// A scroll offset past the end of any log; the layout clamps it to the last line.
pub(super) const LOG_END: f32 = 1.0e9;
const DIALOG_WIDTH: f32 = 420.0;
/// Over the game's windows, level with the invitation dialogs.
const DIALOG_Z: i32 = i32::MAX - 4;
/// Widest a sign gets; a longer title is cut off.
pub(crate) const SIGN_WIDTH: f32 = 230.0;
pub(super) const CARD_WIDTH: f32 = 260.0;
/// How far under its owner's feet a card hangs.
pub(super) const CARD_DROP: f32 = 26.0;
/// Over the name plates (+100), under the game's windows.
const CARD_Z: i32 = WORLD_OVERLAY_Z + 200;

/// The room window's frame: the title bar, the log over the message field, and the
/// side column.
pub fn window() -> impl Scene {
    bsn! {
        ChatRoomWindow
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
            ChatRoomTitlebar
            Node {
                height: px(40),
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: {UiRect::left(px(46.0 - CONTENT_INSET))},
                margin: {UiRect { right: px(14.0 - CONTENT_INSET), bottom: px(6), ..default() }},
            }
            Pickable
            on(drag_window::<ChatRoomTitlebar, ChatRoomWindow>)
            Children [
                ChatRoomHost::Head
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                @ignore_picking()
                --
                @close_button()
                ChatRoomAction::Leave
                on(on_room_button)
            ]
            --
            Node { column_gap: px(14), margin: {UiRect::new(px(0), px(0), px(8), px(4))} }
            @ignore_picking()
            Children [
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                }
                @ignore_picking()
                Children [
                    ChatRoomHost::Log
                    ScrollArea
                    ScrollPosition({Vec2::new(0.0, LOG_END)})
                    @inset()
                    Node {
                        height: px(LOG_HEIGHT),
                        flex_direction: FlexDirection::Column,
                        padding: {UiRect::axes(px(12), px(10))},
                        overflow: {Overflow::scroll_y()},
                    }
                    Pickable
                    --
                    Node { align_items: AlignItems::Center, column_gap: px(8) }
                    @ignore_picking()
                    Children [
                        RoomField::Message
                        @field(String::new(), MESSAGE_MAX)
                        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                        --
                        @wide_button(ButtonArt::PRIMARY, "Send", 64.0)
                        ChatRoomAction::Send
                        on(on_room_button)
                    ]
                    --
                    @chrome_text(
                        "Only people in this room see these messages. Your main chat keeps running.".into(),
                        11.0,
                        ro_dark::MUTED,
                    )
                ]
                --
                ChatRoomHost::Side
                Node {
                    width: px(SIDE_WIDTH),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
            ]
        ]
    }
}

/// The screen-wide region the New Chat Room dialog is centred in. It lets every
/// click through while empty.
pub fn dialog_host() -> impl Scene {
    bsn! {
        ChatRoomHost::Dialog
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        @ignore_picking()
    }
}

/// A 30px [`text_field`], the height of every chat room field.
fn field(value: String, max_chars: usize) -> impl Scene {
    bsn! {
        @text_field(value, max_chars)
        Node { height: px(30), padding: {UiRect::axes(px(8), px(6))} }
    }
}

/// The fill and ink of a head count, red once the room is full.
fn count_tones(full: bool) -> (Color, Color) {
    if full {
        (ro_dark::TONE_ERROR.with_alpha(0.22), ro_dark::TONE_ERROR)
    } else {
        (Color::WHITE.with_alpha(0.08), ro_dark::LABEL)
    }
}

fn lock(size: f32) -> impl Scene {
    bsn! {
        @glyph_icon("lock", size, ro_dark::TONE_WHISPER)
        Node { flex_shrink: 0.0 }
    }
}

fn crown(size: f32) -> impl Scene {
    bsn! {
        @glyph_icon("crown", size, ro_dark::RARE_RING)
        Node { flex_shrink: 0.0 }
    }
}

pub(super) fn head(view: HeadView) -> impl Scene {
    let lock = view.private.then(|| EntityScene(lock(13.0)));
    let (fill, ink) = count_tones(view.full);
    let label = if view.editing {
        "Close Settings"
    } else {
        "Room Settings"
    };
    let settings = view.owner.then(|| {
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, label, 108.0)
            ChatRoomAction::ToggleSettings
            on(on_room_button)
        })
    });
    bsn! {
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        @ignore_picking()
        Children [
            {lock}
            --
            Node { min_width: px(0), max_width: px(440), overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [
                @display_text(view.title, 17.0, ro_dark::TITLE)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            @chip(view.count, fill, ink)
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            {settings}
        ]
    }
}

pub(super) fn log(lines: Vec<LineView>) -> impl Scene {
    let rows: Vec<Box<dyn Scene>> = lines.into_iter().map(line).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(2) }
        @ignore_picking()
        Children [ {rows} ]
    }
}

fn line(view: LineView) -> Box<dyn Scene> {
    let (time, name, me, owner, text) = match view {
        LineView::Notice { text, tone } => {
            return Box::new(bsn! {
                Node {
                    justify_content: JustifyContent::Center,
                    padding: {UiRect::axes(px(4), px(3))},
                }
                @ignore_picking()
                Children [ @chrome_text(text, 11.0, tone) ]
            });
        }
        LineView::Said {
            time,
            name,
            me,
            crown,
            text,
        } => (time, name, me, crown, text),
    };
    let ink = if me { ro_dark::HERO } else { ro_dark::STRONG };
    let runs = vec![
        (ink, name),
        (ro_dark::MUTED, " : ".to_string()),
        (ro_dark::BODY, text),
    ];
    let crown = owner.then(|| {
        EntityScene(bsn! {
            @crown(11.0)
            Node { margin: {UiRect::top(px(4))} }
        })
    });
    Box::new(bsn! {
        Node { column_gap: px(8), padding: {UiRect::axes(px(4), px(3))} }
        @ignore_picking()
        Children [
            @chrome_text(time, 10.0, ro_dark::MUTED)
            Node { width: px(34), flex_shrink: 0.0, margin: {UiRect::top(px(3))} }
            --
            {crown}
            --
            @colored_text(runs, theme::FONT_BODY, 13.0)
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
        ]
    })
}

/// The people in the room over the button that leaves it.
pub(super) fn people(view: PeopleView) -> impl Scene {
    let rows: Vec<_> = view.rows.into_iter().map(person).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        @ignore_picking()
        Children [
            ScrollArea
            @inset()
            Node {
                height: px(PEOPLE_HEIGHT),
                flex_direction: FlexDirection::Column,
                padding: {UiRect::axes(px(6), px(8))},
                overflow: {Overflow::scroll_y()},
            }
            Pickable
            Children [
                Node {
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: {UiRect::new(px(8), px(8), px(2), px(6))},
                }
                @ignore_picking()
                Children [
                    @overline("PEOPLE".into(), ro_dark::ACCENT)
                    --
                    @chrome_text(view.count, 11.0, ro_dark::LABEL)
                ]
                --
                {rows}
            ]
            --
            @row_button(ButtonArt::NORMAL, view.leave, SIDE_WIDTH)
            @usable(true, DANGER_TINT)
            ChatRoomAction::Leave
            on(on_room_button)
        ]
    }
}

fn person(view: PersonView) -> impl Scene {
    let (disc, ink) = match (view.owner, view.me) {
        (true, _) => (ro_dark::RARE_RING, ro_dark::STRONG),
        (false, true) => (ro_dark::FROST, ro_dark::HERO),
        (false, false) => (ro_dark::LABEL, ro_dark::BODY),
    };
    let crown = view.owner.then(|| EntityScene(crown(11.0)));
    let (promote, kick) = (
        ChatRoomAction::MakeOwner(view.char_id),
        ChatRoomAction::Kick(view.char_id),
    );
    let actions = view.manage.then(|| {
        EntityScene(bsn! {
            PersonActions
            Node { flex_shrink: 0.0, align_items: AlignItems::Center, column_gap: px(4) }
            Visibility::Hidden
            @ignore_picking()
            Children [
                @small_button("crown", 24.0)
                promote
                on(on_room_button)
                --
                @row_button(ButtonArt::NORMAL, "Kick", 40.0)
                @usable(true, DANGER_TINT)
                kick
                on(on_room_button)
            ]
        })
    });
    bsn! {
        PersonRow
        Hovered
        Node {
            height: px(36),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(8))},
        }
        Pickable
        Children [
            Node {
                width: px(22),
                height: px(22),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::MAX,
            }
            BackgroundColor(disc)
            @ignore_picking()
            Children [ @chrome_text(view.initial, 10.0, ro_dark::CANVAS) ]
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                align_items: AlignItems::Center,
                column_gap: px(4),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.label, 12.0, ink)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                {crown}
            ]
            --
            {actions}
        ]
    }
}

/// `Flex` while a form's room asks for a password, else out of the layout.
pub(super) fn shown_row(private: bool) -> Display {
    if private {
        Display::Flex
    } else {
        Display::None
    }
}

/// A room form's skeleton: the typed fields, filled from `form`, around the hosts of
/// its options and its buttons.
fn form(form: &RoomForm) -> impl Scene {
    let (title, password) = (form.title.clone(), form.password.clone());
    let password_row = shown_row(form.private);
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        @ignore_picking()
        Children [
            Node { flex_direction: FlexDirection::Column, row_gap: px(5) }
            @ignore_picking()
            Children [
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Baseline,
                }
                @ignore_picking()
                Children [
                    @overline("TITLE".into(), ro_dark::ACCENT)
                    --
                    TitleCount
                    @chrome_text(String::new(), 11.0, ro_dark::MUTED)
                ]
                --
                RoomField::Title
                @field(title, TITLE_MAX)
            ]
            --
            ChatRoomHost::FormOptions
            Node { flex_direction: FlexDirection::Column }
            @ignore_picking()
            --
            PasswordRow
            Node { display: password_row, flex_direction: FlexDirection::Column, row_gap: px(5) }
            @ignore_picking()
            Children [
                @overline("PASSWORD \u{b7} UP TO 8".into(), ro_dark::ACCENT)
                --
                RoomField::Password
                @field(password, PASSWORD_MAX)
            ]
            --
            ChatRoomHost::FormFoot
            Node { flex_direction: FlexDirection::Column, margin: {UiRect::top(px(4))} }
            @ignore_picking()
        ]
    }
}

/// The owner's settings panel, in place of the people list.
pub(super) fn settings(form: &RoomForm) -> impl Scene {
    let fields = self::form(form);
    bsn! {
        @inset()
        Node { flex_direction: FlexDirection::Column, row_gap: px(10), padding: px(12) }
        @ignore_picking()
        Children [
            @overline("ROOM SETTINGS".into(), ro_dark::ACCENT)
            --
            @fields
        ]
    }
}

/// The New Chat Room dialog over a shade that keeps clicks off the game.
pub(super) fn dialog(form: &RoomForm) -> impl Scene {
    let fields = self::form(form);
    bsn! {
        @modal_layer(DIALOG_Z)
        BackgroundColor({Color::srgba(0.02, 0.027, 0.04, 0.5)})
        Children [
            @window_titled()
            Node {
                width: px(DIALOG_WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Pickable
            Children [
                @window_title_bar("New Chat Room", bsn! {
                    @close_button()
                    ChatRoomAction::CancelForm
                    on(on_room_button)
                })
                --
                @fields
                Node { margin: {UiRect::new(px(0), px(0), px(8), px(4))} }
            ]
        ]
    }
}

/// The seat stepper beside the who-can-join choice; stacked in the settings panel,
/// which is too narrow for both.
pub(super) fn options(view: OptionsView) -> impl Scene {
    let (direction, people) = match view.mode {
        FormMode::Create => (FlexDirection::Row, "PEOPLE".to_string()),
        FormMode::Edit { seated } => (
            FlexDirection::Column,
            format!("PEOPLE \u{b7} {seated} IN THE ROOM"),
        ),
    };
    let (fewer, more) = (ChatRoomAction::Seats(-1), ChatRoomAction::Seats(1));
    bsn! {
        Node { flex_direction: direction, column_gap: px(16), row_gap: px(12) }
        @ignore_picking()
        Children [
            Node { flex_direction: FlexDirection::Column, row_gap: px(5) }
            @ignore_picking()
            Children [
                @overline(people, ro_dark::ACCENT)
                --
                Node { align_items: AlignItems::Center, column_gap: px(6) }
                @ignore_picking()
                Children [
                    @small_button("minus", 28.0)
                    @usable(view.fewer, Color::WHITE)
                    fewer
                    on(on_room_button)
                    --
                    Node { width: px(28), justify_content: JustifyContent::Center }
                    @ignore_picking()
                    Children [ @chrome_text(view.seats.to_string(), 15.0, ro_dark::STRONG) ]
                    --
                    @small_button("plus", 28.0)
                    @usable(view.more, Color::WHITE)
                    more
                    on(on_room_button)
                ]
            ]
            --
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
            }
            @ignore_picking()
            Children [
                @overline("WHO CAN JOIN".into(), ro_dark::ACCENT)
                --
                Node { column_gap: px(4) }
                @ignore_picking()
                Children [
                    @choice("Anyone", !view.private, ChatRoomAction::Access(false))
                    --
                    @choice("Password", view.private, ChatRoomAction::Access(true))
                ]
            ]
        ]
    }
}

/// One of the who-can-join choices, `lit` while it is the current one.
fn choice(label: &'static str, lit: bool, action: ChatRoomAction) -> impl Scene {
    let (ink, _) = tab_text(lit);
    bsn! {
        Button
        @segment(lit)
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            height: px(28),
            justify_content: JustifyContent::Center,
        }
        action
        on(on_room_button)
        Children [ @chrome_text(label.into(), 12.0, ink) ]
    }
}

/// A new room's sign as others will see it, over the form's buttons.
pub(super) fn foot(view: FootView) -> impl Scene {
    let preview = view.preview.map(|sign| {
        EntityScene(bsn! {
            Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
            @ignore_picking()
            Children [
                @overline("OVER YOUR HEAD".into(), ro_dark::ACCENT)
                --
                Node {
                    height: px(46),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(6)),
                }
                BackgroundColor({Color::BLACK.with_alpha(0.25)})
                BorderColor::all(ro_dark::INSET_RULE)
                @ignore_picking()
                Children [
                    @sign_plate(sign)
                    @ignore_picking()
                ]
            ]
        })
    });
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(16) }
        @ignore_picking()
        Children [
            {preview}
            --
            Node { justify_content: JustifyContent::FlexEnd, column_gap: px(8) }
            @ignore_picking()
            Children [
                @wide_button(ButtonArt::NORMAL, "Cancel", 72.0)
                ChatRoomAction::CancelForm
                on(on_room_button)
                --
                @wide_button(ButtonArt::PRIMARY, view.submit, 96.0)
                @usable(view.enabled, Color::WHITE)
                ChatRoomAction::SubmitForm
                on(on_room_button)
            ]
        ]
    }
}

/// A chat room's sign: a lock on a password room, a tag on an NPC's, the title and
/// the head count, ringed in its tone. Picking is left to the caller.
pub(crate) fn sign_plate(view: SignView) -> impl Scene {
    let (ring, fill, ink) = view.tone.colors();
    let halo = if view.tone == SignTone::Own {
        ring.with_alpha(0.5)
    } else {
        Color::NONE
    };
    let shade = |color, y, blur| ShadowStyle {
        color,
        x_offset: Val::ZERO,
        y_offset: px(y),
        spread_radius: Val::ZERO,
        blur_radius: px(blur),
    };
    let shadow = BoxShadow(vec![
        shade(Color::BLACK.with_alpha(0.6), 4, 10),
        shade(halo, 0, 12),
    ]);
    let lock = view.private.then(|| EntityScene(lock(11.0)));
    let npc = view.npc.then(|| EntityScene(chip("NPC".into(), fill, ink)));
    bsn! {
        Node {
            height: px(28),
            max_width: px(SIGN_WIDTH),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(7),
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(ro_dark::TITLE_BAR)
        BorderColor::all(ring)
        shadow
        Children [
            {lock}
            --
            {npc}
            --
            Node { min_width: px(0), overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [ @sign_title(view.title) ]
            --
            @chip(view.count, fill, ink)
        ]
    }
}

/// A sign's card, hung under the room's owner: the hosts of its head and foot around
/// the password field of a `locked` room.
pub(super) fn card(locked: bool) -> impl Scene {
    let password = locked.then(|| {
        EntityScene(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                margin: {UiRect::top(px(10))},
            }
            @ignore_picking()
            Children [
                @chrome_text("This room needs a password".into(), 11.0, ro_dark::TONE_WHISPER)
                --
                RoomField::CardPassword
                @field(String::new(), PASSWORD_MAX)
            ]
        })
    });
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.75),
        Val::ZERO,
        px(6),
        Val::ZERO,
        px(16),
    );
    bsn! {
        @hung_root(CARD_WIDTH, CARD_Z)
        Children [
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                padding: {UiRect::new(px(12), px(12), px(11), px(12))},
                border: px(1),
                border_radius: BorderRadius::all(px(7)),
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::BUTTON_STROKE)
            shadow
            Pickable
            Children [
                ChatRoomHost::CardHead
                Node { flex_direction: FlexDirection::Column }
                @ignore_picking()
                --
                {password}
                --
                ChatRoomHost::CardFoot
                Node { flex_direction: FlexDirection::Column }
                @ignore_picking()
            ]
        ]
    }
}

pub(super) fn card_head(view: CardHeadView) -> impl Scene {
    let (fill, ink) = count_tones(view.full);
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(3) }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::FlexStart, column_gap: px(6) }
            @ignore_picking()
            Children [
                @chrome_text(view.title, 13.0, ro_dark::STRONG)
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                --
                @chip(view.count, fill, ink)
            ]
            --
            @chrome_text(view.meta, 11.0, ro_dark::LABEL)
        ]
    }
}

pub(super) fn card_foot(view: CardFootView) -> impl Scene {
    let note = view
        .note
        .map(|(text, tone)| EntityScene(chrome_text(text, 11.0, tone)));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            margin: {UiRect::top(px(10))},
        }
        @ignore_picking()
        Children [
            {note}
            --
            Node { justify_content: JustifyContent::FlexEnd, column_gap: px(6) }
            @ignore_picking()
            Children [
                @row_button(ButtonArt::NORMAL, "Close", 52.0)
                ChatRoomAction::CloseCard
                on(on_room_button)
                --
                @row_button(ButtonArt::PRIMARY, view.join, 96.0)
                @usable(view.enabled, Color::WHITE)
                ChatRoomAction::Join
                on(on_room_button)
            ]
        ]
    }
}
