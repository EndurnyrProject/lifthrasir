//! Inventory window: a read-and-navigate view of the `Inventory` resource.
//!
//! Builds the chrome (titlebar, Use/Etc/Equip tab strip) and rebuilds the item
//! grid on data/tab/selection change — each cell shows the item icon plus qty and
//! refine badges and is clickable to select. The info panel is filled by a later
//! task; `InventoryUi` holds the active tab and selection.

use std::time::Duration;

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::equipment::{EquipItemRequested, UnequipItemRequested};
use game_engine::domain::hotbar::HotbarSlot;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::inventory::{Inventory, Item, ItemCategory, UseItemRequested};

use crate::widgets::hotbar::HotbarDrag;
use game_engine::infrastructure::item::ItemDb;
use leafwing_input_manager::prelude::ActionState;

use crate::rich_text::spawn_colored_text;
use crate::theme;
use crate::widgets::draggable::make_draggable;

/// All items in the active tab's category (worn or not).
fn items_for_tab(inventory: &Inventory, category: ItemCategory) -> Vec<&Item> {
    inventory
        .iter()
        .filter(|item| item.category() == category)
        .collect()
}

fn tab_count(inventory: &Inventory, category: ItemCategory) -> usize {
    items_for_tab(inventory, category).len()
}

/// Marks the inventory-window root so the toggle/close systems can flip its visibility.
#[derive(Component)]
pub struct InventoryWindowRoot;

/// Marks the (initially empty) item-grid container; filled by a later task.
#[derive(Component)]
pub struct InventoryGrid;

/// Marks the (initially empty) right-side info panel; filled by a later task.
#[derive(Component)]
pub struct InventoryInfoPanel;

/// Marks a tab button with the category it selects.
#[derive(Component, Clone, Copy)]
pub struct InventoryTab(pub ItemCategory);

/// Marks the per-tab count text node; filled by a later task.
#[derive(Component, Clone, Copy)]
pub struct InventoryTabCount(pub ItemCategory);

/// Marks a grid cell with the inventory index of the item it shows.
#[derive(Component, Clone, Copy)]
pub struct InventoryCell {
    pub index: u16,
}

const CELL_SIZE: f32 = 32.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Resource, Default)]
struct LastCellClick {
    index: u16,
    at: Duration,
}

fn is_double_click(last: &LastCellClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

fn is_use_double_click(
    last: &LastCellClick,
    index: u16,
    now: Duration,
    category: ItemCategory,
) -> bool {
    category == ItemCategory::Use && is_double_click(last, index, now)
}

fn is_equip_double_click(
    last: &LastCellClick,
    index: u16,
    now: Duration,
    category: ItemCategory,
) -> bool {
    category == ItemCategory::Equip && is_double_click(last, index, now)
}

const TABS: [(ItemCategory, &str, &str); 3] = [
    (ItemCategory::Use, "Use", "flask"),
    (ItemCategory::Etc, "Etc", "cube"),
    (ItemCategory::Equip, "Equip", "shield"),
];

/// Active tab + selected item index. Default tab `Use`, no selection.
#[derive(Resource, Default)]
pub struct InventoryUi {
    pub tab: ItemCategory,
    pub selected: Option<u16>,
}

pub struct InventoryWindowPlugin;

impl Plugin for InventoryWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUi>();
        app.init_resource::<LastCellClick>();
        app.add_systems(
            Update,
            toggle_inventory_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                update_tab_highlight,
                rebuild_grid,
                rebuild_info_panel,
                update_tab_counts,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Builds the inventory-window shell under `parent`: a glass panel with a titlebar
/// (rune + "Inventory" + close), the Use/Etc/Equip tab strip, and a body row with
/// the empty grid on the left and the empty info panel on the right. Hidden by
/// default and draggable by its titlebar.
pub fn spawn_inventory_window(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let root = commands
        .spawn((
            InventoryWindowRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(320.0),
                top: Val::Px(110.0),
                width: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Visibility::Hidden,
            Pickable::default(),
            ChildOf(parent),
        ))
        .id();

    spawn_titlebar(commands, asset_server, root, &font_title);
    spawn_tab_strip(commands, asset_server, root, &font_body);
    spawn_body(commands, root, &font_body);
}

fn spawn_titlebar(
    commands: &mut Commands,
    asset_server: &AssetServer,
    root: Entity,
    font_title: &Handle<Font>,
) {
    let titlebar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(11.0)),
                border: UiRect {
                    bottom: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::default(),
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        theme::icon(asset_server, "bag", 16.0, theme::GOLD),
        ChildOf(titlebar),
    ));
    commands.spawn((
        theme::label("Inventory", font_title.clone(), 15.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(titlebar),
    ));

    let close = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Pickable::default(),
            ChildOf(titlebar),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "close", 13.0, theme::TEXT_DIM),
        ChildOf(close),
    ));
    commands.entity(close).observe(
        |_: On<Pointer<Click>>, mut window: Query<&mut Visibility, With<InventoryWindowRoot>>| {
            if let Ok(mut visibility) = window.single_mut() {
                *visibility = Visibility::Hidden;
            }
        },
    );

    make_draggable(commands, titlebar, root);
}

