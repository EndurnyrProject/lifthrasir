//! Idiomatic BSN chrome for the NPC shop window: a draggable, single-instance
//! window (design `2026-07-07-npc-shops` §5.4). [`window`] builds the whole chrome
//! — root, titlebar, and an (initially empty) body — as one `bsn!` tree; Task 7
//! fills the body region.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{on_shop_close_button, ShopWindowBody, ShopWindowRoot, ShopWindowTitlebar};

const WINDOW_LEFT: f32 = 340.0;
const WINDOW_TOP: f32 = 90.0;
const WINDOW_WIDTH: f32 = 420.0;

/// The whole window: the root card (draggable, carries [`ShopWindowRoot`]), its
/// titlebar, and an empty body placeholder for Task 7's grid/detail/cart/footer.
pub fn window(title: String) -> impl Scene {
    bsn! {
        ShopWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        Children [ titlebar(title), body() ]
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
        ShopWindowTitlebar
        Pickable
        on(on_titlebar_drag)
        Children [
            glyph_icon("rune", 13.0, theme::GOLD),
            (
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
                on(on_shop_close_button)
            ),
        ]
    }
}

/// The (initially empty) body region; Task 7 rebuilds it from `ShopSession`.
fn body() -> impl Scene {
    bsn! {
        ShopWindowBody
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::axes(px(14), px(12))},
        }
        ignore_picking()
    }
}

/// Drags the single open shop window by its titlebar; mirrors `make_draggable`
/// but resolves the root from its marker instead of a captured entity, so the
/// whole window can spawn as one scene with no imperative drag wiring. Only the
/// titlebar itself moves the window: `Pointer<Drag>` bubbles up from the close
/// button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<ShopWindowTitlebar>>,
    mut roots: Query<&mut Node, With<ShopWindowRoot>>,
) {
    if titlebars.get(drag.entity).is_err() {
        return;
    }
    let Ok(mut node) = roots.single_mut() else {
        return;
    };
    node.left = Val::Px(px_or_zero(node.left) + drag.delta.x);
    node.top = Val::Px(px_or_zero(node.top) + drag.delta.y);
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
