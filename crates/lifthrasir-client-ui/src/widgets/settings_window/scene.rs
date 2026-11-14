//! The Settings window scenes: the frame [`build`] spawns once, and the rail, heading,
//! rows and footer that [`rebuild`](super::rebuild) projects from their views. Sizes
//! follow the `Settings` artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{Button, ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_client::domain::input::PlayerAction;
use lifthrasir_client::domain::settings::AudioConfig;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, HoverColors, WINDOW_SHADOW, close_button, diamond, display_text, inset, segment,
    tab_text, window_title_bar, window_titled,
};

use super::view::{
    BindRow, ControlView, FootView, HeadView, KeyState, NavItem, Row, SettingRow, percent_label,
};
use super::{
    AudioChannel, BindSlot, Preset, Setting, SettingsAction, SettingsHost, SettingsSearchField,
    SettingsTitlebar, SettingsWindowRoot, SliderOf, SliderPart, SliderRail, on_settings_button,
    on_settings_click, on_slider_drag, on_slider_press,
};
use crate::screens::shared::{
    chip, cycle_box, glow, link, overline, row_button, search_field, usable, wide_button,
};

const WINDOW_LEFT: f32 = 120.0;
const WINDOW_TOP: f32 = 16.0;
const WINDOW_WIDTH: f32 = 984.0;
const NAV_WIDTH: f32 = 210.0;
/// Fixed height of the section pane: its rows scroll inside it, so the window never
/// grows with the open section.
const PANE_HEIGHT: f32 = 520.0;
/// Width of the column a row's control sits in.
const CONTROL_WIDTH: f32 = 310.0;
/// Width of one key cap column of the Controls section.
const KEY_CELL: f32 = 120.0;

/// Spawn the whole window as one top-level scene.
pub fn build(commands: &mut Commands) {
    commands.spawn_scene(window());
}

fn window() -> impl Scene {
    bsn! {
        SettingsWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        // Float above every other UI root (login panel, in-game HUD) so the
        // window owns picking: otherwise a full-screen screen root spawned
        // later in the stack swallows its clicks and drags.
        GlobalZIndex(1000)
        Pickable
        Children [
            SettingsTitlebar
            @window_title_bar("Settings", bsn! {
                @close_button()
                SettingsAction::Close
                on(on_settings_button)
            })
            Pickable
            on(drag_window::<SettingsTitlebar, SettingsWindowRoot>)
            --
            Node { column_gap: px(18), margin: {UiRect::top(px(10))} }
            @ignore_picking()
            Children [ @rail() -- @pane() ]
            --
            SettingsHost::Foot
            Node {
                align_items: AlignItems::Center,
                column_gap: px(12),
                margin: {UiRect::new(px(0), px(0), px(14), px(8))},
                position_type: PositionType::Relative,
            }
            @ignore_picking()
        ]
    }
}

/// The search box, which outlives every redraw so typing keeps its caret, over the
/// host of the section buttons.
fn rail() -> impl Scene {
    bsn! {
        Node {
            width: px(NAV_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
        }
        @ignore_picking()
        Children [
            Node {}
            @ignore_picking()
            Children [ @search_field::<SettingsSearchField>("Search settings") ]
            --
            SettingsHost::Nav
            Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
            @ignore_picking()
        ]
    }
}

/// The open section: its heading over the scrolling rows.
fn pane() -> impl Scene {
    bsn! {
        @inset()
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(PANE_HEIGHT),
            flex_direction: FlexDirection::Column,
        }
        @ignore_picking()
        Children [
            SettingsHost::Head
            Node {
                flex_shrink: 0.0,
                align_items: AlignItems::Baseline,
                column_gap: px(10),
                padding: {UiRect::new(px(18), px(18), px(14), px(10))},
                border: {UiRect::bottom(px(1))},
            }
            BorderColor::all(ro_dark::INSET_RULE)
            @ignore_picking()
            --
            Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #rows
                SettingsHost::Rows
                ScrollArea
                Pickable
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    flex_direction: FlexDirection::Column,
                    padding: {UiRect::new(px(18), px(22), px(4), px(14))},
                    overflow: {Overflow::scroll_y()},
                }
                --
                @FeathersScrollbar { @target: #rows, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    top: px(8),
                    bottom: px(8),
                    width: px(6),
                }
            ]
        ]
    }
}

/// Mounts each scene under `host`, in order.
fn mount(commands: &mut Commands, host: Entity, scenes: Vec<Box<dyn Scene>>) {
    for scene in scenes {
        commands.spawn_scene(scene).insert(ChildOf(host));
    }
}

