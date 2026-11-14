//! The Storage window scenes: the frame [`build`] spawns once, and the category
//! segments and body [`rebuild`](super::rebuild) projects from their view-models.
//! Sizes follow the `Storage` artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, INPUT_SHADOW, WINDOW_SHADOW, button_with, close_button,
    diamond, inset, segment, tab_text, window_title_bar, window_titled,
};

use super::view::{BodyView, PaneView, SelectionView, action_label, slider_percent};
use super::{
    AmountFill, AmountKnob, AmountLabel, AmountRail, Preset, Side, StorageAction,
    StorageAmountField, StorageCategory, StorageCell, StorageHost, StoragePane, StorageSearchField,
    StorageSelection, StorageSort, StorageToast, StorageWindowRoot, StorageWindowTitlebar,
    on_cell_click, on_cell_drag_end, on_cell_drag_start, on_pane_drop, on_slider_drag,
    on_slider_press, on_storage_button, on_storage_click, on_storage_close,
};
use crate::screens::shared::{
    QTY_DIGITS, Ring, bar, cycle_box, number_field, overline, row_button, search_field, slot_frame,
    small_button, thousands, usable, weight_meter,
};
use crate::widgets::inventory_window::scene::{CellView, slot_marks};

const WINDOW_LEFT: f32 = 100.0;
const WINDOW_TOP: f32 = 40.0;
const WINDOW_WIDTH: f32 = 1004.0;
const PANE_WIDTH: f32 = 418.0;
/// The column of move arrows: what the two panes and their gaps leave of the row.
const MOVE_WIDTH: f32 = 88.0;
/// Fixed height of each pane's grid, five rows, so the window never grows with its
/// content.
const PANE_HEIGHT: f32 = 304.0;
const COLUMNS: usize = 7;
/// Slots a pane draws at least: five whole rows.
const MIN_SLOTS: usize = 35;
const SLOT: f32 = 50.0;
const ICON: f32 = 36.0;
/// Seven slots and six gaps fill the 392px a pane leaves inside its padding.
const SLOT_GAP: f32 = 7.0;
const ROW_GAP: f32 = 7.0;
const SELECTION_SLOT: f32 = 44.0;
const SELECTION_ICON: f32 = 32.0;
/// How far down the frame the toast floats: over the upper rows of the panes.
const TOAST_TOP: f32 = 250.0;

/// Spawn the whole window, hidden, as one scene under `parent`.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

pub(crate) fn window() -> impl Scene {
    bsn! {
        StorageWindowRoot
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
        Pickable
        Children [
            StorageWindowTitlebar
            @window_title_bar("Storage", bsn! {
                @close_button()
                on(on_storage_close)
            })
            Pickable
            on(drag_window::<StorageWindowTitlebar, StorageWindowRoot>)
            --
            @toolbar()
            --
            StorageHost::Body
            Node {
                flex_direction: FlexDirection::Column,
                margin: {UiRect::new(px(0), px(0), px(12), px(8))},
            }
            @ignore_picking()
            --
            @toast()
        ]
    }
}

/// The search box, which outlives every rebuild so typing keeps its caret, beside
/// the host of the category segments.
fn toolbar() -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: {UiRect::top(px(10))},
        }
        @ignore_picking()
        Children [
            @search_field::<StorageSearchField>("Search bag and vault")
            Node { width: px(220.0 + 2.0 * INPUT_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            --
            StorageHost::Filters
            Node {}
            @ignore_picking()
        ]
    }
}

/// The pill [`show_toast`](super::show_toast) writes the last outcome into.
fn toast() -> impl Scene {
    bsn! {
        StorageToast
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(TOAST_TOP),
            justify_content: JustifyContent::Center,
        }
        Visibility::Hidden
        @ignore_picking()
        Children [
            @chrome_text(String::new(), 12.0, ro_dark::BODY)
            Node {
                padding: {UiRect::axes(px(12), px(6))},
                border: px(1),
                border_radius: BorderRadius::all(px(6)),
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::BUTTON_STROKE)
        ]
    }
}

/// The category segments, with `active` lit.
pub(crate) fn filters(active: StorageCategory) -> impl Scene {
    let segments: Vec<_> = StorageCategory::ALL
        .into_iter()
        .map(|category| category_segment(category, category == active))
        .collect();
    bsn! {
        Node { column_gap: px(4) }
        @ignore_picking()
        Children [ {segments} ]
    }
}

