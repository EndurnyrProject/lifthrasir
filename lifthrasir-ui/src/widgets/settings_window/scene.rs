//! Idiomatic BSN chrome for the settings window (mirrors the inventory/shop
//! windows). [`build`] spawns the whole window — root, titlebar, tab rail, a
//! fixed-height scrollable content pane holding the three tab bodies, and the
//! footer — as one `bsn!` tree. The tree is static: the `refresh_*` systems in
//! [`super`] project the live `SettingsUi` draft onto the controls via their
//! marker components, so nothing here is rebuilt at runtime (rebuilding would
//! despawn the slider rails mid-drag).
//!
//! The window is fixed-size: the content pane has a fixed [`PANE_HEIGHT`] and
//! scrolls internally (`ScrollArea` + `FeathersScrollbar`) instead of growing
//! with the active tab — the Input tab's seventeen bind rows scroll.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{Activate, ControlOrientation, ScrollArea};
use bevy_feathers::controls::{FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::input::{PlayerAction, HOTBAR_ACTIONS};
use game_engine::domain::settings::DisplayMode;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{
    on_apply, on_cancel, on_keycap_click, on_mute_click, on_reset, on_segment_click,
    on_slider_drag, on_slider_press, on_stepper_click, on_switch_click, on_tab_click, ApplyButton,
    AudioChannel, BindSlot, DirtyDot, GraphicsField, Keycap, MuteButton, SegButton, SettingsTab,
    SettingsTitlebar, SettingsWindowRoot, SliderFill, SliderKnob, SliderPercent, SliderRail,
    StepDir, StepperArrow, StepperValue, SwitchKnob, SwitchPill, TabBody, TabButton,
};

const WINDOW_LEFT: f32 = 360.0;
const WINDOW_TOP: f32 = 120.0;
const WINDOW_WIDTH: f32 = 560.0;
const RAIL_WIDTH: f32 = 140.0;
/// Fixed height of the rail + content row, so the window never grows with the
/// active tab — the content pane scrolls internally instead.
const PANE_HEIGHT: f32 = 340.0;

/// The rebindable non-hotbar actions in display order. The twelve hotbar slots
/// follow these rows, labelled `Hotbar F1`..`Hotbar F12`.
const ACTIONS: [(PlayerAction, &str); 5] = [
    (PlayerAction::Sit, "Sit / Stand"),
    (PlayerAction::Status, "Status Window"),
    (PlayerAction::Inventory, "Inventory"),
    (PlayerAction::Skills, "Skills Window"),
    (PlayerAction::Equipment, "Equipment"),
];

/// Spawn the whole window as one top-level scene.
pub fn build(commands: &mut Commands) {
    commands.spawn_scene(window());
}

fn window() -> impl Scene {
    bsn! {
        SettingsWindowRoot
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
        // Float above every other UI root (login panel, in-game HUD) so the
        // window owns picking — otherwise a full-screen screen root spawned
        // later in the stack swallows its clicks and drags.
        GlobalZIndex(1000)
        Pickable
        Children [ titlebar(), main_row(), footer() ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        SettingsTitlebar
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
            glyph_icon("gear", 16.0, theme::GOLD),
            (
                Text("System Settings")
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

/// The tab rail (left) and the scrollable content pane (right).
fn main_row() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, height: px(PANE_HEIGHT) }
        ignore_picking()
        Children [ rail(), content_pane() ]
    }
}

fn rail() -> impl Scene {
    bsn! {
        Node {
            width: px(RAIL_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::all(px(14))},
            border: {UiRect { right: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            tab_button(SettingsTab::Graphics, "Graphics"),
            tab_button(SettingsTab::Sound, "Sound"),
            tab_button(SettingsTab::Input, "Input"),
        ]
    }
}

fn tab_button(tab: SettingsTab, label: &'static str) -> impl Scene {
    bsn! {
        template_value(TabButton(tab))
        Node {
            height: px(32),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(10))},
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        Pickable
        on(on_tab_click)
        Children [ chrome_text(label.to_string(), 13.0, theme::TEXT_DIM) ]
    }
}

/// The fixed-height content pane: a wheel-scrollable viewport holding the three
/// tab bodies with a draggable [`FeathersScrollbar`] pinned to the right. The
/// `#pane` id wires the scrollbar to the viewport whose `ScrollPosition` it
/// drives.
fn content_pane() -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            position_type: PositionType::Relative,
        }
        ignore_picking()
        Children [
            (
                #pane
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    padding: {UiRect { left: Val::Px(14.0), right: Val::Px(18.0), top: Val::Px(10.0), bottom: Val::Px(14.0) }},
                }
                ScrollArea
                Pickable
                Children [ graphics_body(), sound_body(), input_body() ]
            ),
            @FeathersScrollbar { @target: #pane, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(3),
                top: px(4),
                bottom: px(4),
                width: px(6),
            }
        ]
    }
}