pub(super) fn spawn_nav(commands: &mut Commands, host: Entity, items: Vec<NavItem>) {
    let scenes = items
        .into_iter()
        .map(|item| Box::new(nav_item(item)) as Box<dyn Scene>)
        .collect();
    mount(commands, host, scenes);
}

/// The dot beside what differs from the applied settings.
fn changed_dot() -> impl Scene {
    let halo = glow(ro_dark::FROST_GLOW.with_alpha(0.9));
    bsn! {
        Node { width: px(6), height: px(6), flex_shrink: 0.0, border_radius: BorderRadius::MAX }
        BackgroundColor(ro_dark::FROST_GLOW)
        halo
        @ignore_picking()
    }
}

fn nav_item(item: NavItem) -> impl Scene {
    let (label, _, icon) = item.tab.meta();
    let (rest, hover, ink) = if item.active {
        let lit = (ro_dark::TAB_ACTIVE, ro_dark::TAB_STROKE);
        (lit, lit, ro_dark::BUTTON_LABEL)
    } else {
        (
            (Color::NONE, Color::NONE),
            (Color::WHITE.with_alpha(0.04), Color::NONE),
            ro_dark::MUTED,
        )
    };
    let colors = HoverColors { rest, hover };
    let (fill, stroke) = rest;
    let action = SettingsAction::Section(item.tab);
    let hits = item
        .hits
        .map(|hits| EntityScene(chrome_text(hits.to_string(), 11.0, ro_dark::MUTED)));
    let dot = item.changed.then(|| EntityScene(changed_dot()));
    bsn! {
        action
        Hovered
        colors
        Node {
            height: px(38),
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(12))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(on_settings_click)
        Children [
            @glyph_icon(icon, 15.0, ink)
            --
            @chrome_text(label.into(), 13.0, ink)
            Node { flex_grow: 1.0 }
            --
            {hits}
            --
            {dot}
        ]
    }
}

pub(super) fn spawn_head(commands: &mut Commands, host: Entity, head: HeadView) {
    let reset: Box<dyn Scene> = if head.can_reset {
        Box::new(bsn! {
            @link("Reset this section")
            SettingsAction::ResetSection
            on(on_settings_click)
        })
    } else {
        Box::new(chrome_text(
            "Reset this section".into(),
            12.0,
            ro_dark::DISABLED_LABEL,
        ))
    };
    let scenes: Vec<Box<dyn Scene>> = vec![
        Box::new(display_text(head.title.into(), 18.0, ro_dark::HERO)),
        Box::new(bsn! {
            @chrome_text(head.subtitle.into(), 12.0, ro_dark::MUTED)
            Node { flex_grow: 1.0 }
        }),
        reset,
    ];
    mount(commands, host, scenes);
}

pub(super) fn spawn_rows(
    commands: &mut Commands,
    host: Entity,
    rows: Vec<Row>,
    audio: &AudioConfig,
) {
    let scenes = rows.into_iter().map(|row| row_scene(row, audio)).collect();
    mount(commands, host, scenes);
}

fn row_scene(row: Row, audio: &AudioConfig) -> Box<dyn Scene> {
    match row {
        Row::Presets(current) => Box::new(presets(current)),
        Row::Group(label) => Box::new(bsn! {
            @overline(label.into(), ro_dark::ACCENT)
            Node { margin: {UiRect::new(px(4), px(0), px(16), px(2))} }
        }),
        Row::Setting { row, lit } => Box::new(setting_row(row, lit, audio)),
        Row::BindHeader => Box::new(bind_header()),
        Row::Clash { key, other } => Box::new(clash_alert(key, other)),
        Row::Bind { row, lit } => Box::new(bind_row(row, lit)),
        Row::BindHint => Box::new(bsn! {
            @chrome_text(
                "Click a key, then press the new one \u{b7} Esc cancels \u{b7} Backspace clears".into(),
                11.0,
                ro_dark::MUTED,
            )
            Node { margin: {UiRect::top(px(10))} }
        }),
        Row::Empty(query) => Box::new(bsn! {
            Node { justify_content: JustifyContent::Center, margin: {UiRect::top(px(120))} }
            @ignore_picking()
            Children [
                @chrome_text(
                    format!("No settings match \u{201c}{query}\u{201d}"),
                    12.0,
                    ro_dark::MUTED,
                )
            ]
        }),
    }
}

