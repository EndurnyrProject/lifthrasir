//! Idiomatic BSN chrome for the NPC dialogue window: a fixed, bottom-center,
//! single-instance window built as one `bsn!` tree. [`window`] spawns the whole
//! chrome (wrapper, card, titlebar, first body) on the first frame of a
//! conversation; [`body`] alone is spawned again under the captured card entity to
//! rebuild the body region on every later frame.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use net_contract::dto::NpcDialogExpect;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_PANEL_BG, TOKEN_PANEL_BORDER, TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG,
    TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};

use super::{
    on_footer_button, on_text_click, FooterButtonAction, NpcDialogBody, NpcDialogParts,
    NpcDialogRoot, NpcDialogTitle, NpcInputField, Typewriter,
};

const WINDOW_WIDTH: f32 = 560.0;
const WINDOW_BOTTOM: f32 = 100.0;

/// Floats above normal HUD windows (which default to z=0) but stays below the
/// settings modal (`GlobalZIndex(1000)`), matching the other top-level windows'
/// precedent (`system_dialog.rs` at `i32::MAX - 2`).
const WINDOW_Z: i32 = 900;

/// The whole window: wrapper, card, titlebar, and the first body. Spawned once per
/// conversation, at top level — it doesn't cover any world-click area outside its
/// own bounds, so it needs no HUD parent.
pub fn window(
    title: String,
    text: String,
    expect: NpcDialogExpect,
    options: Vec<String>,
) -> impl Scene {
    bsn! {
        NpcDialogRoot
        NpcDialogParts { card: #Card }
        Node {
            position_type: PositionType::Absolute,
            bottom: px(WINDOW_BOTTOM),
            width: percent(100),
            justify_content: JustifyContent::Center,
        }
        GlobalZIndex(WINDOW_Z)
        ignore_picking()
        Children [ ( #Card card(title, text, expect, options) ) ]
    }
}

/// Just the swappable body region, spawned as a child of the existing card entity
/// on every later frame of a conversation.
pub fn body(text: String, expect: NpcDialogExpect, options: Vec<String>) -> impl Scene {
    body_scene(text, expect, options)
}

fn card(title: String, text: String, expect: NpcDialogExpect, options: Vec<String>) -> impl Scene {
    bsn! {
        Node {
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        Children [ titlebar(title), body_scene(text, expect, options) ]
    }
}

fn titlebar(title: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::axes(px(10), px(7))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        Children [
            (
                NpcDialogTitle
                Text(title)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 11.0, theme::TEXT_DIM) } }
                Node { width: px(20), height: px(16) }
                template_value(FooterButtonAction::CloseOrCancel)
                on(on_footer_button)
            ),
        ]
    }
}

fn body_scene(text: String, expect: NpcDialogExpect, options: Vec<String>) -> impl Scene {
    let input = matches!(
        expect,
        NpcDialogExpect::InputInt | NpcDialogExpect::InputStr
    )
    .then(|| EntityScene(input_field()));
    bsn! {
        NpcDialogBody
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::axes(px(14), px(12))},
        }
        ignore_picking()
        Children [ dialog_text(text), {input}, footer_row(expect, options) ]
    }
}

/// The `EditableText` field for `INPUT_INT`/`INPUT_STR`: free-entry (the proto
/// carries no min/max for `INPUT_INT`), marked so `Confirm` can read its value.
fn input_field() -> impl Scene {
    bsn! {
        NpcInputField
        EditableText
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        ThemeTextColor({TOKEN_TEXT})
        Node {
            height: px(28),
            padding: {UiRect::axes(px(10), px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        ThemeBackgroundColor({TOKEN_PANEL_BG})
        ThemeBorderColor({TOKEN_PANEL_BORDER})
    }
}

/// The dialogue text entity: starts empty, its typewriter reveal driven by
/// `typewriter_reveal` each frame. Pickable (not `ignore_picking`) so clicking it
/// fires `on_text_click` and skips straight to the full line. `full` keeps the
/// `^RRGGBB` codes intact so the reveal can recompute colored runs via
/// [`crate::rich_text::parse_color_codes`] at any length (see [`slice_colored_runs`]).
fn dialog_text(text: String) -> impl Scene {
    bsn! {
        Typewriter { full: text }
        Text(String::new())
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(theme::TEXT)
        Pickable
        on(on_text_click)
    }
}

pub(super) fn text_span(content: String, color: Color) -> impl Scene {
    bsn! {
        TextSpan(content)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(color)
    }
}

/// Slices already-color-parsed `runs` (see [`crate::rich_text::parse_color_codes`]) down to the
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

/// The footer buttons for `expect`: `[Close, Next]` for `NEXT`, `[Close]` for
/// `CLOSE` (terminal), one button per `options` entry plus `[Leave]` for `MENU`,
/// and `[Cancel, Confirm]` for `INPUT_INT`/`INPUT_STR`.
fn footer_row(expect: NpcDialogExpect, options: Vec<String>) -> impl Scene {
    let buttons: Vec<_> = match expect {
        NpcDialogExpect::Menu => menu_buttons(&options),
        NpcDialogExpect::InputInt | NpcDialogExpect::InputStr => owned_buttons(input_buttons()),
        NpcDialogExpect::Next | NpcDialogExpect::Close => owned_buttons(footer_buttons(expect)),
    }
    .into_iter()
    .map(|(label, action)| footer_button(label, action))
    .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: px(8),
        }
        ignore_picking()
        Children [ {buttons} ]
    }
}

fn owned_buttons(
    pairs: Vec<(&'static str, FooterButtonAction)>,
) -> Vec<(String, FooterButtonAction)> {
    pairs
        .into_iter()
        .map(|(label, action)| (label.to_string(), action))
        .collect()
}

/// `[Close, Next]` for `NEXT`, `[Close]` for `CLOSE` (terminal). Only ever called
/// with these two variants — `MENU` and `INPUT_INT`/`INPUT_STR` build their own
/// buttons (`menu_buttons`/`input_buttons`) in `footer_row` instead.
fn footer_buttons(expect: NpcDialogExpect) -> Vec<(&'static str, FooterButtonAction)> {
    match expect {
        NpcDialogExpect::Next => vec![
            ("Close", FooterButtonAction::CloseOrCancel),
            ("Next", FooterButtonAction::Continue),
        ],
        NpcDialogExpect::Close => vec![("Close", FooterButtonAction::CloseOrCancel)],
        _ => Vec::new(),
    }
}

/// `[Cancel, Confirm]` for `INPUT_INT`/`INPUT_STR`: `Cancel` ends the conversation,
/// `Confirm` submits the field's current value.
fn input_buttons() -> Vec<(&'static str, FooterButtonAction)> {
    vec![
        ("Cancel", FooterButtonAction::CloseOrCancel),
        ("Confirm", FooterButtonAction::Confirm),
    ]
}

/// `(label, action)` pairs for a `MENU` frame: one `Choice(i + 1)` button per
/// option, in render order (the server's `Choice` is 1-based), plus a trailing
/// `Leave` button that cancels the conversation. Empty `options` still yields
/// `Leave` alone.
fn menu_buttons(options: &[String]) -> Vec<(String, FooterButtonAction)> {
    let mut buttons: Vec<_> = options
        .iter()
        .enumerate()
        .map(|(i, label)| (label.clone(), FooterButtonAction::Choice(i as u32 + 1)))
        .collect();
    buttons.push(("Leave".to_string(), FooterButtonAction::CloseOrCancel));
    buttons
}

fn footer_button(label: String, action: FooterButtonAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label) } }
        template_value(action)
        Node { width: px(72), height: px(24) }
        on(on_footer_button)
    }
}