// ---------------------------------------------------------------------------
// Tab bodies. Only the active tab's body is displayed: `refresh_tabs` toggles
// `display` (not `Visibility`) so the inactive bodies release their layout slot
// and the active tab always sits at the top of the pane.
// ---------------------------------------------------------------------------

fn graphics_body() -> impl Scene {
    let dlss = cfg!(feature = "dlss").then(|| {
        EntityScene(row(
            "DLSS",
            "NVIDIA render-resolution upscaling (RTX only)",
            stepper(GraphicsField::Dlss),
        ))
    });
    bsn! {
        template_value(TabBody(SettingsTab::Graphics))
        Node { flex_direction: FlexDirection::Column, row_gap: px(10), flex_shrink: 0.0 }
        ignore_picking()
        Children [
            chrome_text("GRAPHICS".to_string(), 11.0, theme::GOLD),
            section("Display"),
            row("Display Mode", "How the game fills your screen", segmented()),
            row("Resolution", "Screen size in pixels", stepper(GraphicsField::Resolution)),
            section("Quality"),
            row("Antialiasing", "Smooths jagged edges", stepper(GraphicsField::Antialiasing)),
            row("Anisotropic Filtering", "Sharpens ground textures at grazing angles", stepper(GraphicsField::Anisotropy)),
            row("Upscaling", "xBRZ sprite & texture upscaling (applies on map reload)", stepper(GraphicsField::Upscaling)),
            {dlss},
            row("Ambient Occlusion", "Contact shadows in crevices (SSAO); forces MSAA off", stepper(GraphicsField::Ssao)),
            row("Bloom", "Glow around bright lights", switch(GraphicsField::Bloom)),
            row("Shadows", "Sun shadow casting", switch(GraphicsField::Shadows)),
            row("VSync", "Sync frames to display refresh", switch(GraphicsField::Vsync)),
            row("Frame Rate Cap", "Maximum frames per second", stepper(GraphicsField::FpsCap)),
            section("Interface"),
            row("UI Scaling", "Scales the interface for high resolutions", stepper(GraphicsField::UiScaling)),
        ]
    }
}

fn sound_body() -> impl Scene {
    bsn! {
        template_value(TabBody(SettingsTab::Sound))
        Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(10), flex_shrink: 0.0 }
        ignore_picking()
        Children [
            chrome_text("SOUND".to_string(), 11.0, theme::GOLD),
            section("Volume Mix"),
            row("Background Music", "Ambient score & themes", sound_control(AudioChannel::Bgm)),
            row("Sound Effects", "Hits, skills & impacts", sound_control(AudioChannel::Sfx)),
            row("Ambient", "World, weather & footsteps", sound_control(AudioChannel::Ambient)),
        ]
    }
}

fn input_body() -> impl Scene {
    let rows: Vec<_> = ACTIONS
        .into_iter()
        .map(|(action, label)| bind_row(action, label.to_string()))
        .chain(
            HOTBAR_ACTIONS
                .into_iter()
                .enumerate()
                .map(|(i, action)| bind_row(action, format!("Hotbar F{}", i + 1))),
        )
        .collect();
    bsn! {
        template_value(TabBody(SettingsTab::Input))
        Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(10), flex_shrink: 0.0 }
        ignore_picking()
        Children [
            chrome_text("INPUT".to_string(), 11.0, theme::GOLD),
            section("Key Bindings"),
            bind_header(),
            {rows},
        ]
    }
}

// ---------------------------------------------------------------------------
// Shared row scaffolding.
// ---------------------------------------------------------------------------

/// A gold uppercase section caption with a trailing hairline.
fn section(text: &'static str) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
            margin: {UiRect::top(px(4))},
        }
        ignore_picking()
        Children [
            chrome_text(text.to_string(), 10.0, theme::GOLD),
            (
                Node { flex_grow: 1.0, height: px(1) }
                BackgroundColor(theme::GOLD_FAINT)
                ignore_picking()
            ),
        ]
    }
}

