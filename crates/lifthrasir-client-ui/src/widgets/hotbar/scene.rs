//! How the hotbar is drawn: the plate with its slot cells, what a slot shows, and the
//! vitals, buttons, tooltip, picker and toast `sync_panel` hangs on the plate.

use std::f32::consts::TAU;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{Button, ScrollArea};
use lifthrasir_client::domain::hotbar::{HotbarSlot, SLOTS};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{HoverColors, segment, tab_text};

use super::view::{Choice, Filled, PickTab, SHORT_INK, SlotView, TipView, Tone};
use super::{
    Backdrop, CastFill, Drawn, HOTBAR_BOTTOM, HotbarAction, HotbarSlotUi, Region, SecondRow,
    SlotBody, Sweep, on_action, on_slot_click, on_slot_drag_drop, on_slot_drag_enter,
    on_slot_drag_leave, on_slot_drag_start, on_slot_out, on_slot_over,
};
use crate::screens::shared::{
    badge, glow, hud_plate, link, overline, plate_button, plate_sheen, thousands, tooltip_frame,
};
use crate::widgets::chat_box::CHAT_RIGHT;
use crate::widgets::hud_layout::{HudWidget, movable};

const SLOT: f32 = 44.0;
const ICON: f32 = 30.0;
const SLOT_GAP: f32 = 4.0;
/// Gap between the groups of four a row is split into.
const GROUP_GAP: f32 = 10.0;
const ROW_GAP: f32 = 6.0;
const PAD_LEFT: f32 = 12.0;
const PAD_Y: f32 = 8.0;
const VITALS_WIDTH: f32 = 58.0;
/// Gap between the vitals, the rows and the buttons.
const COLUMN_GAP: f32 = 12.0;
/// Room kept between the chat and the plate.
const CHAT_GAP: f32 = 12.0;
/// The plate with both rows on show, border included.
pub(super) const PLATE_HEIGHT: f32 = 2.0 * (SLOT + PAD_Y + 1.0) + ROW_GAP;
/// Wash over an icon that is cooling down.
const SHADE: Color = Color::srgba(0.016, 0.024, 0.035, 0.74);
/// Ink of a skill's level in its slot's corner.
const LEVEL_INK: Color = Color::srgb_u8(0xa9, 0xc9, 0x9a);

/// The hotbar's layer of the HUD: the plate at the bottom centre, pushed right where
/// it would cover the chat, over the backdrop that closes the picker.
pub(super) fn root() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::FlexEnd,
            padding: {UiRect::bottom(px(HOTBAR_BOTTOM))},
        }
        @ignore_picking()
        Children [
            Backdrop
            Button
            HotbarAction::ClosePicker
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                display: Display::None,
            }
            Pickable
            on(on_action)
            --
            @spacer(CHAT_RIGHT + CHAT_GAP)
            --
            @plate()
            --
            @spacer(0.0)
        ]
    }
}

/// The room on one side of the plate. Two equal ones centre it, and `min` keeps the
/// left one from shrinking under the chat.
fn spacer(min: f32) -> impl Scene {
    bsn! {
        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(min) }
        @ignore_picking()
    }
}

fn plate() -> impl Scene {
    let rows: Vec<_> = (0..2).map(row).collect();
    bsn! {
        @hud_plate()
        @movable(HudWidget::Hotbar)
        Node {
            flex_shrink: 0.0,
            align_items: AlignItems::FlexEnd,
            column_gap: px(COLUMN_GAP),
            padding: {UiRect::new(px(PAD_LEFT), px(10), px(PAD_Y), px(PAD_Y))},
        }
        Children [
            @plate_sheen()
            --
            Region::Vitals
            Node {
                width: px(VITALS_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: {UiRect::bottom(px(2))},
            }
            @ignore_picking()
            --
            Node { flex_direction: FlexDirection::Column, row_gap: px(ROW_GAP) }
            @ignore_picking()
            Children [ {rows} ]
            --
            Region::Buttons
            Node { flex_shrink: 0.0 }
            @ignore_picking()
            --
            @overlay(Region::Tip)
            --
            @overlay(Region::Picker)
            --
            @overlay(Region::Toast)
        ]
    }
}

/// A region covering the plate, for what floats over it.
fn overlay(region: Region) -> impl Scene {
    bsn! {
        region
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
        }
        @ignore_picking()
    }
}

