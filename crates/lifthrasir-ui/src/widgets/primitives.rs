//! RO-dark art primitives: 9-sliced windows, inputs, buttons, checkboxes and dividers,
//! plus the scrolling grid windows lay their options out in.
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
use bevy::ui_widgets::{Button, ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;

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
/// Inner padding of a [`scroll_grid`], so a selected cell's glow is not clipped.
const GRID_PAD: f32 = 3.0;
/// Width a [`scroll_grid`] keeps free for its scrollbar: its cells get the container's
/// width minus this.
pub const SCROLL_GRID_GUTTER: f32 = 11.0;

pub struct PrimitivesPlugin;

impl Plugin for PrimitivesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (style_buttons, style_hover, render_checkboxes));
    }
}

/// The `(fill, stroke)` a flat control shows at rest and under the pointer. The node
/// needs `Hovered` for the pointer to be tracked.
#[derive(Component, Clone)]
pub struct HoverColors {
    pub rest: (Color, Color),
    pub hover: (Color, Color),
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

/// The title row of a [`window_titled`] with `trailing` pushed to its right end, 14px
/// in from the shape edge. The caller adds the drag marker and observer.
pub fn window_title_bar(text: impl Into<String>, trailing: impl Scene) -> impl Scene {
    let text = text.into();
    bsn! {
        Node {
            height: px(TITLE_BAR_HEIGHT),
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::left(px(46.0 - CONTENT_INSET))},
            margin: {UiRect { right: px(14.0 - CONTENT_INSET), bottom: px(6), ..default() }},
        }
        Children [
            @display_text(text, 17.0, ro_dark::TITLE)
            Node { flex_grow: 1.0 }
            --
            @trailing
        ]
    }
}

