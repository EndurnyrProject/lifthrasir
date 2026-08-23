//! BSN chrome for the skill selection menu: a fixed, bottom-center card built as
//! one `bsn!` tree and respawned wholesale on every change (see
//! [`super::sync_window`]). Deliberately mirrors the NPC dialogue window's
//! geometry and row styling — both are "the server is waiting for one answer"
//! windows and the player reads them the same way.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::chrome::ignore_picking;

use super::{SkillMenuAction, SkillMenuRoot, on_menu_row};

const WINDOW_WIDTH: f32 = 320.0;
const WINDOW_BOTTOM: f32 = 100.0;

/// Same layer as the NPC dialogue window: above the HUD, below the settings modal.
const WINDOW_Z: i32 = 900;

/// The whole window: wrapper, card, titlebar, optional catalyst region, and one
/// row per entry.
pub fn window(
    title: String,
    catalysts: Option<String>,
    catalyst_buttons: Vec<(String, SkillMenuAction)>,
    rows: Vec<(String, SkillMenuAction)>,
) -> impl Scene {
    bsn! {
        SkillMenuRoot
        Node {
            position_type: PositionType::Absolute,
            bottom: px(WINDOW_BOTTOM),
            width: percent(100),
            justify_content: JustifyContent::Center,
        }
        GlobalZIndex(WINDOW_Z)
        ignore_picking()
        Children [ card(title, catalysts, catalyst_buttons, rows) ]
    }
}

fn card(
    title: String,
    catalysts: Option<String>,
    catalyst_buttons: Vec<(String, SkillMenuAction)>,
    rows: Vec<(String, SkillMenuAction)>,
) -> impl Scene {
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
        Children [ titlebar(title), body(catalysts, catalyst_buttons, rows) ]
    }
}

fn titlebar(title: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: {UiRect::axes(px(10), px(7))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            (
                Text(title)
                TextFont {
                    font: FontSourceTemplate::Handle("ro://fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
        ]
    }
}

/// The catalyst region (only present for a forge menu that has chips to show or
/// picks to clear) above the entry rows.
fn body(
    catalysts: Option<String>,
    catalyst_buttons: Vec<(String, SkillMenuAction)>,
    rows: Vec<(String, SkillMenuAction)>,
) -> impl Scene {
    let summary = catalysts.map(|text| EntityScene(summary_text(text)));
    let chips = (!catalyst_buttons.is_empty()).then(|| EntityScene(catalyst_row(catalyst_buttons)));
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(label, action)| menu_row(label, action))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: {UiRect::axes(px(14), px(12))},
        }
        ignore_picking()
        Children [ {summary}, {chips}, ( entry_list() Children [ {rows} ] ) ]
    }
}

fn entry_list() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
        }
        ignore_picking()
    }
}

/// The picked-catalyst line, e.g. `Catalysts: Star Crumb, Flame Heart`.
fn summary_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        TextColor(theme::GOLD)
        ignore_picking()
    }
}

fn catalyst_row(buttons: Vec<(String, SkillMenuAction)>) -> impl Scene {
    let chips: Vec<_> = buttons
        .into_iter()
        .map(|(label, action)| chip(label, action))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(4),
            row_gap: px(4),
            margin: {UiRect { bottom: Val::Px(6.0), ..default() }},
        }
        ignore_picking()
        Children [ {chips} ]
    }
}

/// A catalyst chip: a small bordered button, sized to its label.
fn chip(label: String, action: SkillMenuAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { row_text(label) } }
        template_value(action)
        template_value(ButtonVariant::Normal)
        Node {
            height: px(22),
            padding: {UiRect::axes(px(8), px(0))},
            justify_content: JustifyContent::Center,
        }
        on(on_menu_row)
    }
}

/// An entry row: full-width, left-aligned and chrome-less until hovered, matching
/// the NPC dialogue's MENU rows.
fn menu_row(label: String, action: SkillMenuAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { row_text(label) } }
        template_value(action)
        template_value(ButtonVariant::Plain)
        Node {
            width: percent(100),
            height: px(24),
            justify_content: JustifyContent::FlexStart,
        }
        on(on_menu_row)
    }
}

fn row_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}
