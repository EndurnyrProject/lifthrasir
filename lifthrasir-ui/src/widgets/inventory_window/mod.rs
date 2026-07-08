//! Inventory window: a read-and-navigate view of the `Inventory` resource.
//!
//! The chrome (a draggable, Alt+E-toggled glass window with a titlebar and a
//! swappable body) is authored declaratively with `bsn!` in [`scene`]; Feathers
//! supplies the buttons and the scrollbars. The body (Use/Etc/Equip tab strip,
//! fixed-height scrollable item grid, and the selection info panel) is a pure
//! projection of `Inventory` + [`InventoryUi`], rebuilt by [`rebuild_body`] on
//! every change. The window is fixed-size: the grid and info panel scroll
//! internally instead of growing the window.

use std::time::Duration;

use bevy::prelude::*;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::equipment::{EquipItemRequested, UnequipItemRequested};
use game_engine::domain::hotbar::HotbarSlot;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::inventory::{Inventory, Item, ItemCategory, UseItemRequested};
use game_engine::infrastructure::item::ItemDb;
use leafwing_input_manager::prelude::ActionState;

use crate::theme::feathers_theme::install_norse_theme;
use crate::widgets::hotbar::HotbarDrag;

pub mod scene;

/// The bag items in the active tab's category. Worn items live in the equipment
/// window, not the bag, so they are excluded here.
pub(crate) fn items_for_tab(inventory: &Inventory, category: ItemCategory) -> Vec<&Item> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped() && item.category() == category)
        .collect()
}

pub(crate) fn tab_count(inventory: &Inventory, category: ItemCategory) -> usize {
    items_for_tab(inventory, category).len()
}

/// Marks the inventory-window root so the toggle/close/drag systems can find it.
#[derive(Component, Default, Clone)]
pub struct InventoryWindowRoot;

/// The swappable body region (tab strip, grid, info panel), rebuilt on every
/// `Inventory`/`InventoryUi` change.
#[derive(Component, Default, Clone)]
pub struct InventoryWindowBody;

/// The draggable titlebar; the drag observer only moves the window when the drag's
/// target is the titlebar itself, so dragging from the close button is inert.
#[derive(Component, Default, Clone)]
pub struct InventoryTitlebar;

/// Marks a tab button with the category it selects.
#[derive(Component, Clone, Copy, Default)]
pub struct InventoryTab(pub ItemCategory);

/// Marks a grid cell with the inventory index of the item it shows.
#[derive(Component, Clone, Copy, Default)]
pub struct InventoryCell {
    pub index: u16,
}

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

/// Tab table: category, caption, and glyph-icon name, in strip order.
pub(crate) const TABS: [(ItemCategory, &str, &str); 3] = [
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
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<InventoryUi>();
        app.init_resource::<LastCellClick>();
        app.add_systems(
            Update,
            toggle_inventory_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(Update, rebuild_body.run_if(in_state(GameState::InGame)));
        app.add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Spawn the hidden inventory window under `parent`. Delegates the BSN chrome to
/// [`scene::build`]; asset paths resolve inside the scene, so no `AssetServer` is
/// needed.
pub fn spawn_inventory_window(commands: &mut Commands, parent: Entity) {
    scene::build(commands, parent);
}

/// Rebuilds the swappable [`InventoryWindowBody`] on every `Inventory`/`InventoryUi`
/// change: despawns its children and respawns the projected body scene. Guarded by
/// change detection so it only runs on a real change (and on the first frame, when
/// the resources read as changed). A missing body entity (the frame before the
/// chrome's deferred spawn lands) skips silently; the next change retries.
fn rebuild_body(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<InventoryUi>,
    item_db: Option<Res<ItemDb>>,
    bodies: Query<(Entity, Option<&Children>), With<InventoryWindowBody>>,
) {
    if !inventory.is_changed() && !ui.is_changed() {
        return;
    }
    let Ok((body, children)) = bodies.single() else {
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::body(&inventory, &ui, item_db.as_deref()))
        .insert(ChildOf(body));
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