/// Twelve cells in groups of four. The second row starts hidden.
fn row(row: usize) -> impl Scene {
    let cells: Vec<_> = (0..SLOTS).map(|col| cell(row, col)).collect();
    let second = (row == 1).then_some(bsn! { SecondRow Node { display: Display::None } });
    bsn! {
        Node { align_items: AlignItems::Center }
        @ignore_picking()
        @{second}
        Children [ {cells} ]
    }
}

/// A slot's cell: the outline and the pointer handling. `sync_slots` draws the rest.
fn cell(row: usize, col: usize) -> impl Scene {
    let at = HotbarSlotUi { row, col };
    let gap = match col {
        0 => 0.0,
        col if col % 4 == 0 => GROUP_GAP,
        _ => SLOT_GAP,
    };
    let halo = glow(Color::NONE);
    bsn! {
        at
        Drawn
        UiTransform
        Node {
            width: px(SLOT),
            height: px(SLOT),
            flex_shrink: 0.0,
            margin: {UiRect::left(px(gap))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
            position_type: PositionType::Relative,
        }
        BorderColor::all(ro_dark::SLOT_STROKE)
        halo
        Pickable
        on(on_slot_click)
        on(on_slot_over)
        on(on_slot_out)
        on(on_slot_drag_start)
        on(on_slot_drag_enter)
        on(on_slot_drag_leave)
        on(on_slot_drag_drop)
    }
}

/// Where a popup hangs over the cell at `col` of `row`: its centre from the plate's
/// left edge and its foot above the plate's bottom edge. `rows` is how many are shown.
pub(super) fn anchor(row: usize, col: usize, rows: usize) -> Vec2 {
    let groups = (col / 4) as f32;
    let x = PAD_LEFT
        + VITALS_WIDTH
        + COLUMN_GAP
        + col as f32 * (SLOT + SLOT_GAP)
        + groups * (GROUP_GAP - SLOT_GAP)
        + SLOT / 2.0;
    let rows_below = rows.saturating_sub(row + 1) as f32;
    Vec2::new(x, PAD_Y + rows_below * (SLOT + ROW_GAP) + SLOT + 8.0)
}

/// Fill of a slot: a plain wash when empty, else a glow in the tint of its kind.
fn backdrop(filled: Option<&Filled>) -> (Color, BackgroundGradient) {
    let Some(filled) = filled else {
        return (Color::BLACK.with_alpha(0.28), BackgroundGradient::default());
    };
    let (centre, rim) = if filled.is_skill() {
        (
            Color::srgb_u8(0x2a, 0x3a, 0x33),
            Color::srgb_u8(0x17, 0x1c, 0x1f),
        )
    } else {
        (
            Color::srgb_u8(0x3a, 0x32, 0x24),
            Color::srgb_u8(0x1c, 0x1a, 0x17),
        )
    };
    let glow = RadialGradient {
        shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
        stops: vec![
            ColorStop::new(centre, percent(0)),
            ColorStop::new(rim, percent(75)),
        ],
        ..default()
    };
    (rim, BackgroundGradient::from(glow))
}

/// What a slot shows inside its cell.
pub(super) fn body(view: SlotView) -> impl Scene {
    let (fill, glow) = backdrop(view.filled.as_ref());
    let marks = marks(&view);
    bsn! {
        SlotBody
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(fill)
        glow
        @ignore_picking()
        Children [ {marks} ]
    }
}

/// The key in a slot's top-left corner.
fn key_mark(key: String) -> impl Scene {
    bsn! {
        @badge(key, 9.0, ro_dark::LABEL)
        Node { left: px(3), top: px(2) }
    }
}

fn marks(view: &SlotView) -> Vec<Box<dyn Scene>> {
    let mut marks: Vec<Box<dyn Scene>> = Vec::new();
    let Some(filled) = &view.filled else {
        let faint = ro_dark::MUTED.with_alpha(0.35);
        marks.push(Box::new(glyph_icon("plus", 10.0, faint)));
        marks.push(Box::new(key_mark(view.key.clone())));
        return marks;
    };
    if let Some(path) = filled.icon.clone() {
        let alpha = match (filled.spent(), filled.cooldown.is_some()) {
            (true, _) => 0.35,
            (false, true) => 0.55,
            (false, false) => 1.0,
        };
        marks.push(Box::new(bsn! {
            ImageNode { image: {path}, color: {Color::WHITE.with_alpha(alpha)} }
            Node { width: px(ICON), height: px(ICON) }
            @ignore_picking()
        }));
    }
    if let (Some(left), HotbarSlot::Skill(id)) = (filled.cooldown.clone(), filled.slot) {
        marks.push(Box::new(cooldown(id, left)));
    }
    if filled.casting {
        marks.push(Box::new(cast_bar()));
    }
    marks.push(Box::new(key_mark(view.key.clone())));
    if let Some(count) = filled.count {
        let (text, ink) = match count {
            0 => ("0".to_string(), SHORT_INK),
            1000.. => ("999+".to_string(), ro_dark::STRONG),
            _ => (count.to_string(), ro_dark::STRONG),
        };
        marks.push(Box::new(bsn! {
            @badge(text, 10.0, ink)
            Node { right: px(3), bottom: px(2) }
        }));
    }
    match (filled.buff.clone(), filled.level) {
        (Some(left), _) => marks.push(Box::new(bsn! {
            @badge(left, 10.0, ro_dark::BRONZE_LIGHT)
            Node { right: px(3), bottom: px(2) }
        })),
        (None, Some(level)) => marks.push(Box::new(bsn! {
            @badge(format!("Lv{level}"), 9.0, LEVEL_INK)
            Node { left: px(3), bottom: px(2) }
        })),
        (None, None) => {}
    }
    if filled.lacks_sp {
        marks.push(Box::new(bsn! {
            @badge("SP".to_string(), 8.0, super::view::SP_INK)
            Node { right: px(3), top: px(2) }
        }));
    }
    marks
}

/// The wedge of a cooldown still to run, clockwise from the top: `fraction` of a turn.
pub(super) fn sweep(fraction: f32) -> BackgroundGradient {
    let angle = fraction.clamp(0.0, 1.0) * TAU;
    let clear = SHADE.with_alpha(0.0);
    BackgroundGradient::from(ConicGradient::new(
        UiPosition::CENTER,
        vec![
            AngularColorStop::new(SHADE, 0.0),
            AngularColorStop::new(SHADE, angle),
            AngularColorStop::new(clear, angle),
            AngularColorStop::new(clear, TAU),
        ],
    ))
}

/// The cooldown over a skill's icon: the wedge `animate` turns, and the time left.
fn cooldown(skill: u32, left: String) -> impl Scene {
    let (mark, wedge) = (Sweep(skill), sweep(1.0));
    bsn! {
        mark
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(4)),
        }
        wedge
        @ignore_picking()
        Children [
            @chrome_text(left, 13.0, ro_dark::STRONG)
            TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
        ]
    }
}