/// A segment sharing its row evenly with the others.
fn segment_frame(label: &'static str, active: bool) -> impl Scene {
    let (ink, _) = tab_text(active);
    bsn! {
        @segment(active)
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(28),
            padding: {UiRect::all(px(0))},
            justify_content: JustifyContent::Center,
        }
        Children [ @chrome_text(label.into(), 12.0, ink) ]
    }
}

fn option_segment(label: &'static str, active: bool, action: SettingsAction) -> impl Scene {
    bsn! {
        @segment_frame(label, active)
        action
        on(on_settings_click)
    }
}

/// The quality presets. "Custom" lights up when the settings match none and does
/// nothing when clicked.
fn presets(current: Option<Preset>) -> impl Scene {
    let mut segments: Vec<Box<dyn Scene>> = Preset::ALL
        .into_iter()
        .map(|preset| {
            let action = SettingsAction::Preset(preset);
            Box::new(option_segment(
                preset.label(),
                current == Some(preset),
                action,
            )) as Box<dyn Scene>
        })
        .collect();
    segments.push(Box::new(segment_frame("Custom", current.is_none())));
    let note = if current.is_some() {
        "All quality settings match this preset"
    } else {
        "You\u{2019}ve tuned these yourself \u{b7} Pick a preset to start over"
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            padding: {UiRect::new(px(4), px(4), px(12), px(6))},
        }
        @ignore_picking()
        Children [
            @overline("QUALITY PRESET".into(), ro_dark::ACCENT)
            --
            Node { column_gap: px(4) }
            @ignore_picking()
            Children [ {segments} ]
            --
            @chrome_text(note.into(), 11.0, ro_dark::MUTED)
        ]
    }
}

/// The fill of a row the search found.
fn row_fill(lit: bool) -> Color {
    if lit {
        ro_dark::FROST_GLOW.with_alpha(0.05)
    } else {
        Color::NONE
    }
}

fn setting_row(row: SettingRow, lit: bool, audio: &AudioConfig) -> impl Scene {
    let dot = row.changed.then(|| EntityScene(changed_dot()));
    let tag = row.tag.map(|tag| {
        EntityScene(chip(
            tag.into(),
            ro_dark::STATUS_BUSY.with_alpha(0.16),
            ro_dark::STATUS_BUSY,
        ))
    });
    let control = control(row.setting, row.control, audio);
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(16),
            padding: {UiRect::axes(px(4), px(12))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor({row_fill(lit)})
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
            }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(8) }
                @ignore_picking()
                Children [
                    @chrome_text(row.label.into(), 13.0, ro_dark::STRONG)
                    --
                    {dot}
                    --
                    {tag}
                ]
                --
                @chrome_text(row.desc.into(), 11.0, ro_dark::MUTED)
            ]
            --
            Node {
                width: px(CONTROL_WIDTH),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
            }
            @ignore_picking()
            Children [ @control ]
        ]
    }
}

fn control(setting: Setting, control: ControlView, audio: &AudioConfig) -> Box<dyn Scene> {
    match control {
        ControlView::Cycle(label) => {
            let action = SettingsAction::Cycle(setting);
            Box::new(bsn! {
                @cycle_box(chrome_text(label, 13.0, ro_dark::BODY), CONTROL_WIDTH)
                action
                on(on_settings_click)
            })
        }
        ControlView::Segments { options, selected } => {
            let segments: Vec<_> = options
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let action = SettingsAction::Choose(setting, index);
                    option_segment(label, index == selected, action)
                })
                .collect();
            Box::new(bsn! {
                Node { width: percent(100), column_gap: px(4) }
                @ignore_picking()
                Children [ {segments} ]
            })
        }
        ControlView::Toggle(enabled) => Box::new(toggle(setting, enabled)),
        ControlView::Slider { channel, muted } => {
            let (volume, _) = channel.read(audio);
            Box::new(volume_control(channel, volume, muted))
        }
    }
}

/// A switch with its state in words. A click anywhere on it flips it.
fn toggle(setting: Setting, enabled: bool) -> impl Scene {
    let (fill, stroke, left, word) = if enabled {
        (ro_dark::PRIMARY, ro_dark::PRIMARY_STROKE, 20.0, "On")
    } else {
        (ro_dark::BAR_TROUGH, ro_dark::MUTED_BRONZE, 2.0, "Off")
    };
    let action = SettingsAction::Toggle(setting);
    bsn! {
        action
        Node { align_items: AlignItems::Center, column_gap: px(10) }
        Pickable
        on(on_settings_click)
        Children [
            @chrome_text(word.into(), 12.0, ro_dark::MUTED)
            --
            Node {
                width: px(40),
                height: px(22),
                border: px(1),
                border_radius: BorderRadius::MAX,
                position_type: PositionType::Relative,
            }
            BackgroundColor(fill)
            BorderColor::all(stroke)
            @ignore_picking()
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(2),
                    width: px(16),
                    height: px(16),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor(ro_dark::BRONZE_LIGHT)
                @ignore_picking()
            ]
        ]
    }
}

