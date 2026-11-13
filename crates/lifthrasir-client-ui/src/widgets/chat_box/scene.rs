//! The chat panel scenes: the frame [`panel`] spawns once and the regions
//! `rebuild_chat_ui` projects from their view-models. Sizes follow the `Chat`
//! artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{EditableText, FontSourceTemplate};
use bevy::ui_widgets::{Button, ScrollArea, TextInput};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::placeholder::Placeholder;
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, HoverColors, INPUT_SHADOW, diamond, input_frame, tab, tab_text,
};

use super::view::{Command, HeadView, MenuRow, PopupView, Scope, TabView, WHISPER, WHISPER_NAME};
use super::{
    CHAT_MAX_CHARS, ChatAction, ChatBoxRoot, ChatEmpty, ChatHint, ChatHistory, ChatHost, ChatInput,
    ChatLogFrame, LOG_END, on_chat_button, on_line_click,
};
use crate::screens::shared::{overline, usable, wide_button};

const PANEL_WIDTH: f32 = 540.0;
const LOG_HEIGHT: f32 = 200.0;
/// How far a popup floats above the bottom of the input row.
const POPUP_LIFT: f32 = 38.0;
const POPUP_Z: i32 = 20;

/// The frame: the tab strip, the log and the input row. The hosts are filled by
/// `rebuild_chat_ui`.
pub(super) fn panel() -> impl Scene {
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.7),
        Val::ZERO,
        px(4),
        Val::ZERO,
        px(12),
    );
    bsn! {
        ChatBoxRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(16),
            width: px(PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            padding: {UiRect::new(px(10), px(10), px(8), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({ro_dark::WINDOW_BODY.with_alpha(0.86)})
        BorderColor::all(ro_dark::BRONZE.with_alpha(0.8))
        shadow
        Pickable
        Children [
            @stud(true, true)
            --
            @stud(false, true)
            --
            @stud(true, false)
            --
            @stud(false, false)
            --
            ChatHost::Head
            Node { flex_direction: FlexDirection::Column }
            @ignore_picking()
            --
            @log()
            --
            Node {
                margin: {UiRect::top(px(8))},
                align_items: AlignItems::Center,
                column_gap: px(8),
            }
            @ignore_picking()
            Children [
                ChatHost::Channel
                Node { flex_shrink: 0.0 }
                @ignore_picking()
                --
                @field()
                --
                ChatHost::Send
                Node { flex_shrink: 0.0 }
                @ignore_picking()
                --
                ChatHost::Popup
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    bottom: px(POPUP_LIFT),
                }
                GlobalZIndex(POPUP_Z)
                @ignore_picking()
            ]
        ]
    }
}

/// A bronze stud on one corner of the panel.
fn stud(left: bool, top: bool) -> impl Scene {
    let near = px(-5);
    let (left, right) = if left {
        (near, Val::Auto)
    } else {
        (Val::Auto, near)
    };
    let (top, bottom) = if top {
        (near, Val::Auto)
    } else {
        (Val::Auto, near)
    };
    bsn! {
        @diamond(8.0, ro_dark::BRONZE, ro_dark::CORNER_DISC)
        Node {
            position_type: PositionType::Absolute,
            left: {left},
            right: {right},
            top: {top},
            bottom: {bottom},
        }
    }
}

/// The scrolling log with its empty note and the host of the new-messages pill.
/// Lines are spawned under [`ChatHistory`] as they arrive.
fn log() -> impl Scene {
    bsn! {
        ChatLogFrame
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            ChatHistory
            ScrollArea
            ScrollPosition({Vec2::new(0.0, LOG_END)})
            Node {
                height: px(LOG_HEIGHT),
                flex_direction: FlexDirection::Column,
                padding: {UiRect::new(px(12), px(12), px(6), px(8))},
                border: px(1),
                border_radius: {BorderRadius::new(px(0), px(6), px(6), px(6))},
                overflow: {Overflow::scroll_y()},
            }
            BackgroundColor({ro_dark::INSET.with_alpha(0.72)})
            BorderColor::all(ro_dark::SLOT_STROKE.with_alpha(0.45))
            Pickable
            on(on_line_click)
            --
            ChatEmpty
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(76),
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
            Children [ @chrome_text("Nothing here yet".into(), 12.0, ro_dark::MUTED) ]
            --
            ChatHost::Pill
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(8),
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
        ]
    }
}

