//! RO-dark art primitives: 9-sliced windows, inputs, buttons, checkboxes and dividers.
//!
//! The art is the 2x cut of `lifthrasir-ui-ro/primitives`, shipped under
//! `ro://ui/primitives/<category>/` with the same layout. Shadows and glows are baked
//! into every PNG, so the sizes below include them. Geometry comes from the design
//! source (`tools/primitives/prims.py`), not from rendered mockups.
//!
//! Every sliced primitive is drawn over the node's *border box*: Bevy 0.20 defaults
//! `ImageNode` to the content box, which shrinks the art by the node's own padding.
//! [`PrimitivesPlugin`] runs the hover/press and checkbox art swaps.

use bevy::picking::hover::Hovered;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::sprite::{BorderRect, TextureSlicer};
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{Pressed, VisualBox};
use bevy::ui_widgets::Button;

use crate::theme::{self, ro_dark};
use crate::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};

pub const PRIMITIVES_DIR: &str = "ro://ui/primitives/";
/// The art is the 2x cut and slicing runs in logical px, so capping the corner scale
/// at 0.5 keeps corners at their 1x design size on any DPI.
const ART_SCALE: f32 = 0.5;
/// Baked shadow around each shape (`pad` in `prims.py`), 1x px.
pub const WINDOW_SHADOW: f32 = 22.0;
pub const INPUT_SHADOW: f32 = 6.0;
pub const BUTTON_SHADOW: f32 = 12.0;
/// Window content inset from the shape edge; the title bar sits 4px in and is 36 tall.
pub const CONTENT_INSET: f32 = 24.0;
const TITLE_BAR_HEIGHT: f32 = 40.0;
const INPUT_HEIGHT: f32 = 28.0;
const BUTTON_HEIGHT: f32 = 32.0;

pub struct PrimitivesPlugin;

impl Plugin for PrimitivesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (style_buttons, render_checkboxes));
    }
}

/// The three art states of a [`button`], relative to [`PRIMITIVES_DIR`].
#[derive(Component, Clone)]
pub struct ButtonArt {
    pub normal: &'static str,
    pub hover: &'static str,
    pub pressed: &'static str,
}

impl ButtonArt {
    pub const NORMAL: Self = Self {
        normal: "button/button_normal.png",
        hover: "button/button_hover.png",
        pressed: "button/button_pressed.png",
    };
    pub const PRIMARY: Self = Self {
        normal: "button/button_primary.png",
        hover: "button/button_primary_hover.png",
        pressed: "button/button_primary_pressed.png",
    };
}

/// The state of a [`checkbox`]. Clicking the row flips it; read it to get the value.
#[derive(Component, Default, Clone)]
pub struct ArtCheckbox(pub bool);

/// A 9-sliced primitive (`path` relative to [`PRIMITIVES_DIR`]) drawn over the whole
/// node, including its padding. Picking is left to the caller: a bare `Pickable` in
/// `bsn!` is an empty patch, so an ignore baked in here could not be undone by the
/// buttons and fields that use it.
pub fn sliced(path: &'static str, border: f32) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{PRIMITIVES_DIR}{path}")},
            image_mode: {slice_mode(border)},
            visual_box: VisualBox::BorderBox,
        }
    }
}

fn slice_mode(border: f32) -> NodeImageMode {
    NodeImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(border),
        max_corner_scale: ART_SCALE,
        ..default()
    })
}

/// The titled window frame with its content padding. Callers add their layout in their
/// own `Node` patch and start the children with [`window_title`].
pub fn window_titled() -> impl Scene {
    bsn! {
        @sliced("window/window_titled.png", 2.0 * (WINDOW_SHADOW + 52.0))
        Node {
            padding: {UiRect::new(
                px(WINDOW_SHADOW + CONTENT_INSET),
                px(WINDOW_SHADOW + CONTENT_INSET),
                px(WINDOW_SHADOW),
                px(WINDOW_SHADOW + 16.0),
            )},
        }
    }
}

/// The title row of a [`window_titled`], aligned to the art's title bar.
pub fn window_title(text: &'static str) -> impl Scene {
    bsn! {
        Node {
            height: px(TITLE_BAR_HEIGHT),
            align_items: AlignItems::Center,
            padding: {UiRect::left(px(46.0 - CONTENT_INSET))},
            margin: {UiRect::bottom(px(6))},
        }
        Children [ @title_text(text, 17.0) ]
    }
}

/// Text in the title font and colour.
pub fn title_text(text: &'static str, size: f32) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(size)},
        }
        TextColor(ro_dark::TITLE)
        @ignore_picking()
    }
}

/// A text-input frame with a hidden focus [`Outline`] on the shape edge. The owner
/// adds picking, fills the child text and colours the outline on focus.
pub fn input_frame() -> impl Scene {
    bsn! {
        @sliced("panel/input_normal.png", 2.0 * (INPUT_SHADOW + 8.0))
        Outline { width: px(1), offset: px(-INPUT_SHADOW), color: Color::NONE }
        Node {
            height: px(INPUT_HEIGHT + 2.0 * INPUT_SHADOW),
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(INPUT_SHADOW + 10.0))},
            overflow: Overflow::clip(),
        }
    }
}