/// A channel's mute button, slider and readout.
fn volume_control(channel: AudioChannel, volume: f32, muted: bool) -> impl Scene {
    let (rest, icon, icon_ink, readout_ink) = if muted {
        (
            (ro_dark::DANGER_PRESSED, ro_dark::BRONZE),
            "ban",
            ro_dark::CLOSE_X,
            ro_dark::TONE_ERROR,
        )
    } else {
        (
            (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE),
            "mega",
            ro_dark::MUTED,
            ro_dark::STRONG,
        )
    };
    let (fill, stroke) = rest;
    let colors = HoverColors {
        rest,
        hover: (fill, ro_dark::BUTTON_STROKE),
    };
    let action = SettingsAction::Mute(channel);
    bsn! {
        Node { width: percent(100), align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            action
            Button
            Hovered
            colors
            Node {
                width: px(28),
                height: px(28),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
                border_radius: BorderRadius::all(px(5)),
            }
            BackgroundColor(fill)
            BorderColor::all(stroke)
            Pickable
            on(on_settings_button)
            Children [ @glyph_icon(icon, 14.0, icon_ink) ]
            --
            @slider(channel, volume, muted)
            --
            Node { width: px(48), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [
                SliderPart::Readout
                SliderOf(channel)
                @chrome_text(percent_label(volume, muted), 12.0, readout_ink)
            ]
        ]
    }
}

/// The volume slider: a press or a drag anywhere on it sets the volume from the
/// pointer. [`sync_sliders`](super::sync_sliders) moves the fill and the knob. A muted
/// channel's slider is dimmed and inert.
fn slider(channel: AudioChannel, volume: f32, muted: bool) -> impl Scene {
    let at = Val::Percent(if muted { 0.0 } else { volume * 100.0 });
    let strength = if muted { 0.4 } else { 1.0 };
    bsn! {
        SliderRail(channel)
        RelativeCursorPosition
        Node {
            flex_grow: 1.0,
            min_width: px(60),
            height: px(16),
            align_items: AlignItems::Center,
            position_type: PositionType::Relative,
        }
        Pickable { should_block_lower: {!muted}, is_hoverable: {!muted} }
        on(on_slider_press)
        on(on_slider_drag)
        Children [
            Node {
                width: percent(100),
                height: px(6),
                border: px(1),
                border_radius: BorderRadius::all(px(3)),
            }
            BackgroundColor(ro_dark::BAR_TROUGH)
            BorderColor::all(ro_dark::INSET_RULE)
            @ignore_picking()
            Children [
                SliderPart::Fill
                SliderOf(channel)
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: {at},
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor({ro_dark::SP.with_alpha(strength)})
                @ignore_picking()
            ]
            --
            SliderPart::Knob
            SliderOf(channel)
            @diamond(11.0, ro_dark::FROST.with_alpha(strength), ro_dark::BRONZE_SHADOW)
            Node {
                position_type: PositionType::Absolute,
                left: {at},
                top: px(2.5),
                margin: {UiRect::left(px(-5.5))},
            }
        ]
    }
}

fn key_heading(text: &'static str) -> impl Scene {
    bsn! {
        Node { width: px(KEY_CELL), justify_content: JustifyContent::Center }
        @ignore_picking()
        Children [ @overline(text.into(), ro_dark::ACCENT) ]
    }
}

fn bind_header() -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(16),
            padding: {UiRect::axes(px(4), px(8))},
            margin: {UiRect::top(px(8))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @overline("ACTION".into(), ro_dark::ACCENT)
            Node { flex_grow: 1.0 }
            --
            @key_heading("PRIMARY")
            --
            @key_heading("SECONDARY")
        ]
    }
}

