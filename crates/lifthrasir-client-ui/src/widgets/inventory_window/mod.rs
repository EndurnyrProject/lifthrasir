//! The Inventory window: search, sort, category tabs, an 8-column slot grid, a
//! selection bar and the weight and zeny read-outs.
//!
//! [`InventoryUi`] is the only UI state. [`scene::build`] spawns the frame, title bar
//! and search row once. [`rebuild_body`] respawns the tabs, grid and selection bar
//! whenever the inventory or [`InventoryUi`] changes, and [`rebuild_footer`] respawns
//! the read-outs when weight or zeny change. Right-click opens the shared info modal
//! ([`crate::widgets::info_modal`]).

use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_assets::paths::item_icon_path;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::equipment::EquipItemRequested;
use lifthrasir_client::domain::hotbar::HotbarSlot;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};
use lifthrasir_client::domain::inventory::{Inventory, Item, ItemCategory, UseItemRequested};

use crate::screens::shared::{Ring, light_search_focus, thousands};
use crate::widgets::hotbar::HotbarDrag;
use crate::widgets::info_modal::shell::EdgeGrade;
use crate::widgets::info_modal::view::{decorated_name, inventory_item_view, item_edge_grade};
use crate::widgets::info_modal::{InfoContent, ItemAction, ItemActionKind, ShowInfoModal};
use crate::widgets::inventory_window::scene::{
    BodyView, CellView, FooterView, SelectionView, TabView,
};

pub mod scene;
#[cfg(test)]
mod tests;

/// Bag slots the server grants (`@max_inventory` in aesir, rAthena `MAX_INVENTORY`).
pub const CAPACITY: usize = 100;
const COLUMNS: usize = 8;
/// A filtered grid never draws fewer slots than this, so it keeps five full rows.
const MIN_SLOTS: usize = 40;
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// The category a tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InventoryTab {
    #[default]
    All,
    Use,
    Equip,
    Etc,
    Favorites,
}

impl InventoryTab {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Use,
        Self::Equip,
        Self::Etc,
        Self::Favorites,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Use => "Use",
            Self::Equip => "Equip",
            Self::Etc => "Etc",
            Self::Favorites => "Favorites",
        }
    }

    fn holds(self, item: &Item) -> bool {
        match self {
            Self::All => true,
            Self::Use => item.category() == ItemCategory::Use,
            Self::Equip => item.category() == ItemCategory::Equip,
            Self::Etc => item.category() == ItemCategory::Etc,
            Self::Favorites => item.favorite,
        }
    }
}

/// The grid order.
///
/// NOTE: the design also sorts by weight and by newest; the client knows neither per
/// item, so add them (and a real dropdown) once the protocol carries that data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InventorySort {
    #[default]
    Type,
    Name,
}

impl InventorySort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Name => "Name",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Type => Self::Name,
            Self::Name => Self::Type,
        }
    }
}

/// The window's whole UI state. `query` is the trimmed search text.
#[derive(Resource, Default)]
pub struct InventoryUi {
    pub open: bool,
    pub tab: InventoryTab,
    pub sort: InventorySort,
    pub query: String,
    pub selected: Option<u16>,
}

/// Last slot click, for double-click detection across body rebuilds.
#[derive(Resource, Default)]
pub struct LastCellClick {
    index: u16,
    at: Duration,
}

#[derive(Component, Default, Clone)]
pub struct InventoryWindowRoot;

/// The title bar, which is the drag handle.
#[derive(Component, Default, Clone)]
pub struct InventoryTitlebar;

/// Mount of the tabs, grid and selection bar.
#[derive(Component, Default, Clone)]
pub struct InventoryBody;

/// Mount of the weight and zeny read-outs.
#[derive(Component, Default, Clone)]
pub struct InventoryFooter;

/// The grid's scroll viewport.
#[derive(Component, Default, Clone)]
pub struct InventoryGrid;

#[derive(Component, Default, Clone)]
pub struct InventorySearchField;

/// A text of the static frame that [`rebuild_body`] keeps current.
#[derive(Component, Default, Clone, Copy)]
pub enum InventoryLabel {
    #[default]
    Used,
    Sort,
}

#[derive(Component, Default, Clone, Copy)]
pub struct InventoryTabButton(pub InventoryTab);

/// A filled slot, carrying the inventory index of its item.
#[derive(Component, Default, Clone, Copy)]
pub struct InventoryCell {
    pub index: u16,
}