/// A text [`button_with`].
pub fn button(art: ButtonArt, label: &'static str) -> impl Scene {
    button_with(
        art,
        bsn! { @chrome_text(label.into(), 13.0, ro_dark::BUTTON_LABEL) },
    )
}

/// A [`button_with`] showing the `ui/icons/<icon>.svg` glyph instead of a label.
pub fn icon_button(art: ButtonArt, icon: &'static str) -> impl Scene {
    button_with(art, bsn! { @glyph_icon(icon, 18.0, ro_dark::BUTTON_LABEL) })
}

/// A headless [`Button`] drawn with `art` around `content`, growing to share its row.
/// `Hovered` is required for the button to track hover; the widget inserts `Pressed`
/// and emits `Activate` on click.
fn button_with(art: ButtonArt, content: impl Scene) -> impl Scene {
    let normal = art.normal;
    bsn! {
        Button
        Hovered
        art
        @sliced(normal, 2.0 * (BUTTON_SHADOW + 8.0))
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            height: px(BUTTON_HEIGHT + 2.0 * BUTTON_SHADOW),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Pickable
        Children [ @content ]
    }
}

/// A labelled checkbox, unchecked. Clicking anywhere on the row toggles it.
pub fn checkbox(label: &'static str) -> impl Scene {
    bsn! {
        ArtCheckbox(false)
        Pickable
        Node { align_items: AlignItems::Center, column_gap: px(4) }
        on(|click: On<PointerClick>, mut boxes: Query<&mut ArtCheckbox>| {
            if let Ok(mut checkbox) = boxes.get_mut(click.entity) {
                checkbox.0 = !checkbox.0;
            }
        })
        Children [
            ImageNode { image: {format!("{PRIMITIVES_DIR}control/checkbox_off.png")} }
            Node { width: px(34), height: px(34) }
            @ignore_picking()
            --
            @chrome_text(label.into(), 12.0, ro_dark::BODY)
        ]
    }
}

/// A horizontal divider: two stretched lines around `center`.
pub fn divider(center: impl Scene) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        Children [ @divider_line() -- @center -- @divider_line() ]
    }
}

/// The diamond ornament for the centre of a [`divider`].
pub fn divider_diamond() -> impl Scene {
    bsn! {
        ImageNode { image: {format!("{PRIMITIVES_DIR}decor/divider_diamond.png")} }
        Node { width: px(14), height: px(14) }
        @ignore_picking()
    }
}

fn divider_line() -> impl Scene {
    bsn! {
        @sliced("decor/divider_line.png", 6.0)
        Node { flex_grow: 1.0, height: px(8) }
        @ignore_picking()
    }
}

type ButtonState<'a> = (
    Entity,
    Option<Ref<'a, Pressed>>,
    Ref<'a, Hovered>,
    &'a ButtonArt,
    &'a mut ImageNode,
);

/// Swaps button art on hover/press changes. `Hovered` always exists so `is_changed`
/// covers hover transitions and the `Pressed` insertion; its removal is not reported by
/// change detection, so buttons that just lost `Pressed` are restyled too.
fn style_buttons(
    mut buttons: Query<ButtonState>,
    mut removed_pressed: RemovedComponents<Pressed>,
    assets: Res<AssetServer>,
) {
    let just_unpressed: HashSet<Entity> = removed_pressed.read().collect();
    for (entity, pressed, hovered, art, mut image) in &mut buttons {
        let changed = hovered.is_changed()
            || pressed.as_ref().is_some_and(Ref::is_changed)
            || just_unpressed.contains(&entity);
        if !changed {
            continue;
        }
        let path = match (pressed.is_some(), hovered.get()) {
            (true, _) => art.pressed,
            (false, true) => art.hover,
            (false, false) => art.normal,
        };
        image.image = assets.load(format!("{PRIMITIVES_DIR}{path}"));
    }
}

/// Swaps the checkbox art. `checkbox_on` bakes a 2px wider glow, so its node grows by
/// that and pulls its margin in to keep the row in place.
fn render_checkboxes(
    boxes: Query<(&ArtCheckbox, &Children), Changed<ArtCheckbox>>,
    mut images: Query<(&mut ImageNode, &mut Node)>,
    assets: Res<AssetServer>,
) {
    for (checkbox, children) in &boxes {
        let (path, size, glow) = if checkbox.0 {
            ("control/checkbox_on.png", 38.0, 2.0)
        } else {
            ("control/checkbox_off.png", 34.0, 0.0)
        };
        for child in children.iter() {
            let Ok((mut image, mut node)) = images.get_mut(child) else {
                continue;
            };
            image.image = assets.load(format!("{PRIMITIVES_DIR}{path}"));
            node.width = px(size);
            node.height = px(size);
            node.margin = UiRect::all(px(-glow));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_toggle_swaps_art() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_systems(Update, render_checkboxes);
        let row = app.world_mut().commands().spawn_scene(checkbox("x")).id();
        app.update();

        app.world_mut().get_mut::<ArtCheckbox>(row).unwrap().0 = true;
        app.update();

        let world = app.world();
        let image = world
            .get::<Children>(row)
            .unwrap()
            .iter()
            .find_map(|c| world.get::<ImageNode>(c))
            .unwrap();
        let path = image.image.path().unwrap().to_string();
        assert!(path.ends_with("control/checkbox_on.png"), "{path}");
    }
}
