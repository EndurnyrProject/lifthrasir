//! Idiomatic BSN chrome for the inventory window (mirrors the shop/equipment
//! windows). [`window`] builds the persistent chrome — root, titlebar, and an
//! empty body region — as one `bsn!` tree; [`body`] projects the live
//! `Inventory` + `InventoryUi` into the tab strip, a fixed-height scrollable
//! item grid, and the selection info panel, and is respawned by
//! [`rebuild_body`](super::rebuild_body) on every change.
//!
//! The window is fixed-size: the grid and info panel are given a fixed
//! [`PANE_HEIGHT`] and scroll internally (`ScrollArea` + `FeathersScrollbar`)
//! instead of growing the window as items are added.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::inventory::{Inventory, Item, ItemCategory};
use game_engine::infrastructure::item::ItemDb;

use crate::rich_text::parse_color_codes;
use crate::theme;
use crate::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::{body_container, chrome_text, glyph_icon, ignore_picking, titlebar};

use super::{
    items_for_tab, on_cell_click, on_cell_drag_start, on_tab_click, tab_count, InventoryCell,
    InventoryTab, InventoryTitlebar, InventoryUi, InventoryWindowBody, InventoryWindowRoot, TABS,
};

const WINDOW_LEFT: f32 = 320.0;
const WINDOW_TOP: f32 = 110.0;
const WINDOW_WIDTH: f32 = 420.0;
const CELL_SIZE: f32 = 32.0;
const INFO_WIDTH: f32 = 150.0;
const INFO_ICON_SIZE: f32 = 48.0;
/// Fixed height of the grid + info-panel row, so the window never grows with
/// content — both panes scroll internally instead.
const PANE_HEIGHT: f32 = 300.0;

/// Spawn the whole window as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        InventoryWindowRoot
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
        Children [
            titlebar::<InventoryTitlebar, InventoryWindowRoot>("bag", "Inventory"),
            body_container::<InventoryWindowBody>(UiRect {
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(10.0),
                bottom: Val::Px(14.0),
            }),
        ]
    }
}

// ---------------------------------------------------------------------------
// Body: tab strip + (grid | info panel). Projected from live state; `bsn!`
// scenes own their data, so every view-model is prepared as owned values before
// entering a `bsn!` block.
// ---------------------------------------------------------------------------

/// One grid cell's owned view-model.
pub(crate) struct CellView {
    index: u16,
    icon: Option<String>,
    amount: u16,
    refine: u8,
    selected: bool,
}

/// The selected item's owned view-model for the info panel.
pub(crate) struct InfoView {
    icon: Option<String>,
    name: String,
    type_label: String,
    amount: u16,
    refine: u8,
    cards: Vec<bool>,
    description: Vec<String>,
}

fn icon_path(item_db: Option<&ItemDb>, item: &Item) -> Option<String> {
    item_db
        .and_then(|db| db.icon_resource(item.item_id, item.identified))
        .map(item_icon_path)
}

/// The active tab's cells, in inventory order.
pub(crate) fn cell_views(
    inventory: &Inventory,
    category: ItemCategory,
    item_db: Option<&ItemDb>,
    selected: Option<u16>,
) -> Vec<CellView> {
    items_for_tab(inventory, category)
        .into_iter()
        .map(|item| CellView {
            index: item.index,
            icon: icon_path(item_db, item),
            amount: item.amount,
            refine: item.refine,
            selected: selected == Some(item.index),
        })
        .collect()
}

/// One `true`/`false` per card slot the item type has, `true` where the slot is
/// filled — mirrors the shop window's `sell_cards` recipe. Empty when the item
/// type has no sockets.
fn card_slots(item: &Item, item_db: Option<&ItemDb>) -> Vec<bool> {
    let slots = item_db
        .and_then(|db| db.slot_count(item.item_id))
        .unwrap_or(0);
    (0..slots)
        .map(|slot| item.cards.get(slot as usize).copied().unwrap_or(0) != 0)
        .collect()
}

pub(crate) fn info_view(item: &Item, item_db: Option<&ItemDb>) -> InfoView {
    InfoView {
        icon: icon_path(item_db, item),
        name: item_db
            .and_then(|db| db.name(item.item_id, item.identified))
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{}", item.item_id)),
        type_label: item.type_label().to_string(),
        amount: item.amount,
        refine: item.refine,
        cards: card_slots(item, item_db),
        description: item_db
            .and_then(|db| db.description(item.item_id, item.identified))
            .map(|lines| lines.to_vec())
            .unwrap_or_default(),
    }
}

