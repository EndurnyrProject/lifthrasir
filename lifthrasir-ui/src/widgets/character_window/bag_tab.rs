//! The Console's Bag tab: a faithful port of the inventory window's body —
//! Use/Etc/Equip tab strip, a fixed-height scrollable item grid, and the selection
//! info panel — projected into the shell's [`BagTabBody`] container instead of a
//! standalone window.
//!
//! This file is deliberately self-contained: it defines its OWN UI-only types
//! ([`BagUi`], [`BagCell`], [`BagTab`], [`LastBagClick`]) rather than importing the
//! old `inventory_window`'s, so `inventory_window` can be deleted wholesale in the
//! integration task with zero dangling references. It reuses only the shared DOMAIN
//! types + messages (`Inventory`, `Item`, `ItemCategory`, `ItemDb`, `item_icon_path`,
//! `HotbarDrag`/`HotbarSlot`, `Use/Equip/UnequipItemRequested`) and the chrome/theme
//! helpers.

use std::time::Duration;

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::equipment::{EquipItemRequested, UnequipItemRequested};
use game_engine::domain::hotbar::HotbarSlot;
use game_engine::domain::inventory::{Inventory, Item, ItemCategory, UseItemRequested};
use game_engine::infrastructure::item::ItemDb;

use crate::rich_text::parse_color_codes;
use crate::theme;
use crate::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use crate::widgets::hotbar::HotbarDrag;

use super::BagTabBody;

const CELL_SIZE: f32 = 32.0;
const INFO_WIDTH: f32 = 150.0;
const INFO_ICON_SIZE: f32 = 48.0;
/// Fixed height of the grid + info-panel row, so the tab never grows with content —
/// both panes scroll internally instead.
const PANE_HEIGHT: f32 = 300.0;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Tab table: category, caption, and glyph-icon name, in strip order.
const TABS: [(ItemCategory, &str, &str); 3] = [
    (ItemCategory::Use, "Use", "flask"),
    (ItemCategory::Etc, "Etc", "cube"),
    (ItemCategory::Equip, "Equip", "shield"),
];

/// Active tab + selected item index. Default tab `Use`, no selection.
#[derive(Resource, Default)]
pub struct BagUi {
    pub tab: ItemCategory,
    pub selected: Option<u16>,
}

/// Last cell click, for double-click detection (own copy; see module docs).
#[derive(Resource, Default)]
pub struct LastBagClick {
    index: u16,
    at: Duration,
}

/// Marks a tab button with the category it selects.
#[derive(Component, Clone, Copy, Default)]
pub struct BagTab(pub ItemCategory);

/// Marks a grid cell with the inventory index of the item it shows.
#[derive(Component, Clone, Copy, Default)]
pub struct BagCell {
    pub index: u16,
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested).
// ---------------------------------------------------------------------------

/// The bag items in `category` — worn items live on the Character tab, not the bag.
fn items_for_tab(inventory: &Inventory, category: ItemCategory) -> Vec<&Item> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped() && item.category() == category)
        .collect()
}

fn tab_count(inventory: &Inventory, category: ItemCategory) -> usize {
    items_for_tab(inventory, category).len()
}

