//! The Inventory window scenes: the frame [`build`] spawns once, and the body and
//! footer the rebuild systems project from their view-models. Sizes follow the
//! `Inventory` artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui::OverrideClip;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, WINDOW_SHADOW, button_with, close_button, diamond,
    display_text, divider, divider_diamond, inset, tab, tab_text, window_title_bar, window_titled,
};

use crate::screens::shared::{
    DANGER_TINT, Ring, flush_button, search_field, slot_frame, thousands, usable, weight_meter,
};
use crate::widgets::inventory_window::{
    COLUMNS, InventoryBody, InventoryCell, InventoryFooter, InventoryGrid, InventoryLabel,
    InventorySearchField, InventoryTab, InventoryTabButton, InventoryTitlebar, InventoryTooltip,
    InventoryWindowRoot, act_on_selection, close_window, cycle_sort, hide_slot_name, on_cell_click,
    on_cell_drag_start, pick_tab, show_slot_name,
};

const WINDOW_LEFT: f32 = 320.0;
const WINDOW_TOP: f32 = 70.0;
const WINDOW_WIDTH: f32 = 540.0;
const GRID_HEIGHT: f32 = 306.0;
const SLOT: f32 = 50.0;
const SLOT_GAP: f32 = 8.0;
const ICON: f32 = 36.0;
const SELECTION_SLOT: f32 = 44.0;
/// Above every window, like the hotbar's name tags.
const TOOLTIP_Z: i32 = 1500;
const SELECTION_ICON: f32 = 32.0;

pub(super) struct TabView {
    pub tab: InventoryTab,
    pub count: usize,
    pub active: bool,
}

/// One filled slot. `pips` holds one entry per card slot, `true` when a card sits in it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellView {
    pub index: u16,
    pub icon: Option<String>,
    pub amount: u16,
    pub refine: u8,
    pub identified: bool,
    pub damaged: bool,
    pub favorite: bool,
    pub pips: Vec<bool>,
    pub ring: Ring,
}

pub(super) struct SelectionView {
    pub cell: CellView,
    pub name: String,
    pub meta: String,
    pub action_label: &'static str,
    pub action_enabled: bool,
}

/// Everything [`body`] draws. `cells` is the whole grid in order, `None` for an empty
/// slot, and `scroll` is the offset the new grid starts at.
pub(super) struct BodyView {
    pub tabs: [TabView; 5],
    pub cells: Vec<Option<CellView>>,
    pub scroll: Vec2,
    pub no_match: Option<String>,
    pub selection: Option<SelectionView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FooterView {
    pub weight: u32,
    pub max_weight: u32,
    pub zeny: u32,
}

/// Spawn the window, hidden, under `parent`.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        InventoryWindowRoot
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
            @title_bar()
            --
            @search_row()
            --
            InventoryBody
            Node { flex_direction: FlexDirection::Column, margin: {UiRect::top(px(12))} }
            @ignore_picking()
            --
            @divider(divider_diamond())
            Node { margin: {UiRect::vertical(px(12))} }
            --
            InventoryFooter
            Node { flex_direction: FlexDirection::Column, margin: {UiRect::bottom(px(8))} }
            @ignore_picking()
        ]
    }
}

fn title_bar() -> impl Scene {
    bsn! {
        InventoryTitlebar
        @window_title_bar("Inventory", bsn! {
            Node { align_items: AlignItems::Center, column_gap: px(12) }
            @ignore_picking()
            Children [
                InventoryLabel::Used
                @chrome_text(String::new(), 12.0, ro_dark::LABEL)
                --
                @close_button()
                on(close_window)
            ]
        })
        Pickable
        on(drag_window::<InventoryTitlebar, InventoryWindowRoot>)
    }
}

fn search_row() -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: {UiRect::top(px(10))},
        }
        @ignore_picking()
        Children [
            @search_field::<InventorySearchField>("Search items")
            --
            @chrome_text("Sort".into(), 12.0, ro_dark::MUTED)
            --
            @sort_button()
        ]
    }
}

