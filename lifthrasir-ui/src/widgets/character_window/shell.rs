//! The persistent Console chrome, authored as one `bsn!` tree: root, a draggable
//! titlebar (reusing [`chrome::titlebar`]), an inline tab strip, the identity-strip
//! mount, and the three empty tab-body containers. Live content is projected into
//! the mount and bodies by the per-tab rebuild systems (later tasks); this file only
//! builds the furniture.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::chrome::{
    body_container, chrome_text, drag_window, glyph_icon, ignore_picking,
};
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use super::{
    on_close_click, on_tab_click, BagTabBody, CharacterIdentityMount, CharacterTab,
    CharacterTabBody, CharacterTabButton, CharacterTitlebar, CharacterWindowRoot, SkillsTabBody,
};

const WINDOW_LEFT: f32 = 300.0;
const WINDOW_TOP: f32 = 100.0;
const WINDOW_WIDTH: f32 = 460.0;

/// The tab strip, in strip order: label + the tab it selects.
const TABS: [(&str, CharacterTab); 3] = [
    ("Character", CharacterTab::Character),
    ("Bag", CharacterTab::Bag),
    ("Skills", CharacterTab::Skills),
];

/// Spawn the whole Console as one scene and parent it under `parent` with a single
/// insert (the `inventory_window` idiom).
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        CharacterWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [
            titlebar_row(),
            tab_strip(),
            body_container::<CharacterIdentityMount>(UiRect::axes(px(14), px(8))),
            content_region(),
        ]
    }
}

/// The Console titlebar: a `CharacterTitlebar` drag handle (reusing
/// `chrome::drag_window`) with a close button that clears `CharacterWindowState.open`
/// via [`on_close_click`] — the Console keeps the resource as the sole visibility
/// source, so it cannot reuse `chrome::titlebar`'s direct-hide close button.
fn titlebar_row() -> impl Scene {
    bsn! {
        CharacterTitlebar
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        on(drag_window::<CharacterTitlebar, CharacterWindowRoot>)
        Children [
            glyph_icon("user", 16.0, theme::GOLD),
            (
                Text({"Character".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(15.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_DIM) } }
                Node { width: px(22), height: px(22) }
                on(on_close_click)
            ),
        ]
    }
}

fn tab_strip() -> impl Scene {
    let buttons: Vec<_> = TABS
        .iter()
        .map(|(label, tab)| tab_button(label, *tab))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(6),
            padding: {UiRect::axes(px(14), px(6))},
        }
        ignore_picking()
        Children [ {buttons} ]
    }
}

fn tab_button(label: &'static str, tab: CharacterTab) -> impl Scene {
    bsn! {
        template_value(CharacterTabButton(tab))
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            height: px(28),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        Pickable
        on(on_tab_click)
        Children [ chrome_text(label.to_string(), 12.0, theme::TEXT_DIM) ]
    }
}

/// The stacked tab bodies; the active one is shown by `reflect_window_state`.
fn content_region() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_grow: 1.0 }
        ignore_picking()
        Children [
            body_container::<CharacterTabBody>(UiRect::axes(px(14), px(10))),
            body_container::<BagTabBody>(UiRect::axes(px(14), px(10))),
            body_container::<SkillsTabBody>(UiRect::axes(px(14), px(10))),
        ]
    }
}
