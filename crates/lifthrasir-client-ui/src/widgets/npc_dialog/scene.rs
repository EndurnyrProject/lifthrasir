//! The NPC dialogue window as one `bsn!` tree, after the `NPC Dialog` artboard: the
//! log panel over the titled window, whose parchment page holds the typed line and
//! the frame's reply, and whose footer holds the keyed buttons. [`window`] draws all
//! of it from the conversation; every new frame respawns it.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate, LineHeight};
use bevy::ui_widgets::{Button, ScrollArea, TextInput};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, WINDOW_SHADOW, button_with, close_button, diamond,
    parchment, window_title_bar, window_titled,
};
use net_contract::dto::NpcDialogExpect;

use crate::screens::shared::{colored_lines, key_cap};
use crate::widgets::hotbar::{HOTBAR_BOTTOM, HOTBAR_HEIGHT};
use crate::widgets::npc_dialog::{
    ActiveNpcDialog, DIGITS, FooterButtonAction, MenuOption, NpcDialogLog, NpcDialogRoot,
    NpcInputField, ShownWhileTyping, Speaker, Typewriter, TypingLabel, on_footer_button,
    on_log_toggle, on_option_over, on_page_click, parsed_total,
};

const WIDTH: f32 = 640.0;
/// Gap under the window's shape. The artboard draws 22 on an empty screen; this
/// clears the hotbar, which the window would otherwise cover.
const BOTTOM: f32 = HOTBAR_BOTTOM + HOTBAR_HEIGHT + 8.0;
const LOG_WIDTH: f32 = 620.0;
const LOG_MAX_HEIGHT: f32 = 190.0;
/// Past the end of any log, so the panel opens on its latest line. Layout clamps it.
const LOG_END: Vec2 = Vec2::new(0.0, 1.0e6);
const TEXT_SIZE: f32 = 16.0;

/// Floats above normal HUD windows (which default to z=0) but stays below the
/// settings modal (`GlobalZIndex(1000)`), matching the other top-level windows'
/// precedent (`system_dialog.rs` at `i32::MAX - 2`).
const WINDOW_Z: i32 = 900;