/// The sort control. It cycles through the sort orders on click.
fn sort_button() -> impl Scene {
    let colors = HoverColors {
        rest: (ro_dark::INSET, ro_dark::SLOT_STROKE),
        hover: (ro_dark::INSET, ro_dark::BUTTON_STROKE),
    };
    bsn! {
        Hovered
        colors
        Node {
            width: px(112),
            height: px(28),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(ro_dark::SLOT_STROKE)
        Pickable
        on(cycle_sort)
        Children [
            InventoryLabel::Sort
            @chrome_text(String::new(), 13.0, ro_dark::BODY)
            --
            @glyph_icon("chev", 10.0, ro_dark::PRIMARY_STROKE)
        ]
    }
}

/// The rebuilt part of the window: tabs over the grid over the selection bar.
pub(super) fn body(view: BodyView) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @tab_strip(view.tabs)
            --
            @grid_panel(view.cells, view.scroll, view.no_match)
            --
            @selection_bar(view.selection)
        ]
    }
}

fn tab_strip(tabs: [TabView; 5]) -> impl Scene {
    let buttons: Vec<_> = tabs.into_iter().map(tab_button).collect();
    bsn! {
        Node { column_gap: px(4) }
        @ignore_picking()
        Children [ {buttons} ]
    }
}

/// A tab with its item count.
fn tab_button(view: TabView) -> impl Scene {
    let (category, active) = (view.tab, view.active);
    let (label, count) = tab_text(active);
    let mark = (category == InventoryTab::Favorites)
        .then(|| EntityScene(diamond(7.0, ro_dark::BRONZE_LIGHT, ro_dark::BRONZE_LIGHT)));
    bsn! {
        InventoryTabButton(category)
        @tab(active)
        on(pick_tab)
        Children [
            {mark}
            --
            @chrome_text(category.label().into(), 13.0, label)
            --
            @chrome_text(view.count.to_string(), 11.0, count)
        ]
    }
}