/// The cast bar along a slot's bottom edge; `animate` sets how far it has come.
fn cast_bar() -> impl Scene {
    bsn! {
        CastFill
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            bottom: px(0),
            width: percent(0),
            height: px(3),
        }
        BackgroundColor(ro_dark::FROST_GLOW)
        @ignore_picking()
    }
}

/// A resource over its limit: the label, the amount and a thin bar.
pub(super) fn vital(
    label: &'static str,
    ink: Color,
    bar: Color,
    (amount, limit): (u32, u32),
) -> impl Scene {
    let fill = (amount as f32 / limit.max(1) as f32).min(1.0) * 100.0;
    let trough = Color::BLACK.with_alpha(0.6);
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(2) }
        @ignore_picking()
        Children [
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
            }
            @ignore_picking()
            Children [
                @chrome_text(label.into(), 9.0, ink)
                --
                @chrome_text(thousands(amount.into()), 9.0, ro_dark::STRONG)
            ]
            --
            Node {
                height: px(4),
                border_radius: BorderRadius::all(px(2)),
                overflow: {Overflow::clip()},
            }
            BackgroundColor(trough)
            @ignore_picking()
            Children [
                Node { width: {Val::Percent(fill)}, height: percent(100) }
                BackgroundColor(bar)
                @ignore_picking()
            ]
        ]
    }
}