/// The name tag of the hovered slot.
#[derive(Component, Default, Clone)]
pub struct InventoryTooltip;

pub struct InventoryWindowPlugin;

impl Plugin for InventoryWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUi>()
            .init_resource::<LastCellClick>()
            .add_systems(
                Update,
                (
                    toggle_inventory_window.run_if(ui_unfocused),
                    reflect_open,
                    sync_search,
                    light_search_focus::<InventorySearchField>,
                    rebuild_body.after(sync_search),
                    rebuild_footer,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset);
    }
}

fn reset(mut ui: ResMut<InventoryUi>) {
    *ui = InventoryUi::default();
}

fn toggle_inventory_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut ui: ResMut<InventoryUi>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if actions.just_pressed(&PlayerAction::Inventory) {
        ui.open = !ui.open;
    }
}

fn reflect_open(
    ui: Res<InventoryUi>,
    mut roots: Query<&mut Visibility, With<InventoryWindowRoot>>,
) {
    let want = if ui.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut roots {
        visibility.set_if_neq(want);
    }
}

/// Mirrors the search field into [`InventoryUi::query`].
fn sync_search(
    fields: Query<&EditableText, With<InventorySearchField>>,
    mut ui: ResMut<InventoryUi>,
) {
    let Ok(text) = fields.single() else {
        return;
    };
    let value = text.value().to_string();
    if ui.query != value.trim() {
        ui.query = value.trim().to_string();
    }
}

/// Respawns the tabs, grid and selection bar on every inventory or [`InventoryUi`]
/// change, and once when the mount first appears. The grid keeps its scroll offset
/// unless the tab, sort or search changed, so using or selecting an item deep in the
/// bag does not jump back to the top.
#[allow(clippy::too_many_arguments)]
fn rebuild_body(
    mut commands: Commands,
    inventory: Res<Inventory>,
    ui: Res<InventoryUi>,
    item_db: Option<Res<ItemDb>>,
    bodies: Query<(Entity, Option<&Children>, Ref<InventoryBody>)>,
    grids: Query<&ScrollPosition, With<InventoryGrid>>,
    mut labels: Query<(&mut Text, &InventoryLabel)>,
    mut last_filter: Local<(InventoryTab, InventorySort, String)>,
) {
    let Ok((body, children, body_ref)) = bodies.single() else {
        return;
    };
    if !inventory.is_changed() && !ui.is_changed() && !body_ref.is_added() {
        return;
    }
    let filter = (ui.tab, ui.sort, ui.query.clone());
    let scroll = if *last_filter == filter {
        grids.single().map(|scroll| scroll.0).unwrap_or_default()
    } else {
        Vec2::ZERO
    };
    *last_filter = filter;

    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    let view = body_view(&inventory, &ui, item_db.as_deref(), scroll);
    commands
        .spawn_scene(scene::body(view))
        .insert(ChildOf(body));

    for (mut text, label) in &mut labels {
        text.0 = match label {
            InventoryLabel::Used => format!("{} / {CAPACITY}", inventory.len()),
            InventoryLabel::Sort => ui.sort.label().to_string(),
        };
    }
}

/// Respawns the weight and zeny read-outs when their values change or the mount is new.
/// `CharacterStatus` changes on every HP tick, so the shown values are compared instead
/// of trusting change detection.
fn rebuild_footer(
    mut commands: Commands,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mounts: Query<(Entity, Option<&Children>), With<InventoryFooter>>,
    mut shown: Local<Option<(Entity, FooterView)>>,
) {
    let (Ok(status), Ok((mount, children))) = (player.single(), mounts.single()) else {
        return;
    };
    let view = FooterView {
        weight: status.weight,
        max_weight: status.max_weight,
        zeny: status.zeny,
    };
    if *shown == Some((mount, view)) {
        return;
    }
    *shown = Some((mount, view));
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands
        .spawn_scene(scene::footer(view))
        .insert(ChildOf(mount));
}

/// The items carried in the bag. Worn items live on the Character window.
fn bag(inventory: &Inventory) -> impl Iterator<Item = &Item> {
    inventory.iter().filter(|item| !item.is_equipped())
}