fn category_segment(category: StorageCategory, active: bool) -> impl Scene {
    let (ink, _) = tab_text(active);
    let action = StorageAction::Category(category);
    bsn! {
        @segment(active)
        action
        on(on_storage_click)
        Children [ @chrome_text(category.label().into(), 12.0, ink) ]
    }
}

/// The rebuilt body: the panes around the move arrows, the selection bar, then the
/// weight, the zeny and the last action with its Undo.
pub(crate) fn body(view: BodyView) -> impl Scene {
    let side = view
        .selection
        .as_ref()
        .map(|selection| selection.source.side());
    let busy = view.busy;
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        @ignore_picking()
        Children [
            Node { column_gap: px(16), align_items: AlignItems::Stretch }
            @ignore_picking()
            Children [
                Node { width: px(PANE_WIDTH), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @pane(view.bag)
                    --
                    @bag_footer(view.loot, view.top_up, busy)
                ]
                --
                @move_column(side, busy)
                --
                Node { width: px(PANE_WIDTH), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @pane(view.vault)
                    --
                    @vault_footer(view.capacity, view.sort)
                ]
            ]
            --
            @selection_bar(view.selection, view.qty, view.asking, busy)
            --
            @footer(view.weight, view.zeny, view.last, busy)
        ]
    }
}

/// One pane: its caption over a fixed-height scrolling grid of [`COLUMNS`]-wide rows.
/// Rows are explicit, so no slot can wrap at a fractional UI scale. The grid's
/// outline lights up while a slot of the other pane is dragged.
fn pane(view: PaneView) -> impl Scene {
    let PaneView {
        side,
        title,
        count,
        hint: (hint, hint_color),
        cells,
        scroll,
    } = view;
    let total = (cells.len() + 1).next_multiple_of(COLUMNS).max(MIN_SLOTS);
    let mut slots = cells
        .into_iter()
        .map(Some)
        .chain(std::iter::repeat_with(|| None))
        .take(total)
        .map(slot)
        .peekable();
    let mut rows = Vec::new();
    while slots.peek().is_some() {
        rows.push(slot_row(slots.by_ref().take(COLUMNS).collect()));
    }
    bsn! {
        Node { flex_direction: FlexDirection::Column, margin: {UiRect::bottom(px(10))} }
        @ignore_picking()
        Children [
            Node {
                height: px(22),
                margin: {UiRect::bottom(px(6))},
                align_items: AlignItems::Center,
                column_gap: px(8),
            }
            @ignore_picking()
            Children [
                @overline(title.into(), ro_dark::ACCENT)
                --
                @chrome_text(count, 12.0, ro_dark::STRONG)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(hint, 11.0, hint_color)
            ]
            --
            @inset()
            Node { height: px(PANE_HEIGHT), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #grid
                StoragePane(side)
                ScrollArea
                ScrollPosition(scroll)
                Outline { width: px(2), offset: px(0), color: Color::NONE }
                Pickable
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(ROW_GAP),
                    padding: {UiRect::new(px(12), px(14), px(12), px(12))},
                    overflow: {Overflow::scroll_y()},
                }
                on(on_pane_drop)
                Children [ {rows} ]
                --
                @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
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

fn slot_row(slots: Vec<Box<dyn Scene>>) -> impl Scene {
    bsn! {
        Node { column_gap: px(SLOT_GAP), flex_shrink: 0.0 }
        @ignore_picking()
        Children [ {slots} ]
    }
}

/// A filled slot selects on click, moves its whole stack on a double-click, inspects
/// on a right-click and can be dragged to the other pane. An empty one is inert.
fn slot(cell: Option<(StorageSelection, CellView)>) -> Box<dyn Scene> {
    let Some((source, view)) = cell else {
        return Box::new(bsn! { @slot_frame(SLOT, Ring::None) @ignore_picking() });
    };
    let ring = view.ring;
    let (stroke, _, _) = ring.style();
    let hover = match ring {
        Ring::None => (ro_dark::SLOT_HOVER, ro_dark::BUTTON_STROKE),
        Ring::Rare | Ring::Selected => (ro_dark::SLOT_HOVER, stroke),
    };
    let colors = HoverColors {
        rest: (ro_dark::SLOT, stroke),
        hover,
    };
    let cell = StorageCell(source);
    let marks = slot_marks(&view, ICON, true);
    Box::new(bsn! {
        cell
        Hovered
        colors
        @slot_frame(SLOT, ring)
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        on(on_cell_drag_end)
        Children [ {marks} ]
    })
}

/// The bulk deposits under the Bag pane, each naming how many stacks it would move.
fn bag_footer(loot: usize, top_up: usize, busy: bool) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @bulk_button("Deposit loot", loot, 118.0, busy, StorageAction::DepositLoot)
            --
            @bulk_button("Top up stacks", top_up, 124.0, busy, StorageAction::TopUp)
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            @chrome_text("Favorites stay in the bag".into(), 11.0, ro_dark::MUTED)
        ]
    }
}