/// A setting row: a label column (title + sublabel) and a right-aligned control.
fn row(label: &'static str, sublabel: &'static str, control: impl Scene) -> impl Scene {
    let control = EntityScene(control);
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(18),
            min_height: px(46),
        }
        ignore_picking()
        Children [
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(3) }
                ignore_picking()
                Children [
                    chrome_text(label.to_string(), 13.0, theme::TEXT),
                    chrome_text(sublabel.to_string(), 11.0, theme::TEXT_FAINT),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center }
                ignore_picking()
                Children [ {control} ]
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Graphics controls.
// ---------------------------------------------------------------------------

/// Segmented control for `DisplayMode`: one button per variant, active one
/// highlighted by `refresh_graphics`.
fn segmented() -> impl Scene {
    let buttons: Vec<_> = DisplayMode::ALL
        .into_iter()
        .enumerate()
        .map(|(index, mode)| segment_button(index, mode.label()))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(4),
            padding: {UiRect::all(px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ {buttons} ]
    }
}

fn segment_button(index: usize, label: &'static str) -> impl Scene {
    bsn! {
        template_value(SegButton { field: GraphicsField::DisplayMode, index })
        Node {
            height: px(30),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: {UiRect::horizontal(px(13))},
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({Color::NONE})
        Pickable
        on(on_segment_click)
        Children [ chrome_text(label.to_string(), 12.0, theme::TEXT_DIM) ]
    }
}

/// Stepper control: ◀ value ▶ over a field's presets.
fn stepper(field: GraphicsField) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            min_width: px(188),
            height: px(38),
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            stepper_arrow(field, StepDir::Prev),
            stepper_value(field),
            stepper_arrow(field, StepDir::Next),
        ]
    }
}

fn stepper_arrow(field: GraphicsField, dir: StepDir) -> impl Scene {
    let glyph = if dir == StepDir::Prev { "<" } else { ">" };
    bsn! {
        template_value(StepperArrow { field, dir })
        Node {
            width: px(38),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Pickable
        on(on_stepper_click)
        Children [ chrome_text(glyph.to_string(), 14.0, theme::TEXT_DIM) ]
    }
}

/// The stepper's value text; `refresh_graphics` rewrites it.
fn stepper_value(field: GraphicsField) -> impl Scene {
    bsn! {
        template_value(StepperValue(field))
        Text("")
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(theme::TEXT)
        template_value(TextLayout { justify: Justify::Center, ..Default::default() })
        Node { flex_grow: 1.0, justify_content: JustifyContent::Center }
        ignore_picking()
    }
}

/// Toggle switch: a pill with a sliding knob, both restyled by `refresh_graphics`.
fn switch(field: GraphicsField) -> impl Scene {
    bsn! {
        template_value(SwitchPill(field))
        Node {
            width: px(50),
            height: px(28),
            border: px(1),
            border_radius: BorderRadius::all(px(16)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        Pickable
        on(on_switch_click)
        Children [
            (
                template_value(SwitchKnob(field))
                Node {
                    position_type: PositionType::Absolute,
                    top: px(3),
                    left: px(3),
                    width: px(20),
                    height: px(20),
                    border_radius: BorderRadius::all(px(10)),
                }
                BackgroundColor(theme::TEXT_DIM)
                ignore_picking()
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Sound controls.
// ---------------------------------------------------------------------------

/// A channel's control cluster: mute button, slider rail, percent readout.
fn sound_control(channel: AudioChannel) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center }
        ignore_picking()
        Children [ mute_button(channel), slider(channel), percent_readout(channel) ]
    }
}

/// A small square button that flips the channel's mute. `refresh_sound` tints it.
fn mute_button(channel: AudioChannel) -> impl Scene {
    bsn! {
        template_value(MuteButton(channel))
        Node {
            width: px(30),
            height: px(30),
            margin: {UiRect::right(px(12))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        Pickable
        on(on_mute_click)
        Children [ chrome_text("M".to_string(), 11.0, theme::TEXT_FAINT) ]
    }
}

/// A volume slider: a rail (track + fill + knob). Click and drag on the rail map
/// the cursor's `RelativeCursorPosition` to the volume.
fn slider(channel: AudioChannel) -> impl Scene {
    bsn! {
        template_value(SliderRail(channel))
        Node { width: px(200), height: px(22), align_items: AlignItems::Center }
        RelativeCursorPosition
        Pickable
        on(on_slider_press)
        on(on_slider_drag)
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(6),
                    border: px(1),
                    border_radius: BorderRadius::all(px(4)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(theme::STROKE)
                ignore_picking()
                Children [
                    (
                        template_value(SliderFill(channel))
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(0),
                            bottom: px(0),
                            width: percent(0),
                            border_radius: BorderRadius::all(px(4)),
                        }
                        BackgroundColor(theme::EMERALD)
                        ignore_picking()
                    ),
                ]
            ),
            (
                template_value(SliderKnob(channel))
                Node {
                    position_type: PositionType::Absolute,
                    top: px(3),
                    left: percent(0),
                    margin: {UiRect::left(px(-7))},
                    width: px(15),
                    height: px(15),
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                BackgroundColor(theme::DISPLAY_GOLD)
                BorderColor::all(theme::EMERALD_DEEP)
                ignore_picking()
            ),
        ]
    }
}

/// The percent (or "Muted") readout; `refresh_sound` rewrites it.
fn percent_readout(channel: AudioChannel) -> impl Scene {
    bsn! {
        template_value(SliderPercent(channel))
        Text("")
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(theme::TEXT_DIM)
        template_value(TextLayout { justify: Justify::Right, ..Default::default() })
        Node { width: px(44), margin: {UiRect::left(px(14))} }
        ignore_picking()
    }
}

// ---------------------------------------------------------------------------
// Input controls.
// ---------------------------------------------------------------------------

/// The "Action / Primary / Secondary" column header.
fn bind_header() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center }
        ignore_picking()
        Children [
            (
                Node { flex_grow: 1.0 }
                ignore_picking()
                Children [ chrome_text("Action".to_string(), 10.0, theme::TEXT_FAINT) ]
            ),
            header_cell("Primary"),
            header_cell("Secondary"),
        ]
    }
}

fn header_cell(text: &'static str) -> impl Scene {
    bsn! {
        Node { width: px(112), margin: {UiRect::left(px(8))} }
        ignore_picking()
        Children [ chrome_text(text.to_string(), 10.0, theme::TEXT_FAINT) ]
    }
}

/// One action row: action name + Primary and Secondary keycaps.
fn bind_row(action: PlayerAction, label: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            min_height: px(38),
        }
        ignore_picking()
        Children [
            (
                Node { flex_grow: 1.0 }
                ignore_picking()
                Children [ chrome_text(label, 13.0, theme::TEXT) ]
            ),
            keycap(action, BindSlot::Primary),
            keycap(action, BindSlot::Secondary),
        ]
    }
}