/// The item's name as the window shows and searches it, as in `+7 Blade [3]`.
pub(crate) fn item_name(item: &Item, item_db: Option<&ItemDb>) -> String {
    let name = item_db
        .and_then(|db| db.name(item.item_id, item.identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{}", item.item_id));
    decorated_name(&name, item.refine, card_slots(item, item_db))
}

/// Card slots of an identified item. An unidentified one hides them.
fn card_slots(item: &Item, item_db: Option<&ItemDb>) -> u8 {
    if !item.identified {
        return 0;
    }
    item_db
        .and_then(|db| db.slot_count(item.item_id))
        .unwrap_or(0)
}

fn type_rank(category: ItemCategory) -> u8 {
    match category {
        ItemCategory::Use => 0,
        ItemCategory::Equip => 1,
        ItemCategory::Etc => 2,
    }
}

/// The bag items the grid shows, filtered by tab and search and in sort order.
fn visible_items<'a>(
    inventory: &'a Inventory,
    ui: &InventoryUi,
    item_db: Option<&ItemDb>,
) -> Vec<&'a Item> {
    let needle = ui.query.to_lowercase();
    let mut items: Vec<(String, &Item)> = bag(inventory)
        .filter(|item| ui.tab.holds(item))
        .map(|item| (item_name(item, item_db).to_lowercase(), item))
        .filter(|(name, _)| name.contains(&needle))
        .collect();
    match ui.sort {
        InventorySort::Type => {
            items.sort_by_key(|(_, item)| (type_rank(item.category()), item.index));
        }
        InventorySort::Name => items.sort_by(|(a, _), (b, _)| a.cmp(b)),
    }
    items.into_iter().map(|(_, item)| item).collect()
}

/// Slots the grid draws: every bag slot when nothing filters it, otherwise whole rows
/// down to [`MIN_SLOTS`].
fn slot_total(shown: usize, ui: &InventoryUi) -> usize {
    if ui.tab == InventoryTab::All && ui.query.is_empty() {
        return CAPACITY.max(shown);
    }
    shown.next_multiple_of(COLUMNS).max(MIN_SLOTS)
}

/// What a slot holding `item` draws. `selected` is the index the selection ring goes on.
pub(crate) fn cell_view(item: &Item, item_db: Option<&ItemDb>, selected: Option<u16>) -> CellView {
    let slots = usize::from(card_slots(item, item_db));
    let ring = if selected == Some(item.index) {
        Ring::Selected
    } else if item.identified && item_edge_grade(item.refine, false) == EdgeGrade::Rare {
        Ring::Rare
    } else {
        Ring::None
    };
    CellView {
        index: item.index,
        icon: item_db
            .and_then(|db| db.icon_resource(item.item_id, item.identified))
            .map(item_icon_path),
        amount: item.amount,
        refine: item.refine,
        identified: item.identified,
        damaged: item.damaged,
        favorite: item.favorite,
        pips: item
            .cards
            .iter()
            .take(slots)
            .map(|&card| card != 0)
            .collect(),
        ring,
    }
}

fn selection_view(item: &Item, item_db: Option<&ItemDb>) -> SelectionView {
    let action = ItemAction::for_bag_item(item).map(|action| action.kind);
    let carried = (item.amount > 1).then(|| format!("{} carried", thousands(item.amount.into())));
    let meta: Vec<String> = [Some(item.type_label().to_string()), carried]
        .into_iter()
        .flatten()
        .collect();
    SelectionView {
        cell: cell_view(item, item_db, None),
        name: item_name(item, item_db),
        meta: meta.join(" · "),
        action_label: match action {
            Some(ItemActionKind::Equip) => "Equip",
            _ => "Use",
        },
        // The server refuses to equip an unidentified item.
        action_enabled: match action {
            Some(ItemActionKind::Equip) => item.identified,
            Some(_) => true,
            None => false,
        },
    }
}

fn body_view(
    inventory: &Inventory,
    ui: &InventoryUi,
    item_db: Option<&ItemDb>,
    scroll: Vec2,
) -> BodyView {
    let items = visible_items(inventory, ui, item_db);
    let no_match = (items.is_empty() && !ui.query.is_empty())
        .then(|| format!("No items match \u{201c}{}\u{201d}", ui.query));
    let mut cells: Vec<Option<CellView>> = items
        .iter()
        .map(|item| Some(cell_view(item, item_db, ui.selected)))
        .collect();
    cells.resize_with(slot_total(items.len(), ui), || None);
    BodyView {
        tabs: InventoryTab::ALL.map(|tab| TabView {
            tab,
            count: bag(inventory).filter(|item| tab.holds(item)).count(),
            active: tab == ui.tab,
        }),
        cells,
        scroll,
        no_match,
        selection: ui
            .selected
            .and_then(|index| inventory.get(index))
            .filter(|item| !item.is_equipped())
            .map(|item| selection_view(item, item_db)),
    }
}