/// The slot grid: a fixed-height scroll viewport of [`COLUMNS`]-wide rows with the
/// scrollbar pinned to its right. Rows are explicit, so no slot can wrap at a
/// fractional UI scale.
fn grid_panel(cells: Vec<Option<CellView>>, scroll: Vec2, no_match: Option<String>) -> impl Scene {
    let mut slots = cells.into_iter().map(slot).peekable();
    let mut rows = Vec::new();
    while slots.peek().is_some() {
        rows.push(slot_row(slots.by_ref().take(COLUMNS).collect()));
    }
    let message = no_match.map(|text| EntityScene(grid_message(text)));
    bsn! {
        @inset()
        Node {
            height: px(GRID_HEIGHT),
            position_type: PositionType::Relative,
            border_radius: {BorderRadius::all(px(6)).with_top_left(px(0))},
        }
        @ignore_picking()
        Children [
            #grid
            InventoryGrid
            ScrollArea
            ScrollPosition(scroll)
            Pickable
            Node {
                position_type: PositionType::Absolute,
                left: px(0), top: px(0), right: px(0), bottom: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(SLOT_GAP),
                padding: {UiRect::new(px(12), px(22), px(12), px(12))},
                overflow: {Overflow::scroll_y()},
            }
            Children [ {rows} ]
            --
            @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(6),
                top: px(8),
                bottom: px(8),
                width: px(8),
            }
            --
            {message}
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

fn grid_message(text: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(118),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [ @chrome_text(text, 13.0, ro_dark::MUTED) ]
    }
}

fn slot(cell: Option<CellView>) -> Box<dyn Scene> {
    match cell {
        Some(view) => Box::new(item_slot(view)),
        None => Box::new(bsn! { @slot_frame(SLOT, Ring::None) @ignore_picking() }),
    }
}

/// A filled slot: click to select, double-click to act, right-click to inspect, drag
/// to the hotbar.
fn item_slot(view: CellView) -> impl Scene {
    let (index, ring) = (view.index, view.ring);
    let (stroke, _, _) = ring.style();
    let hover = match ring {
        Ring::None => (ro_dark::SLOT_HOVER, ro_dark::BUTTON_STROKE),
        Ring::Rare | Ring::Selected => (ro_dark::SLOT_HOVER, stroke),
    };
    let colors = HoverColors {
        rest: (ro_dark::SLOT, stroke),
        hover,
    };
    let marks = slot_marks(&view, ICON, true);
    bsn! {
        InventoryCell { index: index }
        Hovered
        colors
        @slot_frame(SLOT, ring)
        Pickable
        on(on_cell_click)
        on(crate::widgets::chat_box::link_item_on_shift_click)
        on(on_cell_drag_start)
        on(show_slot_name)
        on(hide_slot_name)
        Children [ {marks} ]
    }
}

/// The name tag centred above a hovered slot. `OverrideClip` lets it leave the grid's
/// scroll viewport, which would otherwise cut it off over the top row.
pub(super) fn slot_tooltip(name: String) -> impl Scene {
    let centre = UiTransform::from_translation(Val2::new(percent(-50), Val::ZERO));
    bsn! {
        InventoryTooltip
        Node {
            position_type: PositionType::Absolute,
            bottom: px(SLOT + 2.0),
            left: percent(50),
            padding: {UiRect::axes(px(10), px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        centre
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        GlobalZIndex(TOOLTIP_Z)
        OverrideClip
        @ignore_picking()
        Children [
            @chrome_text(name, 12.0, ro_dark::BODY)
            TextLayout { linebreak: LineBreak::NoWrap }
        ]
    }
}

/// What a slot draws over its frame: the icon and the unidentified mark, plus the
/// damage wash and the refine, amount, card and favourite `badges` of a grid slot.
pub(crate) fn slot_marks(view: &CellView, icon: f32, badges: bool) -> Vec<Box<dyn Scene>> {
    let mut marks: Vec<Box<dyn Scene>> = Vec::new();
    if let Some(path) = view.icon.clone() {
        marks.push(Box::new(slot_icon(path, icon, view.identified)));
    }
    if !view.identified {
        marks.push(Box::new(unidentified_mark()));
    }
    if !badges {
        return marks;
    }
    if view.damaged {
        marks.push(Box::new(damage_wash()));
    }
    if view.refine > 0 {
        marks.push(Box::new(bsn! {
            @badge(format!("+{}", view.refine), ro_dark::BRONZE_LIGHT)
            Node { left: px(3), top: px(2) }
        }));
    }
    if view.amount > 1 {
        marks.push(Box::new(bsn! {
            @badge(thousands(view.amount.into()), ro_dark::VALUE)
            Node { right: px(3), bottom: px(2) }
        }));
    }
    if !view.pips.is_empty() {
        marks.push(Box::new(card_pips(view.pips.clone())));
    }
    if view.favorite {
        marks.push(Box::new(bsn! {
            @diamond(6.0, ro_dark::BRONZE_LIGHT, ro_dark::BRONZE_LIGHT)
            Node { position_type: PositionType::Absolute, right: px(4), top: px(4) }
        }));
    }
    marks
}

/// The item icon. An unidentified item's is dimmed behind its question mark.
fn slot_icon(path: String, size: f32, identified: bool) -> impl Scene {
    let alpha = if identified { 1.0 } else { 0.28 };
    bsn! {
        ImageNode { image: {path}, color: {Color::WHITE.with_alpha(alpha)} }
        Node { width: px(size), height: px(size) }
        @ignore_picking()
    }
}

fn unidentified_mark() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0), top: px(0), right: px(0), bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [ @display_text("?".into(), 22.0, ro_dark::TITLE) ]
    }
}

fn damage_wash() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(1), top: px(1), right: px(1), bottom: px(1),
        }
        BackgroundColor({ro_dark::HP.with_alpha(0.24)})
        @ignore_picking()
    }
}