/// The alert raised when a pressed key is already bound: keep the old bind or trade.
fn clash_alert(key: String, other: String) -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(10),
            margin: {UiRect::vertical(px(8))},
            padding: {UiRect::axes(px(12), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({ro_dark::STATUS_BUSY.with_alpha(0.1)})
        BorderColor::all(ro_dark::STATUS_BUSY.with_alpha(0.5))
        @ignore_picking()
        Children [
            @chrome_text(format!("{key} is already {other}."), 12.0, ro_dark::BRONZE_LIGHT)
            Node { flex_grow: 1.0 }
            --
            @row_button(ButtonArt::NORMAL, "Keep Old", 72.0)
            SettingsAction::KeepBind
            on(on_settings_button)
            --
            @row_button(ButtonArt::PRIMARY, "Swap Them", 84.0)
            SettingsAction::SwapBinds
            on(on_settings_button)
        ]
    }
}

fn bind_row(row: BindRow, lit: bool) -> impl Scene {
    let dot = row.changed.then(|| EntityScene(changed_dot()));
    let [primary, secondary] = row.keys;
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(16),
            padding: {UiRect::axes(px(4), px(8))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor({row_fill(lit)})
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node { flex_grow: 1.0, align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [
                @chrome_text(row.label, 13.0, ro_dark::STRONG)
                --
                {dot}
            ]
            --
            @key_cap(row.action, BindSlot::Primary, primary)
            --
            @key_cap(row.action, BindSlot::Secondary, secondary)
        ]
    }
}

/// A slot's key cap, centred in its column. A click starts listening for its key.
fn key_cap(action: PlayerAction, slot: BindSlot, (text, state): (String, KeyState)) -> impl Scene {
    let (stroke, ink) = match state {
        KeyState::Bound => (ro_dark::SLOT_STROKE, ro_dark::BODY),
        KeyState::Empty => (ro_dark::SLOT_STROKE, ro_dark::FAINT),
        KeyState::Listening => (ro_dark::FROST_GLOW, ro_dark::CHECK_MARK),
        KeyState::Clash => (ro_dark::STATUS_BUSY, ro_dark::BRONZE_LIGHT),
    };
    let settled = matches!(state, KeyState::Bound | KeyState::Empty);
    let hover_stroke = if settled {
        ro_dark::BUTTON_STROKE
    } else {
        stroke
    };
    let colors = HoverColors {
        rest: (ro_dark::SLOT, stroke),
        hover: (ro_dark::SLOT_HOVER, hover_stroke),
    };
    let halo = glow(if state == KeyState::Listening {
        ro_dark::FROST_GLOW.with_alpha(0.5)
    } else {
        Color::NONE
    });
    let listen = SettingsAction::Listen(action, slot);
    bsn! {
        Node { width: px(KEY_CELL), flex_shrink: 0.0, justify_content: JustifyContent::Center }
        @ignore_picking()
        Children [
            listen
            Hovered
            colors
            Node {
                min_width: px(110),
                height: px(28),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: {UiRect::horizontal(px(10))},
                border: px(1),
                border_radius: BorderRadius::all(px(5)),
            }
            BackgroundColor(ro_dark::SLOT)
            BorderColor::all(stroke)
            halo
            Pickable
            on(on_settings_click)
            Children [ @chrome_text(text, 12.0, ink) ]
        ]
    }
}

pub(super) fn spawn_foot(commands: &mut Commands, host: Entity, foot: FootView) {
    let dirty = foot.pending > 0;
    let pending_ink = if dirty {
        ro_dark::FROST_GLOW
    } else {
        ro_dark::MUTED
    };
    let mut scenes: Vec<Box<dyn Scene>> = vec![
        Box::new(bsn! {
            @wide_button(ButtonArt::NORMAL, "Reset All to Defaults", 176.0)
            SettingsAction::ResetAll
            on(on_settings_button)
        }),
        Box::new(bsn! {
            Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [ @chrome_text(foot.pending_label(), 12.0, pending_ink) ]
        }),
        Box::new(bsn! {
            @wide_button(ButtonArt::NORMAL, "Cancel", 80.0)
            @usable(dirty, Color::WHITE)
            SettingsAction::Cancel
            on(on_settings_button)
        }),
        Box::new(bsn! {
            @wide_button(ButtonArt::PRIMARY, "Apply", 110.0)
            @usable(dirty, Color::WHITE)
            SettingsAction::Apply
            on(on_settings_button)
        }),
    ];
    if let Some(text) = foot.toast {
        scenes.push(Box::new(toast(text)));
    }
    mount(commands, host, scenes);
}

/// A short notice over the footer.
fn toast(text: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(44),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            Node {
                padding: {UiRect::axes(px(12), px(6))},
                border: px(1),
                border_radius: BorderRadius::all(px(6)),
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::BUTTON_STROKE)
            @ignore_picking()
            Children [ @chrome_text(text, 12.0, ro_dark::BODY) ]
        ]
    }
}