fn is_double_click(last: &LastBagClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

/// The action a cell interaction resolves to. Single clicks only select (no action).
#[derive(Debug, PartialEq, Eq)]
enum CellAction {
    Use(u32),
    Equip(u16),
    Unequip(u16),
}

/// Decides what a click on `item` at `index` does: a double-click uses a Use item and
/// equips/unequips an Equip item; anything else (single click, Etc item) is `None`.
fn cell_action(item: &Item, index: u16, is_double: bool) -> Option<CellAction> {
    if !is_double {
        return None;
    }
    match item.category() {
        ItemCategory::Use => Some(CellAction::Use(index as u32)),
        ItemCategory::Equip if item.is_equipped() => Some(CellAction::Unequip(index)),
        ItemCategory::Equip => Some(CellAction::Equip(index)),
        ItemCategory::Etc => None,
    }
}

/// Resolves a cell's inventory index to the stable `item_id` the hotbar stores.
fn bag_item_id(inventory: &Inventory, index: u16) -> Option<u32> {
    inventory.get(index).map(|item| item.item_id)
}

// ---------------------------------------------------------------------------
// Rebuild system.
// ---------------------------------------------------------------------------

/// Rebuilds the [`BagTabBody`]'s children on every `Inventory`/[`BagUi`] change, and
/// once when the body container is first spawned (the shell mounts it deferred, after
/// the resources' first-frame "changed" tick has passed). Despawns the old children
/// and respawns the projected body scene. Mirrors `inventory_window::rebuild_body`.
pub fn rebuild_bag_body(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<BagUi>,
    item_db: Option<Res<ItemDb>>,
    bodies: Query<(Entity, Option<&Children>, Ref<BagTabBody>)>,
) {
    let Ok((body_entity, children, body_ref)) = bodies.single() else {
        return;
    };
    if !inventory.is_changed() && !ui.is_changed() && !body_ref.is_added() {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(body(&inventory, &ui, item_db.as_deref()))
        .insert(ChildOf(body_entity));
}

/// Reset to the default tab/selection when leaving the game.
pub fn reset(mut ui: ResMut<BagUi>) {
    *ui = BagUi::default();
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// Tab click: set the active tab and clear the current selection.
fn on_tab_click(click: On<Pointer<Click>>, tabs: Query<&BagTab>, mut ui: ResMut<BagUi>) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = tab.0;
    ui.selected = None;
}

/// Dragging a bag cell arms the hotbar with that item's stable `item_id` so a slot
/// drop assigns it. A plain click still goes through `on_cell_click` since
/// `bevy_picking` only emits `DragStart` after a press-and-move.
fn on_cell_drag_start(
    drag: On<Pointer<DragStart>>,
    cells: Query<&BagCell>,
    inventory: Res<Inventory>,
    mut hotbar_drag: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    let Some(item_id) = bag_item_id(&inventory, cell.index) else {
        return;
    };
    hotbar_drag.payload = Some(HotbarSlot::Item(item_id));
}

/// Cell click: select the item; a double-click resolves to Use/Equip/Unequip via
/// [`cell_action`].
#[allow(clippy::too_many_arguments)]
fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&BagCell>,
    mut ui: ResMut<BagUi>,
    time: Res<Time>,
    mut last: ResMut<LastBagClick>,
    inventory: Res<Inventory>,
    mut use_writer: MessageWriter<UseItemRequested>,
    mut equip_writer: MessageWriter<EquipItemRequested>,
    mut unequip_writer: MessageWriter<UnequipItemRequested>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    ui.selected = Some(cell.index);
    let now = time.elapsed();
    if let Some(item) = inventory.get(cell.index) {
        let is_double = is_double_click(&last, cell.index, now);
        match cell_action(item, cell.index, is_double) {
            Some(CellAction::Use(index)) => {
                use_writer.write(UseItemRequested { index });
            }
            Some(CellAction::Equip(index)) => {
                equip_writer.write(EquipItemRequested { index });
            }
            Some(CellAction::Unequip(index)) => {
                unequip_writer.write(UnequipItemRequested { index });
            }
            None => {}
        }
    }
    *last = LastBagClick {
        index: cell.index,
        at: now,
    };
}

// ---------------------------------------------------------------------------
// Body: tab strip + (grid | info panel). Projected from live state; `bsn!` scenes
// own their data, so every view-model is prepared as owned values before entering a
// `bsn!` block.
// ---------------------------------------------------------------------------

/// One grid cell's owned view-model.
struct CellView {
    index: u16,
    icon: Option<String>,
    amount: u16,
    refine: u8,
    selected: bool,
}

/// The selected item's owned view-model for the info panel.
struct InfoView {
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

fn cell_views(
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
/// filled. Empty when the item type has no sockets.
fn card_slots(item: &Item, item_db: Option<&ItemDb>) -> Vec<bool> {
    let slots = item_db
        .and_then(|db| db.slot_count(item.item_id))
        .unwrap_or(0);
    (0..slots)
        .map(|slot| item.cards.get(slot as usize).copied().unwrap_or(0) != 0)
        .collect()
}

fn info_view(item: &Item, item_db: Option<&ItemDb>) -> InfoView {
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
fn body(inventory: &Inventory, ui: &BagUi, item_db: Option<&ItemDb>) -> impl Scene {
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
        template_value(BagTab(category))
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

/// The bordered item grid: a fixed-height, wheel-scrollable viewport of wrapped cells
/// with a draggable [`FeathersScrollbar`] pinned to the right. The `#grid` id wires the
/// scrollbar to the viewport whose `ScrollPosition` it drives.
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

/// One grid cell: a bordered icon well carrying the item's inventory index, with the
/// amount and refine badges baked in. Selection highlight is baked at build time.
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
        template_value(BagCell { index: view.index })
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

/// The selection info panel: a fixed-width, fixed-height bordered box whose content
/// scrolls internally. The `#info` id wires the scrollbar to the scrollable viewport.
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
    use bevy::scene::ScenePlugin;

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
        inv.upsert(item(5, 3));
        inv
    }

    #[test]
    fn items_for_tab_filters_by_category() {
        let inv = mixed_inventory();
        let use_indices: Vec<u16> = items_for_tab(&inv, ItemCategory::Use)
            .iter()
            .map(|i| i.index)
            .collect();
        let equip_indices: Vec<u16> = items_for_tab(&inv, ItemCategory::Equip)
            .iter()
            .map(|i| i.index)
            .collect();
        assert_eq!(use_indices, vec![2, 3]);
        assert_eq!(equip_indices, vec![4]);
    }

    #[test]
    fn double_click_use_item_resolves_to_use() {
        let potion = Item {
            index: 5,
            item_type: 0,
            ..Default::default()
        };
        assert_eq!(cell_action(&potion, 5, true), Some(CellAction::Use(5)));
    }

    #[test]
    fn double_click_unequipped_equip_item_resolves_to_equip() {
        let sword = Item {
            index: 4,
            item_type: 5,
            ..Default::default()
        };
        assert!(!sword.is_equipped());
        assert_eq!(cell_action(&sword, 4, true), Some(CellAction::Equip(4)));
    }

    #[test]
    fn double_click_equipped_item_resolves_to_unequip() {
        let worn = Item {
            index: 4,
            item_type: 5,
            wear_state: 0x0002,
            ..Default::default()
        };
        assert!(worn.is_equipped());
        assert_eq!(cell_action(&worn, 4, true), Some(CellAction::Unequip(4)));
    }

    #[test]
    fn single_click_resolves_to_no_action() {
        let potion = Item {
            index: 5,
            item_type: 0,
            ..Default::default()
        };
        assert_eq!(cell_action(&potion, 5, false), None);
    }

    #[test]
    fn double_click_etc_item_resolves_to_no_action() {
        let etc = Item {
            index: 3,
            item_type: 3,
            ..Default::default()
        };
        assert_eq!(cell_action(&etc, 3, true), None);
    }

    #[test]
    fn is_double_click_respects_index_and_window() {
        let last = LastBagClick {
            index: 5,
            at: Duration::from_millis(100),
        };
        assert!(is_double_click(&last, 5, Duration::from_millis(350)));
        assert!(!is_double_click(&last, 6, Duration::from_millis(200)));
        assert!(!is_double_click(&last, 5, Duration::from_millis(500)));
    }

    #[test]
    fn bag_item_id_resolves_present_and_absent() {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 7,
            item_id: 501,
            amount: 3,
            ..Default::default()
        });
        assert_eq!(bag_item_id(&inv, 7), Some(501));
        assert_eq!(bag_item_id(&inv, 99), None);
    }

    fn bag_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<BagUi>();
        app.add_systems(Update, rebuild_bag_body);
        app
    }

    fn bag_cell_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<BagCell>>()
            .iter(world)
            .count()
    }

    #[test]
    fn rebuild_renders_one_cell_per_active_category_item() {
        let mut app = bag_app();
        app.insert_resource(mixed_inventory());
        app.world_mut().spawn(BagTabBody);

        app.update();
        assert_eq!(
            bag_cell_count(&mut app),
            2,
            "Use tab shows its two items by default"
        );

        app.world_mut().resource_mut::<BagUi>().tab = ItemCategory::Equip;
        app.update();
        assert_eq!(
            bag_cell_count(&mut app),
            1,
            "switching to Equip shows its single item"
        );
    }
}