fn bulk_button(
    label: &'static str,
    stacks: usize,
    width: f32,
    busy: bool,
    action: StorageAction,
) -> impl Scene {
    let label = match stacks {
        0 => label.to_string(),
        stacks => format!("{label} ({stacks})"),
    };
    bsn! {
        @row_button(ButtonArt::NORMAL, label, width)
        @usable(stacks > 0 && !busy, Color::WHITE)
        action
        on(on_storage_button)
    }
}

/// How full the vault is, and the order its pane is in. The sort box steps to the
/// next order on click.
fn vault_footer((used, limit): (u32, u32), sort: StorageSort) -> impl Scene {
    let fill = (used as f32 / limit.max(1) as f32).min(1.0) * 100.0;
    let label = chrome_text(sort.label().into(), 13.0, ro_dark::BODY);
    let action = StorageAction::CycleSort;
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @chrome_text("Capacity".into(), 12.0, ro_dark::MUTED)
            --
            @bar(fill, ro_dark::SP, false)
            Node { flex_grow: 1.0, min_width: px(0) }
            --
            @chrome_text("Sort".into(), 12.0, ro_dark::MUTED)
            --
            @cycle_box(label, 104.0)
            action
            on(on_storage_click)
        ]
    }
}

/// The arrows between the panes. The one pointing away from the selected item's pane
/// is lit and moves it, exactly like the commit button.
fn move_column(selected: Option<Side>, busy: bool) -> impl Scene {
    let deposit = !busy && selected == Some(Side::Bag);
    let withdraw = !busy && selected == Some(Side::Vault);
    bsn! {
        Node {
            width: px(MOVE_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(14),
            padding: {UiRect::bottom(px(20))},
        }
        @ignore_picking()
        Children [
            @move_arrow("chevr", "Deposit", deposit)
            --
            @diamond(6.0, ro_dark::DIVIDER, ro_dark::DIVIDER)
            --
            @move_arrow("chevl", "Withdraw", withdraw)
        ]
    }
}

fn move_arrow(icon: &'static str, label: &'static str, lit: bool) -> impl Scene {
    let art = if lit {
        ButtonArt::PRIMARY
    } else {
        ButtonArt::NORMAL
    };
    let action = StorageAction::Commit;
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(6),
        }
        @ignore_picking()
        Children [
            @button_with(art, glyph_icon(icon, 18.0, ro_dark::BUTTON_LABEL))
            Node {
                width: px(64.0 + 2.0 * BUTTON_SHADOW),
                height: px(48.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                margin: {UiRect::all(px(-BUTTON_SHADOW))},
            }
            @usable(lit, Color::WHITE)
            action
            on(on_storage_button)
            --
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
        ]
    }
}