/// Use / Etc / Equip tab buttons, each with a label and an (empty) count text.
fn spawn_tab_strip(
    commands: &mut Commands,
    asset_server: &AssetServer,
    root: Entity,
    font: &Handle<Font>,
) {
    let strip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    for (category, label, icon) in TABS {
        spawn_tab(commands, asset_server, strip, category, label, icon, font);
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_tab(
    commands: &mut Commands,
    asset_server: &AssetServer,
    strip: Entity,
    category: ItemCategory,
    label: &str,
    icon: &str,
    font: &Handle<Font>,
) {
    let tab = commands
        .spawn((
            InventoryTab(category),
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                height: Val::Px(28.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Pickable::default(),
            ChildOf(strip),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, icon, 14.0, theme::TEXT_DIM),
        ChildOf(tab),
    ));
    commands.spawn((
        theme::label(label, font.clone(), 12.0, theme::TEXT_DIM),
        ChildOf(tab),
    ));
    commands.spawn((
        theme::label("", font.clone(), 11.0, theme::TEXT_FAINT),
        InventoryTabCount(category),
        ChildOf(tab),
    ));
    commands.entity(tab).observe(on_tab_click);
}

/// Tab click: set the active tab and clear the current selection.
fn on_tab_click(
    click: On<Pointer<Click>>,
    tabs: Query<&InventoryTab>,
    mut ui: ResMut<InventoryUi>,
) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = tab.0;
    ui.selected = None;
}

/// Body row: the empty item grid on the left, the empty info panel on the right.
fn spawn_body(commands: &mut Commands, root: Entity, _font: &Handle<Font>) {
    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                padding: UiRect {
                    left: Val::Px(14.0),
                    right: Val::Px(14.0),
                    bottom: Val::Px(14.0),
                    top: Val::ZERO,
                },
                min_height: Val::Px(220.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        InventoryGrid,
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(body),
    ));

    commands.spawn((
        InventoryInfoPanel,
        Node {
            width: Val::Px(150.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(theme::FIELD),
        BorderColor::all(theme::GOLD_FAINT),
        Pickable::IGNORE,
        ChildOf(body),
    ));
}

/// Reflects the active tab into the tab-button background, writing only on change.
fn update_tab_highlight(
    ui: Res<InventoryUi>,
    mut tabs: Query<(&mut BackgroundColor, &InventoryTab)>,
) {
    if !ui.is_changed() {
        return;
    }
    for (mut bg, tab) in &mut tabs {
        let color = if tab.0 == ui.tab {
            theme::EMERALD
        } else {
            theme::FIELD
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Rebuilds the grid on inventory, tab, or selection change: despawns the
/// existing cells and spawns one per item in the active tab. Selection highlight
/// is baked in at spawn, so no separate highlight system is needed.
fn rebuild_grid(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<InventoryUi>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    grid: Query<(Entity, Option<&Children>), With<InventoryGrid>>,
) {
    if !inventory.is_changed() && !ui.is_changed() {
        return;
    }
    let Ok((grid, children)) = grid.single() else {
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let font = asset_server.load(theme::FONT_BODY);
    let items = items_for_tab(&inventory, ui.tab);
    if items.is_empty() {
        commands.spawn((
            theme::label("No items.", font.clone(), 12.0, theme::TEXT_FAINT),
            ChildOf(grid),
        ));
        return;
    }

    let db = item_db.as_deref();
    for item in items {
        spawn_cell(
            &mut commands,
            grid,
            item,
            ui.selected,
            db,
            &asset_server,
            &font,
        );
    }
}

fn spawn_cell(
    commands: &mut Commands,
    grid: Entity,
    item: &Item,
    selected: Option<u16>,
    item_db: Option<&ItemDb>,
    asset_server: &AssetServer,
    font: &Handle<Font>,
) {
    let is_selected = selected == Some(item.index);
    let (bg, border) = if is_selected {
        (theme::EMERALD_INK, theme::EMERALD)
    } else {
        (theme::FIELD, theme::GOLD_FAINT)
    };

    let cell = commands
        .spawn((
            InventoryCell { index: item.index },
            Node {
                width: Val::Px(CELL_SIZE),
                height: Val::Px(CELL_SIZE),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
            Pickable::default(),
            ChildOf(grid),
        ))
        .id();

    if let Some(resource) = item_db.and_then(|db| db.icon_resource(item.item_id, item.identified)) {
        commands.spawn((
            ImageNode::new(asset_server.load(item_icon_path(resource))),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(cell),
        ));
    }

    if item.amount > 1 {
        commands.spawn((
            theme::label(item.amount.to_string(), font.clone(), 9.0, theme::TEXT),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(1.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            ChildOf(cell),
        ));
    }

    if item.refine > 0 {
        commands.spawn((
            theme::label(format!("+{}", item.refine), font.clone(), 9.0, theme::GOLD),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(1.0),
                top: Val::Px(0.0),
                ..default()
            },
            ChildOf(cell),
        ));
    }

    commands.entity(cell).observe(on_cell_click);
    commands.entity(cell).observe(on_cell_drag_start);
}

/// Resolves a cell's inventory index to the stable `item_id` the hotbar stores.
fn inventory_item_id(inventory: &Inventory, index: u16) -> Option<u32> {
    inventory.get(index).map(|item| item.item_id)
}

/// Dragging an inventory cell arms the hotbar with that item's stable `item_id`
/// so a slot drop assigns it. A plain click still goes through `on_cell_click`
/// since `bevy_picking` only emits `DragStart` after a press-and-move.
fn on_cell_drag_start(
    drag: On<Pointer<DragStart>>,
    cells: Query<&InventoryCell>,
    inventory: Res<Inventory>,
    mut hotbar_drag: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    let Some(item_id) = inventory_item_id(&inventory, cell.index) else {
        return;
    };
    hotbar_drag.payload = Some(HotbarSlot::Item(item_id));
}

/// Cell click: select the item; double-click on a Use item emits `UseItemRequested`,
/// double-click on an Equip item emits `EquipItemRequested`/`UnequipItemRequested`
/// depending on whether it is currently worn.
#[allow(clippy::too_many_arguments)]
fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&InventoryCell>,
    mut ui: ResMut<InventoryUi>,
    time: Res<Time>,
    mut last: ResMut<LastCellClick>,
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
        let category = item.category();
        let equip_dc = is_equip_double_click(&last, cell.index, now, category);
        if is_use_double_click(&last, cell.index, now, category) {
            use_writer.write(UseItemRequested {
                index: cell.index as u32,
            });
        } else if equip_dc && item.is_equipped() {
            unequip_writer.write(UnequipItemRequested { index: cell.index });
        } else if equip_dc {
            equip_writer.write(EquipItemRequested { index: cell.index });
        }
    }
    *last = LastCellClick {
        index: cell.index,
        at: now,
    };
}

/// Writes each tab's item count into its count label on inventory change.
fn update_tab_counts(
    inventory: Res<Inventory>,
    mut counts: Query<(&mut Text, &InventoryTabCount)>,
) {
    if !inventory.is_changed() {
        return;
    }
    for (mut text, count) in &mut counts {
        let value = tab_count(&inventory, count.0).to_string();
        if text.0 != value {
            *text = Text::new(value);
        }
    }
}

const INFO_ICON_SIZE: f32 = 48.0;

/// Rebuilds the info panel on inventory or selection change: despawns the existing
/// children and, when `selected` resolves to an item, spawns its icon, name, type,
/// meta rows, card-slot pips, and description; otherwise spawns the empty placeholder.
fn rebuild_info_panel(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<InventoryUi>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    panel: Query<(Entity, Option<&Children>), With<InventoryInfoPanel>>,
) {
    if !inventory.is_changed() && !ui.is_changed() {
        return;
    }
    let Ok((panel, children)) = panel.single() else {
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let font = asset_server.load(theme::FONT_BODY);
    let Some(item) = ui.selected.and_then(|index| inventory.get(index)) else {
        commands.spawn((
            theme::label("Select an item", font.clone(), 12.0, theme::TEXT_FAINT),
            ChildOf(panel),
        ));
        return;
    };

    let db = item_db.as_deref();
    spawn_info_content(&mut commands, panel, item, db, &asset_server, &font);
}

fn spawn_info_content(
    commands: &mut Commands,
    panel: Entity,
    item: &Item,
    item_db: Option<&ItemDb>,
    asset_server: &AssetServer,
    font: &Handle<Font>,
) {
    if let Some(resource) = item_db.and_then(|db| db.icon_resource(item.item_id, item.identified)) {
        commands.spawn((
            ImageNode::new(asset_server.load(item_icon_path(resource))),
            Node {
                width: Val::Px(INFO_ICON_SIZE),
                height: Val::Px(INFO_ICON_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ));
    }

    let name = item_db
        .and_then(|db| db.name(item.item_id, item.identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", item.item_id));
    commands.spawn((
        theme::label(name, font.clone(), 13.0, theme::TEXT),
        ChildOf(panel),
    ));
    commands.spawn((
        theme::label(item.type_label(), font.clone(), 11.0, theme::TEXT_DIM),
        ChildOf(panel),
    ));

    spawn_meta_row(commands, panel, "Quantity", item.amount.to_string(), font);
    if item.refine > 0 {
        spawn_meta_row(commands, panel, "Refine", format!("+{}", item.refine), font);
    }

    spawn_card_slots(commands, panel, item, item_db, font);

    if let Some(lines) = item_db.and_then(|db| db.description(item.item_id, item.identified)) {
        for line in lines {
            spawn_colored_text(commands, panel, line, font.clone(), 11.0, theme::TEXT_DIM);
        }
    }
}

fn spawn_meta_row(
    commands: &mut Commands,
    panel: Entity,
    label: &str,
    value: String,
    font: &Handle<Font>,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        theme::label(label, font.clone(), 11.0, theme::TEXT_DIM),
        ChildOf(row),
    ));
    commands.spawn((
        theme::label(value, font.clone(), 11.5, theme::TEXT),
        ChildOf(row),
    ));
}

/// One pip per card slot, filled when the matching `cards[]` entry is set. Nothing
/// is spawned when the item has no slots.
fn spawn_card_slots(
    commands: &mut Commands,
    panel: Entity,
    item: &Item,
    item_db: Option<&ItemDb>,
    font: &Handle<Font>,
) {
    let slots = item_db
        .and_then(|db| db.slot_count(item.item_id))
        .unwrap_or(0);
    if slots == 0 {
        return;
    }

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    for slot in 0..slots {
        let filled = item.cards.get(slot as usize).copied().unwrap_or(0) != 0;
        let color = if filled {
            theme::EMERALD
        } else {
            theme::TEXT_FAINT
        };
        commands.spawn((
            theme::label("\u{25C6}", font.clone(), 11.0, color),
            ChildOf(row),
        ));
    }
}

/// Alt+E toggles the inventory window between hidden and visible.
fn toggle_inventory_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<InventoryWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Inventory) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

/// Reset to the default tab/selection when leaving the game.
fn reset(mut ui: ResMut<InventoryUi>) {
    *ui = InventoryUi::default();
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn double_click_same_use_cell_within_window_is_true() {
        let last = LastCellClick {
            index: 5,
            at: Duration::from_millis(100),
        };
        assert!(is_use_double_click(
            &last,
            5,
            Duration::from_millis(350),
            ItemCategory::Use,
        ));
    }

    #[test]
    fn single_click_different_index_is_false() {
        let last = LastCellClick {
            index: 5,
            at: Duration::from_millis(100),
        };
        assert!(!is_use_double_click(
            &last,
            6,
            Duration::from_millis(200),
            ItemCategory::Use,
        ));
    }

    #[test]
    fn first_click_no_prior_state_is_false() {
        let last = LastCellClick::default();
        assert!(!is_use_double_click(
            &last,
            3,
            Duration::from_millis(5000),
            ItemCategory::Use,
        ));
    }

    #[test]
    fn double_click_too_far_apart_is_false() {
        let last = LastCellClick {
            index: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_use_double_click(
            &last,
            2,
            Duration::from_millis(500),
            ItemCategory::Use,
        ));
    }

    #[test]
    fn double_click_equip_category_is_false() {
        let last = LastCellClick {
            index: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_use_double_click(
            &last,
            2,
            Duration::from_millis(200),
            ItemCategory::Equip,
        ));
    }

    #[test]
    fn double_click_same_equip_cell_within_window_is_true() {
        let last = LastCellClick {
            index: 5,
            at: Duration::from_millis(100),
        };
        assert!(is_equip_double_click(
            &last,
            5,
            Duration::from_millis(350),
            ItemCategory::Equip,
        ));
    }

    #[test]
    fn equip_double_click_use_category_is_false() {
        let last = LastCellClick {
            index: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_equip_double_click(
            &last,
            2,
            Duration::from_millis(200),
            ItemCategory::Use,
        ));
    }

    #[test]
    fn double_click_etc_category_is_false() {
        let last = LastCellClick {
            index: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_use_double_click(
            &last,
            2,
            Duration::from_millis(200),
            ItemCategory::Etc,
        ));
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
        let etc_indices: Vec<u16> = items_for_tab(&inv, ItemCategory::Etc)
            .iter()
            .map(|i| i.index)
            .collect();

        assert_eq!(use_indices, vec![2, 3]);
        assert_eq!(equip_indices, vec![4]);
        assert_eq!(etc_indices, vec![5]);
    }

    #[test]
    fn tab_count_matches_category_totals() {
        let inv = mixed_inventory();

        assert_eq!(tab_count(&inv, ItemCategory::Use), 2);
        assert_eq!(tab_count(&inv, ItemCategory::Equip), 1);
        assert_eq!(tab_count(&inv, ItemCategory::Etc), 1);
    }

    #[test]
    fn reset_restores_default_tab_and_selection() {
        let mut app = App::new();
        app.insert_resource(InventoryUi {
            tab: ItemCategory::Equip,
            selected: Some(7),
        });
        app.add_systems(Update, reset);
        app.update();

        let ui = app.world().resource::<InventoryUi>();
        assert_eq!(ui.tab, ItemCategory::Use);
        assert_eq!(ui.selected, None);
    }

    fn highlight_of(app: &App, e: Entity) -> Color {
        app.world().get::<BackgroundColor>(e).unwrap().0
    }

    #[test]
    fn highlight_tracks_active_tab() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InventoryUi>();

        let use_tab = app
            .world_mut()
            .spawn((
                BackgroundColor(theme::FIELD),
                InventoryTab(ItemCategory::Use),
            ))
            .id();
        let equip_tab = app
            .world_mut()
            .spawn((
                BackgroundColor(theme::FIELD),
                InventoryTab(ItemCategory::Equip),
            ))
            .id();

        app.add_systems(Update, update_tab_highlight);
        app.update();

        assert_eq!(highlight_of(&app, use_tab), theme::EMERALD);
        assert_eq!(highlight_of(&app, equip_tab), theme::FIELD);

        app.world_mut().resource_mut::<InventoryUi>().tab = ItemCategory::Equip;
        app.update();

        assert_eq!(highlight_of(&app, use_tab), theme::FIELD);
        assert_eq!(highlight_of(&app, equip_tab), theme::EMERALD);
    }

    fn child_count(app: &App, grid: Entity) -> usize {
        app.world()
            .get::<Children>(grid)
            .map(|c| c.len())
            .unwrap_or(0)
    }

    #[test]
    fn rebuild_grid_matches_active_tab_count() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.insert_resource(ItemDb::default());
        app.init_resource::<InventoryUi>();
        app.insert_resource(mixed_inventory());

        let grid = app.world_mut().spawn(InventoryGrid).id();

        app.add_systems(Update, rebuild_grid);
        app.update();

        assert_eq!(child_count(&app, grid), 2);

        app.world_mut().resource_mut::<InventoryUi>().tab = ItemCategory::Equip;
        app.update();

        assert_eq!(child_count(&app, grid), 1);
    }

    fn potion_db() -> ItemDb {
        use lifthrasir_data::{ItemData, ItemInfo};
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

    fn potion_item() -> Item {
        Item {
            index: 2,
            item_id: 501,
            item_type: 0,
            amount: 5,
            identified: true,
            ..Default::default()
        }
    }

    #[test]
    fn rebuild_info_panel_renders_content_then_placeholder() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.insert_resource(potion_db());
        let mut inventory = Inventory::default();
        inventory.upsert(potion_item());
        app.insert_resource(inventory);
        app.insert_resource(InventoryUi {
            tab: ItemCategory::Use,
            selected: Some(2),
        });

        let panel = app.world_mut().spawn(InventoryInfoPanel).id();

        app.add_systems(Update, rebuild_info_panel);
        app.update();

        assert!(child_count(&app, panel) > 1);
        let mut texts = app.world_mut().query::<&Text>();
        let rendered: Vec<String> = texts.iter(app.world()).map(|t| t.0.clone()).collect();
        assert!(
            rendered.iter().any(|t| t == "Red Potion"),
            "DB name should render: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|t| t == "Restores 45 HP."),
            "DB description should render: {rendered:?}"
        );

        app.world_mut().resource_mut::<InventoryUi>().selected = None;
        app.update();

        assert_eq!(child_count(&app, panel), 1);
    }

    #[test]
    fn inventory_item_id_resolves_present_and_absent() {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 7,
            item_id: 501,
            amount: 3,
            ..Default::default()
        });
        assert_eq!(inventory_item_id(&inv, 7), Some(501));
        assert_eq!(inventory_item_id(&inv, 99), None);
        assert_eq!(inventory_item_id(&Inventory::default(), 7), None);
    }
}
