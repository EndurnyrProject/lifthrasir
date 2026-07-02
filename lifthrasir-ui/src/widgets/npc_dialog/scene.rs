//! Idiomatic BSN chrome for the NPC dialogue window: a fixed, bottom-center,
//! single-instance window built as one `bsn!` tree. [`window`] spawns the whole
//! chrome (wrapper, card, titlebar, first body) on the first frame of a
//! conversation; [`body`] alone is spawned again under the captured card entity to
//! rebuild the body region on every later frame.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use net_contract::dto::NpcDialogExpect;

use crate::rich_text::parse_color_codes;
use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};

use super::{
    on_footer_button, FooterButtonAction, NpcDialogBody, NpcDialogParts, NpcDialogRoot,
    NpcDialogTitle,
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
pub fn window(title: String, text: String, expect: NpcDialogExpect) -> impl Scene {
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
        Children [ ( #Card card(title, text, expect) ) ]
    }
}

/// Just the swappable body region, spawned as a child of the existing card entity
/// on every later frame of a conversation.
pub fn body(text: String, expect: NpcDialogExpect) -> impl Scene {
    body_scene(text, expect)
}

fn card(title: String, text: String, expect: NpcDialogExpect) -> impl Scene {
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
        Children [ titlebar(title), body_scene(text, expect) ]
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

fn body_scene(text: String, expect: NpcDialogExpect) -> impl Scene {
    bsn! {
        NpcDialogBody
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::axes(px(14), px(12))},
        }
        ignore_picking()
        Children [ dialog_text(text), footer_row(expect) ]
    }
}

/// Splits `text` into `^RRGGBB`-colored runs (see [`crate::rich_text`]) and renders
/// them as one `Text` root plus a `TextSpan` per following run, so multi-color
/// dialogue stays a single text layout block.
fn dialog_text(text: String) -> impl Scene {
    let mut runs = parse_color_codes(&text, theme::TEXT).into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((theme::TEXT, String::new()));
    let spans: Vec<_> = runs
        .map(|(color, content)| text_span(content, color))
        .collect();
    bsn! {
        Text(first_text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(first_color)
        ignore_picking()
        Children [ {spans} ]
    }
}

fn text_span(content: String, color: Color) -> impl Scene {
    bsn! {
        TextSpan(content)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(color)
    }
}

/// The footer buttons for `expect`: `[Close, Next]` for `NEXT`, `[Close]` for
/// `CLOSE` (terminal), and none for the frames Tasks 7/8 add (`MENU`/`INPUT_*`), so
/// those render as a text-only placeholder instead of panicking.
fn footer_row(expect: NpcDialogExpect) -> impl Scene {
    let buttons: Vec<_> = footer_buttons(expect)
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

fn footer_buttons(expect: NpcDialogExpect) -> Vec<(&'static str, FooterButtonAction)> {
    match expect {
        NpcDialogExpect::Next => vec![
            ("Close", FooterButtonAction::CloseOrCancel),
            ("Next", FooterButtonAction::Continue),
        ],
        NpcDialogExpect::Close => vec![("Close", FooterButtonAction::CloseOrCancel)],
        NpcDialogExpect::Menu | NpcDialogExpect::InputInt | NpcDialogExpect::InputStr => Vec::new(),
    }
}

fn footer_button(label: &'static str, action: FooterButtonAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label) } }
        template_value(action)
        Node { width: px(72), height: px(24) }
        on(on_footer_button)
    }
}

fn chrome_text(text: &'static str) -> impl Scene {
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
    fn placeholder_frames_show_no_footer_buttons() {
        assert!(footer_buttons(NpcDialogExpect::Menu).is_empty());
        assert!(footer_buttons(NpcDialogExpect::InputInt).is_empty());
        assert!(footer_buttons(NpcDialogExpect::InputStr).is_empty());
    }
}