fn chrome_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}

/// A square white SVG glyph tinted with `color`. `ImageNode` has no theme-token
/// tint, so glyph colors stay raw palette values.
fn glyph_icon(name: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(size), height: px(size) }
        ignore_picking()
    }
}

/// `Pickable::IGNORE` as a scene, so non-interactive nodes don't swallow clicks.
fn ignore_picking() -> impl Scene {
    bsn! {
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_frame_shows_close_and_next() {
        let buttons = footer_buttons(NpcDialogExpect::Next);
        assert_eq!(
            buttons,
            vec![
                ("Close", FooterButtonAction::CloseOrCancel),
                ("Next", FooterButtonAction::Continue),
            ]
        );
    }

    #[test]
    fn close_frame_shows_only_close() {
        let buttons = footer_buttons(NpcDialogExpect::Close);
        assert_eq!(buttons, vec![("Close", FooterButtonAction::CloseOrCancel)]);
    }

    #[test]
    fn input_frame_shows_cancel_and_confirm() {
        assert_eq!(
            input_buttons(),
            vec![
                ("Cancel", FooterButtonAction::CloseOrCancel),
                ("Confirm", FooterButtonAction::Confirm),
            ]
        );
    }

    #[test]
    fn menu_buttons_map_render_index_to_one_based_choice() {
        let options = vec!["Yes".to_string(), "No".to_string(), "Maybe".to_string()];
        let buttons = menu_buttons(&options);
        assert_eq!(
            buttons,
            vec![
                ("Yes".to_string(), FooterButtonAction::Choice(1)),
                ("No".to_string(), FooterButtonAction::Choice(2)),
                ("Maybe".to_string(), FooterButtonAction::Choice(3)),
                ("Leave".to_string(), FooterButtonAction::CloseOrCancel),
            ]
        );
    }

    #[test]
    fn menu_buttons_empty_options_still_has_leave() {
        let buttons = menu_buttons(&[]);
        assert_eq!(
            buttons,
            vec![("Leave".to_string(), FooterButtonAction::CloseOrCancel)]
        );
    }

    #[test]
    fn slice_zero_chars_yields_nothing() {
        let runs = vec![(theme::TEXT, "hello".to_string())];
        assert_eq!(slice_colored_runs(&runs, 0), Vec::new());
    }

    #[test]
    fn slice_mid_run_truncates_in_place() {
        let runs = vec![
            (theme::TEXT, "hello ".to_string()),
            (Color::WHITE, "world".to_string()),
        ];
        assert_eq!(
            slice_colored_runs(&runs, 8),
            vec![
                (theme::TEXT, "hello ".to_string()),
                (Color::WHITE, "wo".to_string()),
            ]
        );
    }

    #[test]
    fn slice_past_end_returns_every_run_unchanged() {
        let runs = vec![
            (theme::TEXT, "hello ".to_string()),
            (Color::WHITE, "world".to_string()),
        ];
        assert_eq!(slice_colored_runs(&runs, 100), runs);
    }

    #[test]
    fn slice_exact_boundary_between_runs() {
        let runs = vec![
            (theme::TEXT, "hi".to_string()),
            (Color::WHITE, "there".to_string()),
        ];
        assert_eq!(
            slice_colored_runs(&runs, 2),
            vec![(theme::TEXT, "hi".to_string())]
        );
    }

    #[test]
    fn slice_counts_unicode_scalars_not_bytes() {
        let runs = vec![(theme::TEXT, "\u{00e9}\u{00e9}\u{00e9}".to_string())];
        assert_eq!(
            slice_colored_runs(&runs, 2),
            vec![(theme::TEXT, "\u{00e9}\u{00e9}".to_string())]
        );
    }
}