/// A 22px borderless icon button that lights up under the pointer: the remove and
/// dismiss marks of list rows. The caller attaches the `Activate` observer.
pub fn ghost_button(icon: &'static str) -> impl Scene {
    let colors = HoverColors {
        rest: (Color::NONE, Color::NONE),
        hover: (ro_dark::BUTTON, ro_dark::BUTTON_STROKE),
    };
    bsn! {
        Button
        Hovered
        colors
        Node {
            width: px(22),
            height: px(22),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(Color::NONE)
        Pickable
        Children [ @glyph_icon(icon, 9.0, ro_dark::MUTED) ]
    }
}

/// The oxblood window close button. The caller attaches the `Activate` observer.
pub fn close_button() -> impl Scene {
    let colors = HoverColors {
        rest: (ro_dark::CLOSE, ro_dark::TAB_STROKE),
        hover: (ro_dark::CLOSE_HOVER, ro_dark::TAB_STROKE),
    };
    bsn! {
        Button
        Hovered
        colors
        Node {
            width: px(20),
            height: px(20),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
        }
        BackgroundColor(ro_dark::CLOSE)
        BorderColor::all(ro_dark::TAB_STROKE)
        Pickable
        Children [ @glyph_icon("close", 10.0, ro_dark::CLOSE_X) ]
    }
}

/// The recessed panel look that grids, read-outs and value boxes sit in.
pub fn inset() -> impl Scene {
    bsn! {
        Node { border: px(1), border_radius: BorderRadius::all(px(6)) }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(ro_dark::SLOT_STROKE)
    }
}

/// The frame of one tab in a strip over an [`inset`] panel. The active tab drops its
/// bottom stroke so it joins the panel below. The caller adds its marker, the click
/// observer and the children, coloured by [`tab_text`].
pub fn tab(active: bool) -> impl Scene {
    let (fill, stroke, bottom) = if active {
        (ro_dark::TAB_ACTIVE, ro_dark::TAB_STROKE, 0.0)
    } else {
        (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE, 1.0)
    };
    let hover = if active {
        (fill, stroke)
    } else {
        (ro_dark::TAB_HOVER, ro_dark::MUTED_BRONZE)
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover,
    };
    bsn! {
        Hovered
        colors
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(28),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(6),
            border: {UiRect::new(px(1), px(1), px(1), px(bottom))},
            border_radius: {BorderRadius::top(px(5))},
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
    }
}

/// One option of a segmented control: a small pill that is lit while `active`. The
/// caller adds its marker, the click observer and a label coloured by [`tab_text`].
pub fn segment(active: bool) -> impl Scene {
    let (fill, stroke) = if active {
        (ro_dark::TAB_ACTIVE, ro_dark::TAB_STROKE)
    } else {
        (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE)
    };
    let hover = if active {
        (fill, stroke)
    } else {
        (ro_dark::TAB_HOVER, ro_dark::MUTED_BRONZE)
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover,
    };
    bsn! {
        Hovered
        colors
        Node {
            height: px(24),
            padding: {UiRect::horizontal(px(10))},
            align_items: AlignItems::Center,
            column_gap: px(6),
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
    }
}

/// An 18px round button showing the `ui/icons/<icon>.svg` glyph, frost blue when
/// `primary`. The caller positions it and attaches the click observer.
pub fn round_button(icon: &'static str, primary: bool) -> impl Scene {
    let (fill, hover, stroke) = if primary {
        (
            ro_dark::PRIMARY,
            ro_dark::PRIMARY_HOVER,
            ro_dark::PRIMARY_STROKE,
        )
    } else {
        (
            ro_dark::BUTTON,
            ro_dark::BUTTON_HOVER,
            ro_dark::BUTTON_STROKE,
        )
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover: (hover, stroke),
    };
    bsn! {
        Hovered
        colors
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        Children [ @glyph_icon(icon, 8.0, ro_dark::STRONG) ]
    }
}

/// The label and count colours of a [`tab`].
pub fn tab_text(active: bool) -> (Color, Color) {
    if active {
        (ro_dark::BUTTON_LABEL, ro_dark::ACCENT)
    } else {
        (ro_dark::MUTED, ro_dark::MUTED)
    }
}

/// A square turned 45 degrees: favourite marks, card pips and plate studs.
pub fn diamond(size: f32, fill: Color, stroke: Color) -> impl Scene {
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    bsn! {
        Node { width: px(size), height: px(size), flex_shrink: 0.0, border: px(1) }
        tilt
        BackgroundColor(fill)
        BorderColor::all(stroke)
        @ignore_picking()
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

/// Runtime text in the title font; [`title_text`] only takes static strings.
pub fn display_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
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
pub fn button_with(art: ButtonArt, content: impl Scene) -> impl Scene {
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

/// A wrapping grid that shows `rows` rows of `cell`-high items, `gap` apart, and scrolls
/// beyond that, so a growing list never grows its window. The scrollbar hides while
/// everything fits. The negative margin keeps the first column on the container's edge
/// despite the padding.
pub fn scroll_grid<S: Scene>(items: Vec<S>, cell: f32, rows: f32, gap: f32) -> impl Scene {
    let max_height = rows * cell + (rows - 1.0) * gap + 2.0 * GRID_PAD;
    bsn! {
        Node {
            position_type: PositionType::Relative,
            margin: {UiRect::all(px(-GRID_PAD))},
        }
        Children [
            #grid
            ScrollArea
            Pickable
            Node {
                flex_grow: 1.0,
                max_height: px(max_height),
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                column_gap: px(gap),
                row_gap: px(gap),
                padding: {UiRect::new(
                    px(GRID_PAD),
                    px(GRID_PAD + SCROLL_GRID_GUTTER),
                    px(GRID_PAD),
                    px(GRID_PAD),
                )},
                overflow: {Overflow::scroll_y()},
            }
            Children [ {items} ]
            --
            @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(GRID_PAD),
                top: px(GRID_PAD),
                bottom: px(GRID_PAD),
                width: px(6),
            }
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

type HoverState<'a> = (
    &'a Hovered,
    &'a HoverColors,
    &'a mut BackgroundColor,
    &'a mut BorderColor,
);

/// Applies [`HoverColors`] as the pointer enters and leaves.
fn style_hover(mut controls: Query<HoverState, Changed<Hovered>>) {
    for (hovered, colors, mut fill, mut stroke) in &mut controls {
        let (background, border) = if hovered.get() {
            colors.hover
        } else {
            colors.rest
        };
        fill.0 = background;
        *stroke = BorderColor::all(border);
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

    #[test]
    fn hover_swaps_fill_and_stroke() {
        let mut app = App::new();
        app.add_systems(Update, style_hover);
        let colors = HoverColors {
            rest: (ro_dark::SLOT, ro_dark::SLOT_STROKE),
            hover: (ro_dark::SLOT_HOVER, ro_dark::BUTTON_STROKE),
        };
        let slot = app
            .world_mut()
            .spawn((
                Hovered(false),
                colors,
                BackgroundColor(ro_dark::SLOT),
                BorderColor::all(ro_dark::SLOT_STROKE),
            ))
            .id();
        app.update();

        app.world_mut().entity_mut(slot).insert(Hovered(true));
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(slot).unwrap().0,
            ro_dark::SLOT_HOVER
        );
        assert_eq!(
            *app.world().get::<BorderColor>(slot).unwrap(),
            BorderColor::all(ro_dark::BUTTON_STROKE)
        );
    }

    #[test]
    fn scroll_grid_holds_every_item_under_a_height_cap() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        let cells: Vec<_> = (0..5).map(|_| divider_diamond()).collect();
        app.world_mut()
            .commands()
            .spawn_scene(scroll_grid(cells, 10.0, 2.0, 4.0));
        app.update();

        let world = app.world_mut();
        let (node, children) = world
            .query_filtered::<(&Node, &Children), With<ScrollArea>>()
            .single(world)
            .unwrap();
        assert_eq!(node.max_height, px(2.0 * 10.0 + 4.0 + 2.0 * GRID_PAD));
        assert_eq!(node.overflow, Overflow::scroll_y());
        assert_eq!(children.len(), 5);
    }
}