/// The whole swappable body: tab strip over the grid + info-panel row.
pub fn body(inventory: &Inventory, ui: &InventoryUi, item_db: Option<&ItemDb>) -> impl Scene {
    let counts = TABS.map(|(category, _, _)| tab_count(inventory, category));
    let cells = cell_views(inventory, ui.tab, item_db, ui.selected);
    let info = ui
        .selected
        .and_then(|index| inventory.get(index))
        .map(|item| info_view(item, item_db));

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [ tab_strip(ui.tab, counts), content_row(cells, info) ]
    }
}

fn tab_strip(active: ItemCategory, counts: [usize; 3]) -> impl Scene {
    let buttons: Vec<_> = TABS
        .iter()
        .zip(counts)
        .map(|((category, label, icon), count)| {
            tab_button(*category, label, icon, *category == active, count)
        })
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(6), padding: {UiRect::vertical(px(4))} }
        ignore_picking()
        Children [ {buttons} ]
    }
}

fn tab_button(
    category: ItemCategory,
    label: &'static str,
    icon: &'static str,
    active: bool,
    count: usize,
) -> impl Scene {
    let bg = if active { theme::EMERALD } else { theme::FIELD };
    bsn! {
        template_value(InventoryTab(category))
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(5),
            height: px(28),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(bg)
        Pickable
        on(on_tab_click)
        Children [
            glyph_icon(icon, 14.0, theme::TEXT_DIM),
            chrome_text(label.to_string(), 12.0, theme::TEXT_DIM),
            chrome_text(count.to_string(), 11.0, theme::TEXT_FAINT),
        ]
    }
}

fn content_row(cells: Vec<CellView>, info: Option<InfoView>) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(12), height: px(PANE_HEIGHT) }
        ignore_picking()
        Children [ grid_pane(cells), info_panel(info) ]
    }
}

/// The bordered item grid: a fixed-height, wheel-scrollable viewport of wrapped
/// cells with a draggable [`FeathersScrollbar`] pinned to the right. The `#grid`
/// id wires the scrollbar to the viewport whose `ScrollPosition` it drives.
fn grid_pane(cells: Vec<CellView>) -> impl Scene {
    let empty = cells.is_empty();
    let items: Vec<_> = cells.into_iter().map(cell).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("No items.".to_string())));
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.18)})
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [
            (
                #grid
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FlexStart,
                    column_gap: px(6),
                    row_gap: px(6),
                    padding: {UiRect { left: Val::Px(8.0), right: Val::Px(13.0), top: Val::Px(8.0), bottom: Val::Px(8.0) }},
                }
                ScrollArea
                Pickable
                Children [ {items}, {empty_msg} ]
            ),
            @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
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

/// One grid cell: a bordered icon well carrying the item's inventory index, with
/// the amount and refine badges baked in. Selection highlight is baked at build
/// time, so no separate highlight system is needed.
fn cell(view: CellView) -> impl Scene {
    let (bg, border) = if view.selected {
        (theme::EMERALD_INK, theme::EMERALD)
    } else {
        (theme::FIELD, theme::GOLD_FAINT)
    };
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let amount = (view.amount > 1).then(|| EntityScene(amount_badge(view.amount.to_string())));
    let refine = (view.refine > 0).then(|| EntityScene(refine_badge(format!("+{}", view.refine))));
    bsn! {
        template_value(InventoryCell { index: view.index })
        Node {
            width: px(CELL_SIZE),
            height: px(CELL_SIZE),
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        Children [ {icon}, {amount}, {refine} ]
    }
}

/// A contained item icon filling its cell.
fn cell_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node {
            position_type: PositionType::Absolute,
            left: px(0), top: px(0), right: px(0), bottom: px(0),
            width: percent(100),
            height: percent(100),
        }
        ignore_picking()
    }
}

/// The stack-amount badge, pinned to the cell's bottom-right.
fn amount_badge(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::TEXT)
        Node { position_type: PositionType::Absolute, right: px(1), bottom: px(0) }
        ignore_picking()
    }
}

/// The refine badge, pinned to the cell's top-left.
fn refine_badge(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::GOLD)
        Node { position_type: PositionType::Absolute, left: px(1), top: px(0) }
        ignore_picking()
    }
}

/// The selection info panel: a fixed-width, fixed-height bordered box whose
/// content scrolls internally (long descriptions never grow the window). The
/// `#info` id wires the scrollbar to the scrollable content viewport.
fn info_panel(info: Option<InfoView>) -> impl Scene {
    let empty = info.is_none();
    let content = info.map(|view| EntityScene(info_content(view)));
    let empty_msg = empty.then(|| EntityScene(muted_text("Select an item".to_string())));
    bsn! {
        Node {
            width: px(INFO_WIDTH),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [
            (
                #info
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    padding: {UiRect { left: Val::Px(10.0), right: Val::Px(13.0), top: Val::Px(10.0), bottom: Val::Px(10.0) }},
                }
                ScrollArea
                Pickable
                Children [ {content}, {empty_msg} ]
            ),
            @FeathersScrollbar { @target: #info, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(2),
                top: px(4),
                bottom: px(4),
                width: px(5),
            }
        ]
    }
}