/// The bar under the panes: the selected item with its amount and the commit button,
/// or a hint. It is ringed while it asks how many of a dropped stack to move.
fn selection_bar(
    selection: Option<SelectionView>,
    qty: u32,
    asking: bool,
    busy: bool,
) -> impl Scene {
    let content = match selection {
        Some(view) => selected_item(view, qty, asking, busy),
        None => vec![Box::new(chrome_text(
            "Select an item \u{b7} Drag it across, or double-click to move the whole stack".into(),
            12.0,
            ro_dark::MUTED,
        )) as Box<dyn Scene>],
    };
    let ring = if asking {
        ro_dark::FROST_GLOW
    } else {
        Color::NONE
    };
    bsn! {
        @inset()
        Node {
            height: px(64),
            padding: {UiRect::new(px(10), px(12), px(0), px(0))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(14),
        }
        Outline { width: px(1), offset: px(0), color: {ring} }
        @ignore_picking()
        Children [ {content} ]
    }
}

/// The selected item's slot, name and meta line, then the amount controls of a stack
/// or a note for a single item, then the commit button.
fn selected_item(view: SelectionView, qty: u32, asking: bool, busy: bool) -> Vec<Box<dyn Scene>> {
    let ring = view.cell.ring;
    let name_color = if ring == Ring::Rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    };
    let name = if asking {
        format!("How many? \u{b7} {}", view.name)
    } else {
        view.name
    };
    let marks = slot_marks(&view.cell, SELECTION_ICON, false);
    let (line, tone) = view.meta.line(qty);
    let meta = view.meta;
    let label = action_label(view.source, qty);
    let action = StorageAction::Commit;
    let amount: Box<dyn Scene> = if view.stack > 1 {
        Box::new(amount_controls(qty, view.stack))
    } else {
        Box::new(bsn! {
            @chrome_text(view.note.into(), 12.0, ro_dark::MUTED)
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
        })
    };
    vec![
        Box::new(bsn! {
            @slot_frame(SELECTION_SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
        }),
        Box::new(bsn! {
            Node {
                width: px(230),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(name, 13.0, name_color)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                meta
                @chrome_text(line, 11.0, tone)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
        }),
        amount,
        Box::new(bsn! {
            @button_with(ButtonArt::PRIMARY, bsn! {
                AmountLabel
                @chrome_text(label, 13.0, ro_dark::BUTTON_LABEL)
            })
            Node {
                width: px(150.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                margin: {UiRect::all(px(-BUTTON_SHADOW))},
            }
            @usable(!busy, Color::WHITE)
            action
            on(on_storage_button)
        }),
    ]
}

/// The steppers around the typed amount, the slider and the amount chips.
fn amount_controls(qty: u32, stack: u32) -> impl Scene {
    let chips: Vec<_> = Preset::ALL.into_iter().map(amount_chip).collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            @step_button("minus", false)
            --
            @number_field(qty.to_string(), QTY_DIGITS)
            StorageAmountField
            TextLayout { justify: Justify::Center }
            Node { width: px(64) }
            --
            @step_button("plus", true)
            --
            @amount_slider(qty, stack)
            --
            {chips}
        ]
    }
}

fn step_button(icon: &'static str, more: bool) -> impl Scene {
    let action = StorageAction::Step(more);
    bsn! {
        @small_button(icon, 24.0)
        action
        on(on_storage_button)
    }
}

fn amount_chip(preset: Preset) -> impl Scene {
    let action = StorageAction::Amount(preset);
    bsn! {
        @segment(false)
        action
        on(on_storage_click)
        Children [ @chrome_text(preset.label().into(), 11.0, ro_dark::LABEL) ]
    }
}

/// The amount slider: a press or a drag anywhere on it sets the amount from the
/// pointer. [`sync_amount`](super::sync_amount) moves the fill and the knob.
fn amount_slider(qty: u32, stack: u32) -> impl Scene {
    let at = Val::Percent(slider_percent(qty, stack));
    bsn! {
        AmountRail
        RelativeCursorPosition
        Node {
            flex_grow: 1.0,
            min_width: px(60),
            height: px(16),
            align_items: AlignItems::Center,
            position_type: PositionType::Relative,
        }
        Pickable
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
                AmountFill
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: {at},
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor(ro_dark::SP)
                @ignore_picking()
            ]
            --
            AmountKnob
            @diamond(11.0, ro_dark::FROST, ro_dark::BRONZE_SHADOW)
            Node {
                position_type: PositionType::Absolute,
                left: {at},
                top: px(2.5),
                margin: {UiRect::left(px(-5.5))},
            }
        ]
    }
}

/// The carried weight, the zeny and, once something moved, the last action with its
/// Undo.
fn footer(
    (weight, max_weight): (u32, u32),
    zeny: u32,
    last: Option<String>,
    busy: bool,
) -> impl Scene {
    let zeny = format!("{} z", thousands(zeny.into()));
    let action = StorageAction::Undo;
    let undo: Vec<Box<dyn Scene>> = match last {
        None => Vec::new(),
        Some(text) => vec![
            Box::new(bsn! {
                Node { max_width: px(300), overflow: {Overflow::clip()} }
                @ignore_picking()
                Children [
                    @chrome_text(text, 12.0, ro_dark::LABEL)
                    TextLayout { linebreak: LineBreak::NoWrap }
                ]
            }),
            Box::new(bsn! {
                @row_button(ButtonArt::NORMAL, "Undo", 64.0)
                @usable(!busy, Color::WHITE)
                action
                on(on_storage_button)
            }),
        ],
    };
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(20) }
        @ignore_picking()
        Children [
            Node { width: px(300), flex_shrink: 0.0 }
            @ignore_picking()
            Children [ @weight_meter(weight, max_weight) ]
            --
            Node { align_items: AlignItems::Baseline, column_gap: px(8), flex_shrink: 0.0 }
            @ignore_picking()
            Children [
                @chrome_text("Zeny".into(), 12.0, ro_dark::ACCENT)
                --
                @chrome_text(zeny, 13.0, ro_dark::BRONZE_LIGHT)
            ]
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            {undo}
        ]
    }
}