/// The input in its frame, growing to share the row. The hint over it is rewritten
/// when the input aims somewhere else.
fn field() -> impl Scene {
    let text = EditableText {
        max_characters: Some(CHAT_MAX_CHARS),
        ..default()
    };
    bsn! {
        @input_frame()
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            margin: {UiRect::all(px(-INPUT_SHADOW))},
            position_type: PositionType::Relative,
        }
        Children [
            #field
            ChatInput
            text
            TextInput
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_BODY),
                font_size: {FontSize::Px(13.0)},
            }
            TextColor(ro_dark::BODY)
            Node { flex_grow: 1.0, min_width: px(0), height: px(18) }
            --
            ChatHint
            Placeholder(#field)
            @chrome_text(String::new(), 13.0, ro_dark::MUTED)
            Node {
                position_type: PositionType::Absolute,
                left: px(INPUT_SHADOW + 10.0),
                top: px(INPUT_SHADOW + 6.0),
            }
        ]
    }
}

/// The tab strip with the timestamp and collapse toggles at its end.
pub(super) fn head(view: HeadView) -> impl Scene {
    let tabs: Vec<Box<dyn Scene>> = view
        .tabs
        .into_iter()
        .map(|tab_view| Box::new(chat_tab(tab_view)) as Box<dyn Scene>)
        .collect();
    let time_ink = if view.show_time {
        ro_dark::FROST
    } else {
        ro_dark::MUTED
    };
    // The glyph points right: a quarter turn makes it "collapse", the other "expand".
    let turn = if view.collapsed { -90.0 } else { 90.0 };
    bsn! {
        Node { align_items: AlignItems::FlexEnd, column_gap: px(4) }
        @ignore_picking()
        Children [
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                align_items: AlignItems::FlexEnd,
                column_gap: px(4),
                overflow: Overflow::clip(),
            }
            @ignore_picking()
            Children [ {tabs} ]
            --
            @tool_button("clock", time_ink, 0.0)
            ChatAction::ToggleTime
            on(on_chat_button)
            --
            @tool_button("chev", ro_dark::MUTED, turn)
            ChatAction::ToggleCollapse
            on(on_chat_button)
        ]
    }
}

/// One tab: its label, the unread badge and, for a whisper, the button closing it.
fn chat_tab(view: TabView) -> impl Scene {
    let whisper = match &view.scope {
        Some(Scope::Whisper(name)) => Some(name.clone()),
        _ => None,
    };
    let ink = match (&whisper, view.active) {
        (Some(_), true) => WHISPER_NAME,
        (Some(_), false) => WHISPER,
        (None, active) => tab_text(active).0,
    };
    let mut parts: Vec<Box<dyn Scene>> =
        vec![Box::new(chrome_text(view.label().to_string(), 12.0, ink))];
    if let Some(count) = view.badge() {
        parts.push(Box::new(badge(count)));
    }
    if view.unread.ping {
        parts.push(Box::new(ping()));
    }
    if let Some(name) = whisper {
        let close = ChatAction::CloseWhisper(name);
        parts.push(Box::new(bsn! {
            @tool_button("close", ro_dark::MUTED, 0.0)
            Node { width: px(18), height: px(18), margin: {UiRect::right(px(-6))} }
            close
            on(on_chat_button)
        }));
    }
    let show = ChatAction::ShowTab(view.scope);
    bsn! {
        Button
        @tab(view.active)
        Node {
            flex_grow: 0.0,
            flex_basis: Val::Auto,
            flex_shrink: 0.0,
            height: px(24),
            padding: {UiRect::horizontal(px(10))},
        }
        show
        on(on_chat_button)
        Children [ {parts} ]
    }
}

