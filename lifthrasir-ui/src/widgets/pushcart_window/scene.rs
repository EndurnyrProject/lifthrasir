//! Idiomatic BSN chrome for the pushcart window (mirrors the inventory/shop
//! windows). [`window`] builds the persistent chrome — root, titlebar, and an
//! empty body region — as one `bsn!` tree; [`body`] renders one of two mount
//! states and is respawned by [`rebuild_body`](super::rebuild_body) on every
//! mount-state / [`Cart`](game_engine::domain::cart::Cart) change.
//!
//! Task 7 leaves the mounted body a placeholder plus the Unmount affordance;
//! Task 8 replaces [`body`] with the live Bag<->Cart panes, mover, and meters.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{
    on_mount_toggle, CartWindowBody, CartWindowRoot, CartWindowTitlebar, MountToggleButton,
};

const WINDOW_LEFT: f32 = 360.0;
const WINDOW_TOP: f32 = 130.0;
const WINDOW_WIDTH: f32 = 420.0;

/// Spawn the whole window as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        CartWindowRoot
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
        Children [ titlebar(), body_container() ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        CartWindowTitlebar
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
        on(on_titlebar_drag)
        Children [
            glyph_icon("cart", 16.0, theme::GOLD),
            (
                Text("Pushcart")
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
                on(on_close)
            ),
        ]
    }
}

/// The (initially empty) body region; [`body`] fills it via `rebuild_body`.
fn body_container() -> impl Scene {
    bsn! {
        CartWindowBody
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect { left: Val::Px(14.0), right: Val::Px(14.0), top: Val::Px(12.0), bottom: Val::Px(14.0) }},
        }
        ignore_picking()
    }
}

/// The swappable body: the mount prompt when the local player has no cart, or the
/// mounted view (Task 8 panes placeholder + Unmount) when they do. Exactly one
/// branch renders.
pub fn body(mounted: bool, cart_empty: bool) -> impl Scene {
    let prompt = (!mounted).then(|| EntityScene(mount_prompt()));
    let mounted_view = mounted.then(|| EntityScene(mounted_body(cart_empty)));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        ignore_picking()
        Children [ {prompt}, {mounted_view} ]
    }
}

/// Shown when the local player has no cart: a line of copy and a single
/// "Mount Pushcart" button that sends `MountCart { mount: true }`.
fn mount_prompt() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10),
            padding: {UiRect::vertical(px(10))},
        }
        ignore_picking()
        Children [
            chrome_text("You have no pushcart mounted.".to_string(), 12.0, theme::TEXT_DIM),
            (
                @FeathersButton { @caption: bsn! { chrome_text("Mount Pushcart".to_string(), 13.0, theme::TEXT) } }
                template_value(MountToggleButton { mount: true })
                Node {
                    height: px(32),
                    padding: {UiRect::horizontal(px(18))},
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(7)),
                }
                BackgroundColor(theme::EMERALD_INK)
                on(on_mount_toggle)
            ),
        ]
    }
}

/// Shown while the cart is mounted: a placeholder for Task 8's Bag<->Cart panes,
/// an optional hint when the cart is non-empty, and the Unmount button. The
/// button stays clickable but its handler drops a non-empty unmount; the dimmed
/// look here mirrors that guard.
fn mounted_body(cart_empty: bool) -> impl Scene {
    let button_bg = theme::FIELD;
    let label_color = if cart_empty {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    let hint = (!cart_empty).then(|| {
        EntityScene(chrome_text(
            "Empty the cart before unmounting.".to_string(),
            11.0,
            theme::WARN,
        ))
    });
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        ignore_picking()
        Children [
            placeholder_panes(),
            {hint},
            (
                @FeathersButton { @caption: bsn! { chrome_text("Unmount".to_string(), 12.0, label_color) } }
                template_value(MountToggleButton { mount: false })
                Node {
                    height: px(30),
                    align_self: AlignSelf::FlexStart,
                    padding: {UiRect::horizontal(px(16))},
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(7)),
                }
                BackgroundColor(button_bg)
                on(on_mount_toggle)
            ),
        ]
    }
}

/// Task 8 fills this with the live Bag<->Cart panes, mover column, detail strip,
/// and footer meters. Until then it is a bordered stub so the mounted window
/// reads as intentionally incomplete rather than broken.
fn placeholder_panes() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            height: px(120),
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.18)})
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [ chrome_text("Cart contents".to_string(), 12.0, theme::TEXT_FAINT) ]
    }
}

/// A plain colored text label with the body font.
fn chrome_text(text: String, size: f32, color: Color) -> impl Scene {
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

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<CartWindowRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Drag the single pushcart window by its titlebar; mirrors the inventory/shop
/// windows. Only the titlebar itself moves the window: `Pointer<Drag>` bubbles up
/// from the close button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<CartWindowTitlebar>>,
    mut roots: Query<&mut Node, With<CartWindowRoot>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    #[test]
    fn window_spawns_root_and_body() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(window())
            .expect("window spawns");
        app.update();

        let roots = app
            .world_mut()
            .query_filtered::<(), With<CartWindowRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(roots, 1, "exactly one CartWindowRoot");

        let bodies = app
            .world_mut()
            .query_filtered::<(), With<CartWindowBody>>()
            .iter(app.world())
            .count();
        assert_eq!(bodies, 1, "exactly one CartWindowBody");
    }

    #[test]
    fn not_mounted_body_carries_a_mount_button() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(body(false, true))
            .expect("body spawns");
        app.update();

        let mounts: Vec<bool> = app
            .world_mut()
            .query::<&MountToggleButton>()
            .iter(app.world())
            .map(|button| button.mount)
            .collect();
        assert_eq!(mounts, vec![true]);
    }

    #[test]
    fn mounted_body_carries_an_unmount_button() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(body(true, true))
            .expect("body spawns");
        app.update();

        let mounts: Vec<bool> = app
            .world_mut()
            .query::<&MountToggleButton>()
            .iter(app.world())
            .map(|button| button.mount)
            .collect();
        assert_eq!(mounts, vec![false]);
    }
}