/// The log and the window for the frame on screen, bottom-centre. Spawned at top
/// level: it covers no world-click area outside its own bounds, so it needs no HUD
/// parent.
pub fn window(dialog: &ActiveNpcDialog) -> impl Scene + use<> {
    let log = log_panel(log_lines(&dialog.name, &dialog.log), dialog.show_log);
    let title = title_bar(dialog.name.clone(), dialog.show_log);
    let page = page(
        dialog.text.clone(),
        dialog.expect,
        dialog.options.clone(),
        dialog.highlight,
    );
    let footer = footer(dialog.expect, dialog.options.len());
    bsn! {
        NpcDialogRoot
        Node {
            position_type: PositionType::Absolute,
            bottom: px(BOTTOM - WINDOW_SHADOW),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
        }
        GlobalZIndex(WINDOW_Z)
        @ignore_picking()
        Children [
            @log
            --
            @window_titled()
            Node {
                width: px(WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Pickable
            Children [ @title -- @page -- @footer ]
        ]
    }
}

/// The NPC's name with the log switch and the close button at its right end.
fn title_bar(name: String, show_log: bool) -> impl Scene {
    let buttons = bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [
            @log_switch(show_log)
            --
            @close_button()
            FooterButtonAction::CloseOrCancel
            on(on_footer_button)
        ]
    };
    window_title_bar(name, buttons)
}

/// The title bar's `Log` switch, ringed while the log is open.
fn log_switch(open: bool) -> impl Scene {
    let colors = log_switch_colors(open);
    let (fill, stroke) = colors.rest;
    bsn! {
        Hovered
        colors
        Node {
            height: px(22),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(5),
            padding: {UiRect::horizontal(px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(on_log_toggle)
        Children [
            @glyph_icon("scroll", 10.0, ro_dark::LABEL)
            --
            @chrome_text("Log".into(), 11.0, ro_dark::LABEL)
        ]
    }
}

pub(super) fn log_switch_colors(open: bool) -> HoverColors {
    let ring = |stroke| if open { ro_dark::SELECT_RING } else { stroke };
    HoverColors {
        rest: (ro_dark::TAB_INACTIVE, ring(ro_dark::MUTED_BRONZE)),
        hover: (ro_dark::BUTTON, ring(ro_dark::BRONZE_HIGHLIGHT)),
    }
}

/// What was said so far, scrolled to its end. It takes no room while closed.
fn log_panel(lines: Vec<Vec<(Color, String)>>, open: bool) -> impl Scene {
    bsn! {
        NpcDialogLog
        ScrollArea
        ScrollPosition(LOG_END)
        Node {
            display: {log_display(open)},
            width: px(LOG_WIDTH),
            max_height: px(LOG_MAX_HEIGHT),
            flex_direction: FlexDirection::Column,
            margin: {UiRect::bottom(px(8))},
            padding: {UiRect::axes(px(14), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
            overflow: {Overflow::scroll_y()},
        }
        BackgroundColor({ro_dark::INSET.with_alpha(0.94)})
        BorderColor::all(ro_dark::BUTTON_STROKE)
        Pickable
        Children [ @colored_lines(lines) Node { flex_shrink: 0.0 } ]
    }
}

pub(super) fn log_display(open: bool) -> Display {
    if open { Display::Flex } else { Display::None }
}

/// The log's rows: who spoke, then what they said. An empty log says so.
fn log_lines(name: &str, log: &[(Speaker, String)]) -> Vec<Vec<(Color, String)>> {
    if log.is_empty() {
        return vec![vec![(ro_dark::FAINT, "Nothing said yet".to_string())]];
    }
    log.iter()
        .map(|(speaker, line)| {
            let (who, tone) = match speaker {
                Speaker::Npc => (name, ro_dark::BRONZE_LIGHT),
                Speaker::You => ("You", ro_dark::STATUS_ONLINE),
            };
            vec![(tone, format!("{who}  ")), (ro_dark::LABEL, line.clone())]
        })
        .collect()
}

/// The parchment page: the typed line, then the reply the frame expects. Clicking it
/// shows the rest of the line.
fn page(
    text: String,
    expect: NpcDialogExpect,
    options: Vec<String>,
    highlight: usize,
) -> impl Scene {
    let menu = (expect == NpcDialogExpect::Menu).then(|| EntityScene(menu(options, highlight)));
    let field = matches!(
        expect,
        NpcDialogExpect::InputInt | NpcDialogExpect::InputStr
    )
    .then(|| EntityScene(input_field(expect)));
    let more = (expect == NpcDialogExpect::Next).then(|| EntityScene(more_mark()));
    bsn! {
        @parchment()
        Node {
            min_height: px(92),
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
            margin: {UiRect::top(px(6))},
            padding: {UiRect::axes(px(18), px(14))},
        }
        Pickable
        on(on_page_click)
        Children [ @dialog_text(text) -- {menu} -- {field} -- {more} ]
    }
}

/// The serif that text on parchment is set in.
fn prose(size: f32) -> impl Scene {
    bsn! {
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_PROSE),
            font_size: {FontSize::Px(size)},
        }
    }
}

/// The dialogue text entity: starts empty and is typed out by `typewriter_reveal`.
/// `full` keeps the `^RRGGBB` codes intact so the reveal can recompute colored runs
/// at any length (see [`slice_colored_runs`]).
fn dialog_text(text: String) -> impl Scene {
    let (_, total) = parsed_total(&text);
    let typewriter = Typewriter {
        full: text,
        total,
        ..default()
    };
    let leading = LineHeight::RelativeToFont(1.5);
    bsn! {
        typewriter
        Text(String::new())
        @prose(TEXT_SIZE)
        leading
        TextColor(ro_dark::INK)
        @ignore_picking()
    }
}

pub(super) fn text_span(content: String, color: Color) -> impl Scene {
    bsn! {
        TextSpan(content)
        @prose(TEXT_SIZE)
        TextColor(color)
    }
}

/// Slices already-color-parsed `runs` (see [`lifthrasir_ui::rich_text::parse_color_codes`]) down to the
/// first `max_chars` visible characters, preserving each run's color and never
/// splitting inside a UTF-8 scalar. Runs entirely beyond `max_chars` are dropped;
/// the run straddling the boundary is truncated in place.
pub(super) fn slice_colored_runs(
    runs: &[(Color, String)],
    max_chars: usize,
) -> Vec<(Color, String)> {
    let mut remaining = max_chars;
    let mut sliced = Vec::new();
    for (color, content) in runs {
        if remaining == 0 {
            break;
        }
        let len = content.chars().count();
        if len <= remaining {
            sliced.push((*color, content.clone()));
            remaining -= len;
        } else {
            sliced.push((*color, content.chars().take(remaining).collect()));
            remaining = 0;
        }
    }
    sliced
}

/// The options of a `MENU` frame, one numbered row each.
fn menu(options: Vec<String>, highlight: usize) -> impl Scene {
    let rows: Vec<_> = options
        .into_iter()
        .enumerate()
        .map(|(index, label)| option_row(index, label, index == highlight))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(5) }
        @ignore_picking()
        Children [ {rows} ]
    }
}

/// One menu option: its number on a diamond, then its label. The server's `Choice`
/// is 1-based, like the number shown.
fn option_row(index: usize, label: String, lit: bool) -> impl Scene {
    let (fill, stroke) = option_colors(lit);
    let option = MenuOption(index);
    let action = FooterButtonAction::Choice(index as u32 + 1);
    let number = (index + 1).to_string();
    bsn! {
        Button
        option
        action
        Node {
            min_height: px(34),
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::new(px(8), px(12), px(6), px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(on_footer_button)
        on(on_option_over)
        Children [
            Node {
                width: px(20),
                height: px(20),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
            Children [
                @diamond(16.0, ro_dark::BRONZE_SHADOW, ro_dark::BRONZE_SHADOW)
                Node { position_type: PositionType::Absolute, left: px(2), top: px(2) }
                --
                @chrome_text(number, 11.0, ro_dark::HERO)
            ]
            --
            Text(label)
            @prose(14.0)
            TextColor(ro_dark::INK)
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            @ignore_picking()
        ]
    }
}

/// `(fill, stroke)` of a menu option, lit under the pointer or the arrow keys.
pub(super) fn option_colors(lit: bool) -> (Color, Color) {
    if lit {
        (
            ro_dark::PARCHMENT_GLAZE.with_alpha(0.75),
            ro_dark::BRONZE_SHADOW,
        )
    } else {
        (
            ro_dark::PARCHMENT_GLAZE.with_alpha(0.38),
            ro_dark::BRONZE_SHADOW.with_alpha(0.35),
        )
    }
}

/// The field of an `INPUT_INT`/`INPUT_STR` frame. Free entry: the protocol carries
/// neither a range for the number nor a length for the text, so the server judges.
fn input_field(expect: NpcDialogExpect) -> impl Scene {
    let (accepts, width): (fn(char) -> bool, Val) = if expect == NpcDialogExpect::InputInt {
        (|c| c.is_ascii_digit() || c == '-', px(180))
    } else {
        (|_| true, percent(100))
    };
    bsn! {
        NpcInputField
        EditableText
        TextInput
        EditableTextFilter::new(accepts)
        @prose(15.0)
        TextColor(ro_dark::INK)
        Node {
            width: {width},
            height: px(36),
            flex_shrink: 0.0,
            padding: {UiRect::axes(px(12), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor({ro_dark::PARCHMENT_GLAZE.with_alpha(0.7)})
        BorderColor::all(field_stroke(false))
    }
}

/// The input field's stroke, frost while it has the keyboard.
pub(super) fn field_stroke(focused: bool) -> Color {
    if focused {
        ro_dark::PRIMARY
    } else {
        ro_dark::BRONZE_SHADOW.with_alpha(0.6)
    }
}

/// The mark that the page is whole and another follows.
fn more_mark() -> impl Scene {
    let down = UiTransform::from_rotation(Rot2::degrees(90.0));
    bsn! {
        ShownWhileTyping(false)
        @glyph_icon("chev", 12.0, ro_dark::BRONZE_SHADOW)
        Node { position_type: PositionType::Absolute, right: px(12), bottom: px(8) }
        down
        Visibility::Hidden
    }
}

/// The hint for the frame on the left, its buttons on the right.
fn footer(expect: NpcDialogExpect, options: usize) -> impl Scene {
    let skip_hint = (expect == NpcDialogExpect::Next).then(|| {
        EntityScene(bsn! {
            ShownWhileTyping(true)
            @chrome_text("Click the page to show it all".into(), 11.0, ro_dark::FAINT)
        })
    });
    let menu_hint = (expect == NpcDialogExpect::Menu)
        .then(|| EntityScene(chrome_text(menu_hint(options), 11.0, ro_dark::FAINT)));
    let buttons: Vec<_> = footer_buttons(expect)
        .into_iter()
        .map(footer_button)
        .collect();
    bsn! {
        Node {
            min_height: px(32),
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: {UiRect::top(px(12))},
        }
        @ignore_picking()
        Children [
            {skip_hint}
            --
            {menu_hint}
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            {buttons}
        ]
    }
}

/// How a menu of `options` is answered from the keyboard.
fn menu_hint(options: usize) -> String {
    let digits = match options.min(DIGITS.len()) {
        0 | 1 => "1".to_string(),
        last => format!("1\u{2013}{last}"),
    };
    format!("Arrow keys to choose \u{b7} {digits} or Enter to answer")
}

/// One footer button: its label, the key that also triggers it, whether it is the
/// frame's primary action, and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FooterButton {
    label: &'static str,
    key: &'static str,
    primary: bool,
    action: FooterButtonAction,
}

/// The footer of each frame: `[Close, Next]` for `NEXT`, `[Close]` for `CLOSE`
/// (terminal), `[Leave]` under the options of a `MENU`, and `[Cancel, Confirm]` for
/// `INPUT_INT`/`INPUT_STR`. Leaving is always on Esc.
fn footer_buttons(expect: NpcDialogExpect) -> Vec<FooterButton> {
    let leave = |label| FooterButton {
        label,
        key: "Esc",
        primary: false,
        action: FooterButtonAction::CloseOrCancel,
    };
    let primary = |label, key, action| FooterButton {
        label,
        key,
        primary: true,
        action,
    };
    match expect {
        NpcDialogExpect::Next => vec![
            leave("Close"),
            primary("Next", "Space", FooterButtonAction::Continue),
        ],
        NpcDialogExpect::Close => {
            vec![primary("Close", "Space", FooterButtonAction::CloseOrCancel)]
        }
        NpcDialogExpect::Menu => vec![leave("Leave")],
        NpcDialogExpect::InputInt | NpcDialogExpect::InputStr => vec![
            leave("Cancel"),
            primary("Confirm", "Enter", FooterButtonAction::Confirm),
        ],
    }
}

/// An art button as wide as its label and key cap, laid out by its shape.
fn footer_button(button: FooterButton) -> impl Scene {
    let art = if button.primary {
        ButtonArt::PRIMARY
    } else {
        ButtonArt::NORMAL
    };
    let action = button.action;
    let key = button.key;
    // Only `Next` waits for the typewriter: until the line is whole it skips to its end.
    let typing = if action == FooterButtonAction::Continue {
        "Skip"
    } else {
        button.label
    };
    let label = TypingLabel {
        typing,
        done: button.label,
    };
    let content = bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            label
            @chrome_text(typing.into(), 13.0, ro_dark::BUTTON_LABEL)
            --
            @key_cap(key)
        ]
    };
    bsn! {
        @button_with(art, content)
        action
        Node {
            min_width: px(92.0 + 2.0 * BUTTON_SHADOW),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            padding: {UiRect::horizontal(px(BUTTON_SHADOW + 14.0))},
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
        on(on_footer_button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(label, key, action)` of each footer button, in order.
    fn footer(expect: NpcDialogExpect) -> Vec<(&'static str, &'static str, FooterButtonAction)> {
        footer_buttons(expect)
            .into_iter()
            .map(|button| (button.label, button.key, button.action))
            .collect()
    }

    #[test]
    fn next_frame_closes_on_esc_and_turns_the_page_on_space() {
        assert_eq!(
            footer(NpcDialogExpect::Next),
            vec![
                ("Close", "Esc", FooterButtonAction::CloseOrCancel),
                ("Next", "Space", FooterButtonAction::Continue),
            ]
        );
    }

    #[test]
    fn close_frame_shows_only_close() {
        assert_eq!(
            footer(NpcDialogExpect::Close),
            vec![("Close", "Space", FooterButtonAction::CloseOrCancel)]
        );
    }

    #[test]
    fn menu_frame_keeps_leave_out_of_the_options() {
        assert_eq!(
            footer(NpcDialogExpect::Menu),
            vec![("Leave", "Esc", FooterButtonAction::CloseOrCancel)]
        );
    }

    #[test]
    fn input_frames_show_cancel_and_confirm() {
        for expect in [NpcDialogExpect::InputInt, NpcDialogExpect::InputStr] {
            assert_eq!(
                footer(expect),
                vec![
                    ("Cancel", "Esc", FooterButtonAction::CloseOrCancel),
                    ("Confirm", "Enter", FooterButtonAction::Confirm),
                ]
            );
        }
    }

    #[test]
    fn every_frame_has_one_primary_button_at_most_and_it_comes_last() {
        for expect in [
            NpcDialogExpect::Next,
            NpcDialogExpect::Close,
            NpcDialogExpect::Menu,
            NpcDialogExpect::InputInt,
            NpcDialogExpect::InputStr,
        ] {
            let primaries: Vec<_> = footer_buttons(expect)
                .iter()
                .map(|button| button.primary)
                .collect();
            let expected = expect != NpcDialogExpect::Menu;
            assert_eq!(primaries.last(), Some(&expected), "{expect:?}");
            assert!(primaries.iter().filter(|primary| **primary).count() <= 1);
        }
    }

    #[test]
    fn menu_hint_names_the_digits_the_menu_answers_to() {
        assert!(menu_hint(4).contains("1\u{2013}4 or Enter"));
        assert!(menu_hint(1).contains(" 1 or Enter"));
        assert!(menu_hint(20).contains("1\u{2013}9 or Enter"));
    }

    #[test]
    fn log_lines_name_who_spoke() {
        let log = vec![
            (Speaker::Npc, "Anything else?".to_string()),
            (Speaker::You, "Donate".to_string()),
        ];
        let said: Vec<String> = log_lines("Mia", &log)
            .into_iter()
            .map(|runs| runs.into_iter().map(|(_, text)| text).collect())
            .collect();
        assert_eq!(said, vec!["Mia  Anything else?", "You  Donate"]);
        assert_eq!(log_lines("Mia", &[])[0][0].1, "Nothing said yet");
    }

    #[test]
    fn slice_zero_chars_yields_nothing() {
        let runs = vec![(lifthrasir_ui::theme::TEXT, "hello".to_string())];
        assert_eq!(slice_colored_runs(&runs, 0), Vec::new());
    }

    #[test]
    fn slice_mid_run_truncates_in_place() {
        let runs = vec![
            (lifthrasir_ui::theme::TEXT, "hello ".to_string()),
            (Color::WHITE, "world".to_string()),
        ];
        assert_eq!(
            slice_colored_runs(&runs, 8),
            vec![
                (lifthrasir_ui::theme::TEXT, "hello ".to_string()),
                (Color::WHITE, "wo".to_string()),
            ]
        );
    }

    #[test]
    fn slice_past_end_returns_every_run_unchanged() {
        let runs = vec![
            (lifthrasir_ui::theme::TEXT, "hello ".to_string()),
            (Color::WHITE, "world".to_string()),
        ];
        assert_eq!(slice_colored_runs(&runs, 100), runs);
    }

    #[test]
    fn slice_exact_boundary_between_runs() {
        let runs = vec![
            (lifthrasir_ui::theme::TEXT, "hi".to_string()),
            (Color::WHITE, "there".to_string()),
        ];
        assert_eq!(
            slice_colored_runs(&runs, 2),
            vec![(lifthrasir_ui::theme::TEXT, "hi".to_string())]
        );
    }

    #[test]
    fn slice_counts_unicode_scalars_not_bytes() {
        let runs = vec![(
            lifthrasir_ui::theme::TEXT,
            "\u{00e9}\u{00e9}\u{00e9}".to_string(),
        )];
        assert_eq!(
            slice_colored_runs(&runs, 2),
            vec![(lifthrasir_ui::theme::TEXT, "\u{00e9}\u{00e9}".to_string())]
        );
    }
}