/// A corner label over the icon. The caller pins it with `left`/`right`/`top`/`bottom`.
fn badge(text: String, color: Color) -> impl Scene {
    bsn! {
        @chrome_text(text, 10.5, color)
        TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
        Node { position_type: PositionType::Absolute }
    }
}

/// One diamond per card slot along the bottom-left, lit when a card sits in it.
fn card_pips(pips: Vec<bool>) -> impl Scene {
    let pips: Vec<_> = pips
        .into_iter()
        .map(|filled| {
            if filled {
                diamond(5.0, ro_dark::FROST, ro_dark::FROST_GLOW)
            } else {
                diamond(5.0, Color::NONE, ro_dark::MUTED)
            }
        })
        .collect();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(4),
            bottom: px(4),
            column_gap: px(4),
        }
        @ignore_picking()
        Children [ {pips} ]
    }
}

/// The bar under the grid: the selected item with its actions, or a hint.
fn selection_bar(selection: Option<SelectionView>) -> impl Scene {
    let content = match selection {
        Some(view) => selected_item(view),
        None => vec![Box::new(chrome_text(
            "Select an item \u{b7} Right-click it for details".into(),
            12.0,
            ro_dark::MUTED,
        )) as Box<dyn Scene>],
    };
    bsn! {
        @inset()
        Node {
            height: px(64),
            margin: {UiRect::top(px(12))},
            padding: {UiRect::new(px(10), px(12), px(0), px(0))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(12),
        }
        @ignore_picking()
        Children [ {content} ]
    }
}

/// The selected item's slot, name and meta line, then its buttons. Button widths
/// include the art's baked shadow, which `flush_button` cancels with a negative margin.
fn selected_item(view: SelectionView) -> Vec<Box<dyn Scene>> {
    let ring = view.cell.ring;
    let name_color = if ring == Ring::Rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    };
    let favorite_fill = if view.cell.favorite {
        ro_dark::BRONZE_LIGHT
    } else {
        Color::NONE
    };
    let marks = slot_marks(&view.cell, SELECTION_ICON, false);
    let (label, enabled) = (view.action_label, view.action_enabled);
    vec![
        Box::new(bsn! {
            @slot_frame(SELECTION_SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
        }),
        Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.name, 13.0, name_color)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(view.meta, 11.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
        }),
        Box::new(bsn! {
            @flush_button(ButtonArt::PRIMARY, label)
            Node { width: px(76.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            @usable(enabled, Color::WHITE)
            on(act_on_selection)
        }),
        // NOTE: the favourite and drop buttons are disabled placeholders: the protocol
        // has no command for either yet. Wire them when aesir gains those messages.
        Box::new(bsn! {
            @button_with(
                ButtonArt::NORMAL,
                diamond(9.0, favorite_fill, ro_dark::BRONZE_LIGHT),
            )
            Node {
                width: px(32.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                margin: {UiRect::all(px(-BUTTON_SHADOW))},
            }
            @usable(false, Color::WHITE)
        }),
        Box::new(bsn! {
            @flush_button(ButtonArt::NORMAL, "Drop")
            Node { width: px(68.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            @usable(false, DANGER_TINT)
        }),
    ]
}

/// The weight meter and the zeny box.
pub(super) fn footer(view: FooterView) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::FlexEnd, column_gap: px(20) }
        @ignore_picking()
        Children [
            @weight_meter(view.weight, view.max_weight)
            --
            @zeny_box(view.zeny)
        ]
    }
}

fn zeny_box(zeny: u32) -> impl Scene {
    let amount = format!("{} z", thousands(zeny.into()));
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(10), flex_shrink: 0.0 }
        @ignore_picking()
        Children [
            @chrome_text("Zeny".into(), 12.0, ro_dark::ACCENT)
            --
            @inset()
            Node {
                width: px(132),
                height: px(28),
                padding: {UiRect::horizontal(px(10))},
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                border_radius: BorderRadius::all(px(5)),
            }
            @ignore_picking()
            Children [ @chrome_text(amount, 13.0, ro_dark::BRONZE_LIGHT) ]
        ]
    }
}
