//! Idiomatic BSN chrome for the equipment window.
//!
//! The whole window is one declarative `bsn!` tree built from composable scene
//! functions ([`equipment_window`] -> [`titlebar`] / [`body`] / [`footer`], with
//! [`slot_column`] -> [`slot_well`]). Buttons are `@FeathersButton` scene components,
//! interaction is wired with inline `on(...)` observers, colors resolve through the
//! Norse theme tokens, and slot child entities are captured into [`EquipSlotParts`]
//! via `#Name` references. The only imperative step is a single `ChildOf` insert that
//! parents the resolved window under the HUD root.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor, ThemeToken};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::preview::{on_rotate_left, on_rotate_right};
use super::slots::{on_slot_click, on_slot_hover_out, on_slot_hover_over};
use super::{
    EquipSlotParts, EquipSlotRefine, EquipSlotWell, EquipmentPreviewFrame, EquipmentTitlebar,
    EquipmentWindowRoot, RotateLeftButton, RotateRightButton, SlotSpec, LEFT_SLOTS, RIGHT_SLOTS,
};

/// Spawn the whole window as one scene and parent it under `parent` with a single insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(equipment_window())
        .insert(ChildOf(parent));
}

fn equipment_window() -> impl Scene {
    bsn! {
        EquipmentWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(360),
            top: px(90),
            width: Val::Auto,
            align_items: AlignItems::Stretch,
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [ titlebar(), body() ]
    }
}

fn titlebar() -> impl Scene {
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
        EquipmentTitlebar
        Pickable
        on(on_titlebar_drag)
        Children [
            glyph_icon("rune", 13.0, theme::GOLD),
            (
                Text("Equipment")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(12.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("minus", 11.0, theme::TEXT_DIM) } }
                Node { width: px(20), height: px(16) }
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 11.0, theme::TEXT_DIM) } }
                Node { width: px(20), height: px(16) }
                on(on_close)
            ),
        ]
    }
}

fn body() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            column_gap: px(12),
            padding: {UiRect::axes(px(12), px(10))},
        }
        ignore_picking()
        Children [
            slot_column(&LEFT_SLOTS),
            center(),
            slot_column(&RIGHT_SLOTS),
        ]
    }
}

fn center() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(6),
            padding: {UiRect::horizontal(px(4))},
        }
        ignore_picking()
        Children [
            (
                EquipmentPreviewFrame
                Node {
                    width: px(165),
                    height: px(220),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                ThemeBackgroundColor({theme::feathers_theme::TOKEN_PANEL_BG})
                ThemeBorderColor({TOKEN_WINDOW_BORDER})
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                }
                ignore_picking()
                Children [
                    (
                        @FeathersButton { @caption: bsn! { glyph_icon("rotl", 12.0, theme::TEXT_DIM) } }
                        RotateLeftButton
                        Node { width: px(24), height: px(20) }
                        on(on_rotate_left)
                    ),
                    chrome_text("Rotate", "fonts/manrope.ttf", 10.0, TOKEN_TEXT_DIM),
                    (
                        @FeathersButton { @caption: bsn! { glyph_icon("rotr", 12.0, theme::TEXT_DIM) } }
                        RotateRightButton
                        Node { width: px(24), height: px(20) }
                        on(on_rotate_right)
                    ),
                ]
            ),
        ]
    }
}

fn slot_column(slots: &'static [SlotSpec]) -> impl Scene {
    let wells: Vec<_> = slots.iter().map(|spec| slot_well(*spec)).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        ignore_picking()
        Children [ {wells} ]
    }
}

/// One slot well: a bordered icon box holding the empty-state glyph, the item icon, and
/// the refine badge. The three patched children are named with `#` references and
/// captured into [`EquipSlotParts`]; the well itself carries the slot kind and the
/// hover / double-click observers. The item name shows on hover, not as a caption.
fn slot_well(spec: SlotSpec) -> impl Scene {
    bsn! {
        EquipSlotWell
        Node {
            width: px(40),
            height: px(40),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        ThemeBackgroundColor({theme::feathers_theme::TOKEN_PANEL_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        template_value(spec.kind)
        Pickable
        EquipSlotParts { glyph: #Glyph, icon: #Icon, refine: #Refine }
        on(on_slot_click)
        on(on_slot_hover_over)
        on(on_slot_hover_out)
        Children [
            ( #Glyph glyph_icon(spec.glyph, 22.0, theme::TEXT_FAINT) ),
            (
                #Icon
                ImageNode
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                }
                Visibility::Hidden
                ignore_picking()
            ),
            (
                #Refine
                EquipSlotRefine
                Text("")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(9.0)},
                }
                TextColor({theme::GOLD})
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3),
                    top: px(2),
                }
                Visibility::Hidden
                ignore_picking()
            ),
        ]
    }
}

/// Static themed label: color from a theme token, font loaded by asset path (Feathers
/// has no font token, only inheritance, which our standalone text can't reach).
fn chrome_text(text: &'static str, font: &'static str, size: f32, token: ThemeToken) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: {FontSize::Px(size)},
        }
        ThemeTextColor(token)
        ignore_picking()
    }
}

/// A square white SVG glyph tinted with `color`. `ImageNode` has no theme-token tint,
/// so glyph colors stay raw palette values.
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

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<EquipmentWindowRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Drag the single equipment window root by the titlebar; mirrors `make_draggable` but
/// resolves the root from its marker instead of a captured entity, so the whole window
/// can spawn as one scene with no imperative drag wiring. Only the titlebar itself moves
/// the window: `Pointer<Drag>` bubbles up from the close/minimize buttons, so a drag
/// targeting a child button is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<EquipmentTitlebar>>,
    mut roots: Query<&mut Node, With<EquipmentWindowRoot>>,
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
    use super::super::EquipSlotKind;
    use super::*;
    use bevy::scene::ScenePlugin;

    #[test]
    fn slot_well_captures_its_child_entities_into_parts() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();

        let spec = SlotSpec {
            kind: EquipSlotKind::Body,
            label: "Body",
            glyph: "armor",
        };
        let root = app
            .world_mut()
            .spawn_scene(slot_well(spec))
            .expect("slot well spawns")
            .id();

        let world = app.world();
        let kind = world.get::<EquipSlotKind>(root).expect("slot carries kind");
        assert_eq!(*kind, EquipSlotKind::Body);

        let parts = world
            .get::<EquipSlotParts>(root)
            .expect("slot carries parts");
        assert_ne!(
            parts.glyph, parts.icon,
            "glyph and icon are distinct entities"
        );
        assert!(world.get::<ImageNode>(parts.glyph).is_some());
        assert!(world.get::<ImageNode>(parts.icon).is_some());
        assert_eq!(
            world.get::<Text>(parts.refine).map(|t| t.0.as_str()),
            Some("")
        );
    }
}