fn is_double_click(last: &LastCellClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

/// Writers for a bag item's primary action, shared by a double-click and the selection
/// bar's button.
#[derive(SystemParam)]
pub(super) struct ItemActions<'w> {
    use_item: MessageWriter<'w, UseItemRequested>,
    equip: MessageWriter<'w, EquipItemRequested>,
}

impl ItemActions<'_> {
    fn request(&mut self, item: &Item) {
        match ItemAction::for_bag_item(item).map(|action| action.kind) {
            Some(ItemActionKind::Use) => {
                self.use_item.write(UseItemRequested {
                    index: item.index.into(),
                });
            }
            Some(ItemActionKind::Equip) => {
                self.equip.write(EquipItemRequested { index: item.index });
            }
            Some(ItemActionKind::Unequip) | None => {}
        }
    }
}

pub(super) fn close_window(_: On<Activate>, mut ui: ResMut<InventoryUi>) {
    ui.open = false;
}

pub(super) fn pick_tab(
    click: On<PointerClick>,
    tabs: Query<&InventoryTabButton>,
    mut ui: ResMut<InventoryUi>,
) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = tab.0;
}

pub(super) fn cycle_sort(_: On<PointerClick>, mut ui: ResMut<InventoryUi>) {
    ui.sort = ui.sort.next();
}

/// The selection bar's primary button acts on the selected item.
pub(super) fn act_on_selection(
    _: On<Activate>,
    ui: Res<InventoryUi>,
    inventory: Res<Inventory>,
    mut actions: ItemActions,
) {
    let Some(item) = ui.selected.and_then(|index| inventory.get(index)) else {
        return;
    };
    actions.request(item);
}

/// Dragging a slot arms the hotbar with the item's stable `item_id`, so dropping it on
/// a bar slot assigns it. A plain click still reaches [`on_cell_click`] because
/// `bevy_picking` only emits `DragStart` after a press-and-move.
pub(super) fn on_cell_drag_start(
    drag: On<PointerDragStart>,
    cells: Query<&InventoryCell>,
    inventory: Res<Inventory>,
    mut hotbar_drag: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    let Some(item) = inventory.get(cell.index) else {
        return;
    };
    hotbar_drag.payload = Some(HotbarSlot::Item(item.item_id));
}

/// Hovering a slot shows its item's name above it. The tag is the slot's child, so a
/// body rebuild takes it down with the slot.
pub(super) fn show_slot_name(
    over: On<PointerOver>,
    cells: Query<&InventoryCell>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    mut commands: Commands,
) {
    let Ok(cell) = cells.get(over.entity) else {
        return;
    };
    let Some(item) = inventory.get(cell.index) else {
        return;
    };
    let name = item_name(item, item_db.as_deref());
    commands
        .spawn_scene(scene::slot_tooltip(name))
        .insert(ChildOf(over.entity));
}

pub(super) fn hide_slot_name(
    _: On<PointerOut>,
    tooltips: Query<Entity, With<InventoryTooltip>>,
    mut commands: Commands,
) {
    for tooltip in &tooltips {
        commands.entity(tooltip).despawn();
    }
}

/// A click selects the slot's item and a double-click runs its primary action. A
/// secondary click opens the info modal instead and leaves the selection alone.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_cell_click(
    click: On<PointerClick>,
    cells: Query<&InventoryCell>,
    mut ui: ResMut<InventoryUi>,
    time: Res<Time>,
    mut last: ResMut<LastCellClick>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    option_db: Option<Res<RandomOptionDb>>,
    mut actions: ItemActions,
    mut info: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    let Some(item) = inventory.get(cell.index) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        let Some(item_db) = item_db.as_deref() else {
            warn!("inventory: ItemDb not loaded yet, ignoring inspect");
            return;
        };
        info.write(ShowInfoModal {
            content: InfoContent::Item {
                view: inventory_item_view(item, item_db, option_db.as_deref()),
                action: ItemAction::for_bag_item(item),
            },
        });
        return;
    }
    ui.selected = Some(cell.index);
    let now = time.elapsed();
    if is_double_click(&last, cell.index, now) {
        actions.request(item);
    }
    *last = LastCellClick {
        index: cell.index,
        at: now,
    };
}
