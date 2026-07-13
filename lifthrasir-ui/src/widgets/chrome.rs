//! Shared BSN window-chrome helpers.
//!
//! Every scene-based in-game window draws the same furniture: a `Pickable::IGNORE`
//! scene for non-interactive nodes, tinted SVG glyphs, plain body labels, a
//! draggable titlebar with a close button, and an empty body region filled later by
//! a refresh system. These helpers are the single source for that furniture so the
//! per-window scene files stay to their own content. The drag and close observers
//! are generic over the window's marker components; the titlebar and body container
//! are generic over the same so the four uniform windows (settings, pushcart, party,
//! inventory) share one definition.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::draggable::px_or_zero;

/// `Pickable::IGNORE` as a scene, so non-interactive nodes don't swallow clicks.
pub fn ignore_picking() -> impl Scene {
    bsn! {
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

/// A square white SVG glyph tinted with `color`. `ImageNode` has no theme-token tint,
/// so glyph colors stay raw palette values.
pub fn glyph_icon(name: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(size), height: px(size) }
        ignore_picking()
    }
}

/// A plain colored text label with the body font.
pub fn chrome_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        ignore_picking()
    }
}

/// The uniform window titlebar: gold glyph, cinzel title, and a close button, draggable
/// by the bar itself. `Tb` marks the bar (the drag handle) and `Root` the window root
/// that drags/closes.
pub fn titlebar<Tb, Root>(icon: &'static str, title: impl Into<String>) -> impl Scene
where
    Tb: Component + Default + Clone + Unpin,
    Root: Component,
{
    let title = title.into();
    bsn! {
        template_value(Tb::default())
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
        on(drag_window::<Tb, Root>)
        Children [
            glyph_icon(icon, 16.0, theme::GOLD),
            (
                Text(title)
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
                on(close_window::<Root>)
            ),
        ]
    }
}

/// The empty body region under a titlebar; a refresh system fills it with content on
/// every state change. `M` marks the region and `padding` is its inner padding.
pub fn body_container<M>(padding: UiRect) -> impl Scene
where
    M: Component + Default + Clone + Unpin,
{
    bsn! {
        template_value(M::default())
        Node {
            flex_direction: FlexDirection::Column,
            padding: {padding},
        }
        ignore_picking()
    }
}

/// Close a single-instance window by hiding its `Root`; the titlebar close button's
/// `Activate` observer.
pub fn close_window<Root: Component>(
    _: On<Activate>,
    mut window: Query<&mut Visibility, With<Root>>,
) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Drag a single-instance window by its titlebar. `Tb` marks the drag handle and `Root`
/// the moved window root. `Pointer<Drag>` bubbles up from child buttons, so a drag whose
/// target is not the titlebar itself is ignored.
pub fn drag_window<Tb: Component, Root: Component>(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<Tb>>,
    mut roots: Query<&mut Node, With<Root>>,
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