/// A plate button that stays lit while what it switches is `on`.
fn toggle(icon: &'static str, on: bool) -> impl Scene {
    let lit = on.then(|| {
        let fill = Color::srgb_u8(0x2a, 0x32, 0x3d);
        let colors = HoverColors {
            rest: (fill, ro_dark::SELECT_RING),
            hover: (ro_dark::BUTTON, ro_dark::SELECT_RING),
        };
        bsn! {
            colors
            BackgroundColor(fill)
            BorderColor::all(ro_dark::SELECT_RING)
        }
    });
    bsn! {
        @plate_button(icon)
        @{lit}
    }
}

/// The pager over the row and lock switches. `pages` names the pages on show.
///
/// Beside two rows the pager stands on end. That is taller than one row of slots, so
/// beside one it lies flat and the buttons are exactly as tall as a slot.
pub(super) fn buttons(pages: String, two_rows: bool, locked: bool) -> impl Scene {
    // The chevron art points right.
    let up = UiTransform::from_rotation(Rot2::degrees(-90.0));
    let down = UiTransform::from_rotation(Rot2::degrees(90.0));
    let lock = if locked { "lock" } else { "unlock" };
    let (pager, gap) = if two_rows {
        (FlexDirection::Column, 6)
    } else {
        (FlexDirection::Row, 0)
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(gap),
        }
        @ignore_picking()
        Children [
            Node {
                flex_direction: {pager},
                align_items: AlignItems::Center,
                row_gap: px(3),
                column_gap: px(3),
            }
            @ignore_picking()
            Children [
                @plate_button("chev")
                up
                HotbarAction::PagePrev
                on(on_action)
                --
                @chrome_text(pages, 11.0, ro_dark::BRONZE_LIGHT)
                --
                @plate_button("chev")
                down
                HotbarAction::PageNext
                on(on_action)
            ]
            --
            Node { column_gap: px(3) }
            @ignore_picking()
            Children [
                @toggle("rows", two_rows)
                HotbarAction::ToggleRows
                on(on_action)
                --
                @toggle(lock, locked)
                HotbarAction::ToggleLock
                on(on_action)
            ]
        ]
    }
}

/// A popup's place on the plate: centred on `at.x`, standing on `at.y`.
fn hung(at: Vec2, width: f32) -> impl Scene {
    let centre = UiTransform::from_translation(Val2::new(percent(-50), Val::ZERO));
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(at.x),
            bottom: px(at.y),
            width: px(width),
            max_width: px(width),
        }
        centre
    }
}

/// The tooltip of the hovered slot, hung over it.
pub(super) fn tip(view: TipView, at: Vec2) -> impl Scene {
    let stats: Vec<_> = view
        .stats
        .into_iter()
        .map(|(label, value, ink)| {
            bsn! {
                Node { column_gap: px(4) }
                @ignore_picking()
                Children [
                    @chrome_text(label.into(), 11.0, ro_dark::FAINT)
                    --
                    @chrome_text(value, 11.0, ink)
                ]
            }
        })
        .collect();
    let rule = BorderColor::all(ro_dark::SLOT_STROKE);
    bsn! {
        @tooltip_frame()
        @hung(at, 232.0)
        Node { row_gap: px(5) }
        Children [
            Node { align_items: AlignItems::Baseline, column_gap: px(6) }
            @ignore_picking()
            Children [
                @chrome_text(view.name, 13.0, view.tone)
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                --
                @overline(view.tag.into(), ro_dark::ACCENT)
            ]
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(10), row_gap: px(4) }
            @ignore_picking()
            Children [ {stats} ]
            --
            Node { padding: {UiRect::top(px(5))}, border: {UiRect::top(px(1))} }
            rule
            @ignore_picking()
            Children [ @chrome_text(view.hint, 10.0, ro_dark::FAINT) ]
        ]
    }
}