/// The count of unseen lines on a tab.
fn badge(count: String) -> impl Scene {
    bsn! {
        Node {
            min_width: px(16),
            height: px(16),
            padding: {UiRect::horizontal(px(4))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(ro_dark::PRIMARY)
        @ignore_picking()
        Children [ @chrome_text(count, 10.0, ro_dark::CHECK_MARK) ]
    }
}

/// The dot on a tab holding a line that names the player.
fn ping() -> impl Scene {
    bsn! {
        Node { width: px(6), height: px(6), border_radius: BorderRadius::all(px(3)) }
        BackgroundColor(ro_dark::FROST_GLOW)
        @ignore_picking()
    }
}

/// A borderless icon button that lights up under the pointer. `turn` rotates the
/// glyph, in degrees. The caller adds its action.
fn tool_button(icon: &'static str, ink: Color, turn: f32) -> impl Scene {
    let colors = HoverColors {
        rest: (Color::NONE, Color::NONE),
        hover: (ro_dark::BUTTON, ro_dark::BUTTON_STROKE),
    };
    let tilt = UiTransform::from_rotation(Rot2::degrees(turn));
    bsn! {
        Button
        Hovered
        colors
        Node {
            width: px(24),
            height: px(24),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(Color::NONE)
        Pickable
        Children [
            @glyph_icon(icon, 12.0, ink)
            tilt
        ]
    }
}

/// The button naming where the input sends; it opens the channel menu.
pub(super) fn channel_button(target: Scope) -> impl Scene {
    let ink = target.color();
    let colors = HoverColors {
        rest: (ro_dark::BUTTON, ro_dark::BUTTON_STROKE),
        hover: (ro_dark::BUTTON_HOVER, ro_dark::BRONZE_HIGHLIGHT),
    };
    let down = UiTransform::from_rotation(Rot2::degrees(90.0));
    bsn! {
        Button
        Hovered
        colors
        Node {
            height: px(30),
            max_width: px(132),
            padding: {UiRect::new(px(10), px(8), px(0), px(0))},
            align_items: AlignItems::Center,
            column_gap: px(7),
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
            overflow: Overflow::clip(),
        }
        BackgroundColor(ro_dark::BUTTON)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        Pickable
        ChatAction::ToggleMenu
        on(on_chat_button)
        Children [
            @dot(ink)
            --
            @chrome_text(target.label().to_string(), 12.0, ink)
            --
            @glyph_icon("chev", 9.0, ro_dark::PRIMARY_STROKE)
            down
        ]
    }
}

fn dot(ink: Color) -> impl Scene {
    bsn! {
        Node {
            width: px(7),
            height: px(7),
            flex_shrink: 0.0,
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(ink)
        @ignore_picking()
    }
}

/// The Send button, greyed out while there is nothing to send.
pub(super) fn send_button(enabled: bool) -> impl Scene {
    bsn! {
        @wide_button(ButtonArt::PRIMARY, "Send", 64.0)
        @usable(enabled, Color::WHITE)
        ChatAction::Send
        on(on_chat_button)
    }
}

/// The channel menu or the command palette, floating over the input row.
pub(super) fn popup(view: PopupView) -> impl Scene {
    let (rows, width): (Vec<Box<dyn Scene>>, Val) = match view {
        PopupView::Menu(rows) => (
            rows.into_iter()
                .map(|row| Box::new(menu_row(row)) as Box<dyn Scene>)
                .collect(),
            px(240),
        ),
        PopupView::Palette { rows, draft } => (palette(rows, draft), percent(100)),
    };
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.7),
        Val::ZERO,
        px(3),
        Val::ZERO,
        px(8),
    );
    bsn! {
        Node {
            width: {width},
            flex_direction: FlexDirection::Column,
            padding: px(4),
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        shadow
        Pickable
        Children [ {rows} ]
    }
}

/// The frame of a popup row; the lit one is the current choice.
fn option_row(lit: bool) -> impl Scene {
    let (fill, stroke) = if lit {
        (ro_dark::TAB_ACTIVE, ro_dark::TAB_STROKE)
    } else {
        (Color::NONE, Color::NONE)
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover: (if lit { fill } else { ro_dark::TAB_HOVER }, stroke),
    };
    bsn! {
        Button
        Hovered
        colors
        Node {
            min_height: px(30),
            padding: {UiRect::axes(px(8), px(5))},
            align_items: AlignItems::Center,
            column_gap: px(10),
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
    }
}

fn menu_row(row: MenuRow) -> impl Scene {
    let ink = row.scope.color();
    let label = row.label();
    let aim = ChatAction::Aim(row.scope);
    bsn! {
        @option_row(row.active)
        aim
        on(on_chat_button)
        Children [
            @dot(ink)
            --
            @chrome_text(label, 12.0, ink)
            Node { flex_grow: 1.0 }
            --
            @chrome_text(row.hint.into(), 12.0, ro_dark::MUTED)
        ]
    }
}

/// The palette: a caption, then the commands the draft could become.
fn palette(rows: Vec<(Command, bool)>, draft: String) -> Vec<Box<dyn Scene>> {
    let caption = bsn! {
        Node {
            padding: {UiRect::new(px(8), px(8), px(4), px(6))},
            justify_content: JustifyContent::SpaceBetween,
        }
        @ignore_picking()
        Children [
            @overline("COMMANDS".into(), ro_dark::ACCENT)
            --
            @chrome_text("Tab to complete \u{b7} Esc to close".into(), 10.0, ro_dark::MUTED)
        ]
    };
    let mut scenes: Vec<Box<dyn Scene>> = vec![Box::new(caption)];
    if rows.is_empty() {
        scenes.push(Box::new(bsn! {
            Node { padding: px(8) }
            @ignore_picking()
            Children [
                @chrome_text(format!("No command starts with {draft}"), 12.0, ro_dark::MUTED)
            ]
        }));
    }
    let commands = rows.into_iter().enumerate().map(|(row, (command, lit))| {
        Box::new(bsn! {
            @option_row(lit)
            ChatAction::Complete(row)
            on(on_chat_button)
            Children [
                @chrome_text(command.name.into(), 12.0, ro_dark::FROST)
                Node { width: px(72), flex_shrink: 0.0 }
                --
                @chrome_text(command.args.into(), 12.0, ro_dark::MUTED)
                Node { width: px(150), flex_shrink: 0.0 }
                --
                @chrome_text(command.about.into(), 12.0, ro_dark::LABEL)
            ]
        }) as Box<dyn Scene>
    });
    scenes.extend(commands);
    scenes
}

/// The pill over the log's end counting the lines that arrived while it was
/// scrolled up; it jumps back down.
pub(super) fn pill(below: u32) -> impl Scene {
    let label = match below {
        1 => "1 new message".to_string(),
        count => format!("{count} new messages"),
    };
    let colors = HoverColors {
        rest: (ro_dark::PRIMARY, ro_dark::PRIMARY_STROKE),
        hover: (ro_dark::PRIMARY_HOVER, ro_dark::PRIMARY_STROKE_HOVER),
    };
    let down = UiTransform::from_rotation(Rot2::degrees(90.0));
    bsn! {
        Button
        Hovered
        colors
        Node {
            height: px(24),
            padding: {UiRect::horizontal(px(12))},
            align_items: AlignItems::Center,
            column_gap: px(6),
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        BackgroundColor(ro_dark::PRIMARY)
        BorderColor::all(ro_dark::PRIMARY_STROKE)
        Pickable
        ChatAction::Jump
        on(on_chat_button)
        Children [
            @chrome_text(label, 11.0, ro_dark::STRONG)
            --
            @glyph_icon("chev", 9.0, ro_dark::STRONG)
            down
        ]
    }
}