fn info_content(view: InfoView) -> impl Scene {
    let icon = view.icon.map(|path| EntityScene(info_icon(path)));
    let refine = (view.refine > 0)
        .then(|| EntityScene(meta_row("Refine".to_string(), format!("+{}", view.refine))));
    let cards = (!view.cards.is_empty()).then(|| EntityScene(card_row(view.cards)));
    let description: Vec<_> = view.description.into_iter().map(colored_line).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [
            {icon},
            name_text(view.name),
            chrome_text(view.type_label, 11.0, theme::TEXT_DIM),
            meta_row("Quantity".to_string(), view.amount.to_string()),
            {refine},
            {cards},
            {description},
        ]
    }
}

fn info_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node { width: px(INFO_ICON_SIZE), height: px(INFO_ICON_SIZE) }
        ignore_picking()
    }
}

fn name_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(theme::TEXT)
        ignore_picking()
    }
}

/// A `label : value` row (info panel meta).
fn meta_row(label: String, value: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            chrome_text(label, 11.0, theme::TEXT_DIM),
            chrome_text(value, 11.5, theme::TEXT),
        ]
    }
}

/// One "◆" pip per card slot, emerald when filled and faint when empty.
fn card_row(cards: Vec<bool>) -> impl Scene {
    let pips: Vec<_> = cards.into_iter().map(card_pip).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(4) }
        ignore_picking()
        Children [ {pips} ]
    }
}

fn card_pip(filled: bool) -> impl Scene {
    let color = if filled {
        theme::EMERALD
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        Text({"\u{25C6}".to_string()})
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        TextColor(color)
        ignore_picking()
    }
}

/// A description line, split into `^RRGGBB`-colored runs: a `Text` root holding the
/// first run plus a `TextSpan` child per following run.
fn colored_line(text: String) -> impl Scene {
    let mut runs = parse_color_codes(&text, theme::TEXT_DIM).into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((theme::TEXT_DIM, String::new()));
    let spans: Vec<_> = runs.map(|(color, seg)| colored_span(color, seg)).collect();
    bsn! {
        Text(first_text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        TextColor(first_color)
        ignore_picking()
        Children [ {spans} ]
    }
}

fn colored_span(color: Color, text: String) -> impl Scene {
    bsn! {
        TextSpan(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        TextColor(color)
    }
}

fn muted_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{ItemData, ItemInfo};

    fn item(index: u16, item_type: u8) -> Item {
        Item {
            index,
            item_type,
            amount: 1,
            ..Default::default()
        }
    }

    fn mixed_inventory() -> Inventory {
        let mut inv = Inventory::default();
        inv.upsert(item(2, 0));
        inv.upsert(item(3, 2));
        inv.upsert(item(4, 5));
        inv
    }

    #[test]
    fn cell_views_projects_active_tab_only() {
        let inv = mixed_inventory();
        let use_cells = cell_views(&inv, ItemCategory::Use, None, Some(2));
        assert_eq!(use_cells.len(), 2);
        assert!(use_cells
            .iter()
            .any(|cell| cell.index == 2 && cell.selected));
        assert!(use_cells.iter().all(|cell| cell.icon.is_none()));

        let equip_cells = cell_views(&inv, ItemCategory::Equip, None, None);
        assert_eq!(equip_cells.len(), 1);
        assert_eq!(equip_cells[0].index, 4);
    }

    fn potion_db() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                identified_description: vec!["Restores 45 HP.".to_string()],
                slot_count: 0,
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn info_view_resolves_name_and_description() {
        let db = potion_db();
        let potion = Item {
            index: 2,
            item_id: 501,
            item_type: 0,
            amount: 5,
            identified: true,
            ..Default::default()
        };
        let view = info_view(&potion, Some(&db));
        assert_eq!(view.name, "Red Potion");
        assert_eq!(view.amount, 5);
        assert_eq!(view.description, vec!["Restores 45 HP.".to_string()]);
        assert!(view.icon.is_some());
    }

    #[test]
    fn info_view_falls_back_to_nameid_without_db() {
        let potion = Item {
            index: 2,
            item_id: 501,
            ..Default::default()
        };
        let view = info_view(&potion, None);
        assert_eq!(view.name, "#501");
        assert!(view.description.is_empty());
    }
}