/// A clickable keycap cell. `refresh_input` rewrites its label; clicking it
/// starts a rebind capture for this `(action, slot)`.
fn keycap(action: PlayerAction, slot: BindSlot) -> impl Scene {
    bsn! {
        template_value(Keycap { action, slot })
        Node {
            width: px(104),
            height: px(30),
            margin: {UiRect::left(px(8))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        Pickable
        on(on_keycap_click)
        Children [ chrome_text(String::new(), 12.0, theme::TEXT_DIM) ]
    }
}

// ---------------------------------------------------------------------------
// Footer.
// ---------------------------------------------------------------------------

/// Footer: Reset to Defaults · unsaved-changes dot · Cancel · Apply. Apply keeps
/// a raw `BackgroundColor` (not a Feathers button) because `refresh_footer` dims
/// its alpha when the draft is clean.
fn footer() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::all(px(14))},
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            (
                @FeathersButton { @caption: bsn! { chrome_text("Reset to Defaults".to_string(), 13.0, theme::TEXT_DIM) } }
                Node { height: px(32) }
                on(on_reset)
            ),
            // Pushes the dirty dot + Cancel/Apply to the right edge.
            ( Node { flex_grow: 1.0 } ignore_picking() ),
            (
                DirtyDot
                Node { width: px(8), height: px(8), border_radius: BorderRadius::all(px(4)) }
                BackgroundColor(theme::WARN)
                Visibility::Hidden
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { chrome_text("Cancel".to_string(), 13.0, theme::TEXT_DIM) } }
                Node { height: px(32) }
                on(on_cancel)
            ),
            (
                ApplyButton
                Node {
                    height: px(32),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: {UiRect::horizontal(px(14))},
                    border_radius: BorderRadius::all(px(7)),
                }
                BackgroundColor(theme::EMERALD)
                Pickable
                on(on_apply)
                Children [ chrome_text("Apply".to_string(), 13.0, theme::EMERALD_INK) ]
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (mirror the inventory/shop scenes).
// ---------------------------------------------------------------------------

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

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<SettingsWindowRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Drag the settings window by its titlebar; the root is resolved from its
/// marker so the whole window spawns as one scene with no imperative drag
/// wiring. Only the titlebar itself moves the window: `Pointer<Drag>` bubbles up
/// from the close button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<SettingsTitlebar>>,
    mut roots: Query<&mut Node, With<SettingsWindowRoot>>,
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