/// The list an empty slot is filled from, hung over that slot. `key` names the slot.
pub(super) fn picker(key: String, tab: PickTab, choices: Vec<Choice>, at: Vec2) -> impl Scene {
    let tabs: Vec<_> = PickTab::ALL
        .into_iter()
        .map(|option| tab_button(option, option == tab))
        .collect();
    let nothing = choices.is_empty().then(|| {
        let text = match tab {
            PickTab::Skills => "No skills to cast yet",
            PickTab::Items => "No usable items in your bag",
        };
        EntityScene(bsn! {
            @chrome_text(text.into(), 11.0, ro_dark::MUTED)
            Node { margin: {UiRect::axes(px(8), px(8))} }
        })
    });
    let rows: Vec<_> = choices.into_iter().map(choice).collect();
    let rule = BorderColor::all(ro_dark::SLOT_STROKE);
    bsn! {
        @tooltip_frame()
        @hung(at, 248.0)
        Node { padding: px(8), row_gap: px(4) }
        Pickable { should_block_lower: true, is_hoverable: true }
        Children [
            Node {
                align_items: AlignItems::Center,
                column_gap: px(4),
                padding: {UiRect::bottom(px(6))},
                border: {UiRect::bottom(px(1))},
            }
            rule
            @ignore_picking()
            Children [
                @chrome_text(key, 11.0, ro_dark::ACCENT)
                Node { margin: {UiRect::right(px(4))} }
                --
                {tabs}
            ]
            --
            ScrollArea
            ScrollPosition
            Pickable
            Node {
                flex_direction: FlexDirection::Column,
                max_height: px(176),
                overflow: {Overflow::scroll_y()},
            }
            Children [ {nothing} -- {rows} ]
            --
            @chrome_text(
                "You can also drag from the Skills window or your bag".into(),
                10.0,
                ro_dark::FAINT,
            )
        ]
    }
}

fn tab_button(tab: PickTab, active: bool) -> impl Scene {
    let (ink, _) = tab_text(active);
    let action = HotbarAction::Tab(tab);
    bsn! {
        @segment(active)
        Button
        action
        on(on_action)
        Children [ @chrome_text(tab.label().into(), 12.0, ink) ]
    }
}

/// One thing the picker offers. What the bar already has is dimmed and cannot be
/// picked.
fn choice(choice: Choice) -> impl Scene {
    let action = HotbarAction::Pick(choice.slot);
    let colors = HoverColors {
        rest: (Color::NONE, Color::NONE),
        hover: (Color::WHITE.with_alpha(0.06), Color::NONE),
    };
    let free = !choice.on_bar;
    let alpha = if free { 1.0 } else { 0.4 };
    let icon = choice.icon.map(|path| {
        EntityScene(bsn! {
            ImageNode { image: {path}, color: {Color::WHITE.with_alpha(alpha)} }
            Node { width: px(18), height: px(18) }
            @ignore_picking()
        })
    });
    bsn! {
        Button
        Hovered
        colors
        action
        Node {
            height: px(34),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(Color::NONE)
        Pickable { should_block_lower: {free}, is_hoverable: {free} }
        on(on_action)
        Children [
            Node {
                width: px(26),
                height: px(26),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
                border_radius: BorderRadius::all(px(4)),
            }
            BackgroundColor(ro_dark::INSET)
            BorderColor::all(ro_dark::SLOT_STROKE)
            @ignore_picking()
            Children [ {icon} ]
            --
            @chrome_text(choice.name, 12.0, ro_dark::STRONG.with_alpha(alpha))
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            --
            @chrome_text(choice.meta, 11.0, ro_dark::MUTED)
        ]
    }
}

/// The pill over the plate that says what just happened, with Undo when it can be
/// taken back.
pub(super) fn toast(text: String, tone: Tone, undo: bool) -> impl Scene {
    let (ink, ring) = tone.style();
    let undo = undo.then(|| {
        EntityScene(bsn! {
            @link("Undo")
            Button
            HotbarAction::Undo
            on(on_action)
        })
    });
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: percent(100),
            justify_content: JustifyContent::Center,
            padding: {UiRect::bottom(px(18))},
        }
        @ignore_picking()
        Children [
            @tooltip_frame()
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                max_width: Val::Auto,
                padding: {UiRect::axes(px(12), px(6))},
                border_radius: BorderRadius::all(px(6)),
            }
            BorderColor::all(ring)
            Children [
                @chrome_text(text, 12.0, ink)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                {undo}
            ]
        ]
    }
}
