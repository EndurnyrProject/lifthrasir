//! Hotbar: a fixed, always-visible 12-slot quick-bar pinned bottom-center.
//!
//! Renders the `Hotbar` resource every frame (skill/item/empty styling, icon,
//! stack count, and a cooldown overlay + seconds for skills) and activates a
//! filled slot on click by writing `HotbarSlotActivated` — the same seam the
//! F1..F12 keys use (Task 4).
//!
//! Slots are also `bevy_picking` drag targets: dropping a `SkillCell` /
//! `InventoryCell` assigns it, dropping another slot swaps the two, and a
//! right-click clears. Dragging a filled slot *off* the bar (releasing over
//! anything that isn't a slot) unregisters it. The dragged payload (a
//! skill/item) is carried in the `HotbarDrag` resource, set by the source cells
//! on `DragStart` and reset on `DragEnd`; a slot↔slot swap is detected from the
//! `DragDrop` dragged entity.
//!
//! `SkillCooldownTracker` only exposes the remaining seconds (not the original
//! duration), so the cooldown render is a darkening overlay plus the rounded-up
//! seconds rather than a proportional sweep (design D7's accepted fallback).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_engine::core::state::GameState;
use game_engine::domain::hotbar::{Hotbar, HotbarSlot, HotbarSlotActivated};
use game_engine::domain::inventory::Inventory;
use game_engine::domain::skill::SkillCooldownTracker;
use game_engine::infrastructure::assets::item_icon_path;
use game_engine::infrastructure::item::ItemDb;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;
use crate::worldspace::viewport_to_ui;

const SLOTS: usize = 12;
const SLOT_SIZE: f32 = 44.0;
const BAR_PADDING_TOP: f32 = 5.0;
const BAR_PADDING_BOTTOM: f32 = 7.0;
const BAR_BOTTOM_BORDER: f32 = 1.0;
pub(crate) const HOTBAR_BOTTOM: f32 = 14.0;
pub(crate) const HOTBAR_HEIGHT: f32 =
    SLOT_SIZE + BAR_PADDING_TOP + BAR_PADDING_BOTTOM + BAR_BOTTOM_BORDER;
const ICON_SIZE: f32 = 32.0;
const ICON_INSET: f32 = (SLOT_SIZE - ICON_SIZE) / 2.0;

/// The cursor-following drag ghost: a translucent icon shown while a skill/item is
/// being dragged. Sits above every window (`GHOST_Z`) and is `Pickable::IGNORE` so
/// it never steals the drop target's hit.
const GHOST_SIZE: f32 = 34.0;
const GHOST_Z: i32 = 2000;
const GHOST_ALPHA: f32 = 0.85;

/// The hover name toast floats above its slot; lifted over neighbouring cells (but
/// below the drag ghost) so a wide name is never occluded by the next slot.
const TOOLTIP_Z: i32 = 1500;

const BAR_BG: Color = Color::srgb(0.043, 0.067, 0.059);
const SLOT_EMPTY_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.28);
const SLOT_SKILL_BG: Color = Color::srgba(0.02, 0.078, 0.051, 0.5);
const SLOT_ITEM_BG: Color = Color::srgba(0.086, 0.063, 0.012, 0.45);
const COOLDOWN_BG: Color = Color::srgba(0.008, 0.02, 0.016, 0.72);
const DISABLED_ICON_ALPHA: f32 = 0.34;
const COOLDOWN_ICON_ALPHA: f32 = 0.5;

/// Marks a hotbar cell with its slot index (0..11).
#[derive(Component)]
pub struct HotbarSlotUi(pub usize);

/// Marks the icon image node inside slot `i`.
#[derive(Component)]
struct HotbarIcon(usize);

/// Marks the darkening cooldown overlay inside slot `i`.
#[derive(Component)]
struct HotbarCooldownOverlay(usize);

/// Marks the cooldown-seconds text inside slot `i`.
#[derive(Component)]
struct HotbarCooldownText(usize);

/// Marks the stack-count text inside slot `i`.
#[derive(Component)]
struct HotbarStackText(usize);

/// Marks the hover name toast spawned above a slot; despawned on pointer-out.
#[derive(Component)]
struct HotbarTooltip;

/// The skill/item currently being dragged onto the bar. Set by the source cells
/// (skill window, inventory, or a filled slot) on `DragStart`, consumed by a
/// slot's `DragDrop`, and reset on `DragEnd`.
#[derive(Resource, Default)]
pub struct HotbarDrag {
    pub payload: Option<HotbarSlot>,
    /// The bar slot this drag started from, if any. A drag that ends *not* over
    /// a slot clears it (unregister). Stays `None` for drags from the skill or
    /// inventory windows, which can never unregister a slot.
    source: Option<usize>,
    /// Set when a `DragDrop` lands on a slot, so `reset_drag` knows the drag was
    /// consumed (placed/swapped) and must not unregister the source.
    dropped_on_slot: bool,
}

/// The single cursor-following ghost icon spawned while a drag is in flight.
#[derive(Component)]
struct HotbarDragGhost;

pub struct HotbarWidgetPlugin;

impl Plugin for HotbarWidgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HotbarDrag>();
        app.add_observer(reset_drag);
        app.add_systems(
            Update,
            (update_hotbar, update_drag_ghost).run_if(in_state(GameState::InGame)),
        );
    }
}

/// A resolved drop: either swap with another slot or place a fresh payload.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DropSource {
    Swap(usize),
    Place(HotbarSlot),
}

/// Classifies a drop: a dragged slot index means swap, otherwise place the
/// carried payload (if any).
fn drop_source(dragged_slot: Option<usize>, payload: Option<HotbarSlot>) -> Option<DropSource> {
    match dragged_slot {
        Some(from) => Some(DropSource::Swap(from)),
        None => payload.map(DropSource::Place),
    }
}

/// Applies a resolved drop onto `target`. All `Hotbar` helpers are bounds-safe.
fn apply_drop(hotbar: &mut Hotbar, target: usize, dropped: DropSource) {
    match dropped {
        DropSource::Swap(from) => hotbar.swap(from, target),
        DropSource::Place(slot) => hotbar.assign(target, slot),
    }
}

/// What a single slot should render this frame, derived purely from the bar +
/// inventory + catalog + the resolved cooldown seconds.
#[derive(Debug, Clone, PartialEq)]
enum SlotKind {
    Empty,
    Skill,
    Item,
    DisabledItem,
}

#[derive(Debug, Clone, PartialEq)]
struct SlotDisplay {
    kind: SlotKind,
    icon: Option<String>,
    stack: Option<u16>,
    cooldown_secs: Option<u32>,
}

/// Pure per-slot display state. `cooldown_secs` is the already-resolved
/// `remaining_secs` for a skill slot (the caller looks it up), kept out of this
/// function so it stays trivially testable.
fn slot_display(
    slot: Option<HotbarSlot>,
    inventory: &Inventory,
    catalog: Option<&SkillCatalog>,
    item_db: Option<&ItemDb>,
    cooldown_secs: Option<f32>,
) -> SlotDisplay {
    match slot {
        None => SlotDisplay {
            kind: SlotKind::Empty,
            icon: None,
            stack: None,
            cooldown_secs: None,
        },
        Some(HotbarSlot::Skill(id)) => SlotDisplay {
            kind: SlotKind::Skill,
            icon: catalog.and_then(|c| c.icon_path(id)),
            stack: None,
            cooldown_secs: cooldown_secs.map(|s| s.ceil() as u32),
        },
        Some(HotbarSlot::Item(item_id)) => {
            match inventory.iter().find(|it| it.item_id == item_id) {
                Some(item) => SlotDisplay {
                    kind: SlotKind::Item,
                    icon: item_db
                        .and_then(|db| db.icon_resource(item_id, item.identified))
                        .map(item_icon_path),
                    stack: Some(item.amount),
                    cooldown_secs: None,
                },
                None => SlotDisplay {
                    kind: SlotKind::DisabledItem,
                    icon: item_db
                        .and_then(|db| db.icon_resource(item_id, true))
                        .map(item_icon_path),
                    stack: None,
                    cooldown_secs: None,
                },
            }
        }
    }
}

/// Icon tint (the BMP icons carry their own color, so this only controls
/// presence/dimming via alpha). `Color::NONE` hides the icon.
fn icon_color(display: &SlotDisplay) -> Color {
    if display.icon.is_none() {
        return Color::NONE;
    }
    let alpha = match display.kind {
        SlotKind::DisabledItem => DISABLED_ICON_ALPHA,
        _ => 1.0,
    };
    let alpha = if display.cooldown_secs.is_some() {
        alpha * COOLDOWN_ICON_ALPHA
    } else {
        alpha
    };
    Color::WHITE.with_alpha(alpha)
}

/// The name a slot shows on hover: the skill's display name, or the item's
/// (identified-aware) name. `None` for an empty slot or an id missing from the
/// catalog/db. Falls back to the identified name for an item no longer held,
/// mirroring `ghost_icon`.
fn slot_label(
    slot: Option<HotbarSlot>,
    inventory: &Inventory,
    catalog: Option<&SkillCatalog>,
    item_db: Option<&ItemDb>,
) -> Option<String> {
    match slot? {
        HotbarSlot::Skill(id) => catalog
            .and_then(|c| c.get(id))
            .map(|meta| meta.display_name.clone()),
        HotbarSlot::Item(item_id) => {
            let identified = inventory
                .iter()
                .find(|it| it.item_id == item_id)
                .map(|it| it.identified)
                .unwrap_or(true);
            item_db
                .and_then(|db| db.name(item_id, identified))
                .map(str::to_string)
        }
    }
}

/// Builds the bottom-center bar under `parent`: a centered row of 12 cells, each
/// with an F-key label, icon, cooldown overlay + seconds, and a stack count.
pub fn spawn_hotbar(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font = asset_server.load(theme::FONT_BODY);

    let wrapper = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(HOTBAR_BOTTOM),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect {
                    left: Val::Px(7.0),
                    right: Val::Px(7.0),
                    top: Val::Px(BAR_PADDING_TOP),
                    bottom: Val::Px(BAR_PADDING_BOTTOM),
                },
                border: UiRect {
                    bottom: Val::Px(BAR_BOTTOM_BORDER),
                    ..default()
                },
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            BorderColor::all(theme::EMERALD_DEEP),
            Pickable::IGNORE,
            ChildOf(wrapper),
        ))
        .id();

    for i in 0..SLOTS {
        spawn_slot(commands, bar, i, &font);
    }
}

fn spawn_slot(commands: &mut Commands, bar: Entity, i: usize, font: &Handle<Font>) {
    let cell = commands
        .spawn((
            HotbarSlotUi(i),
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(SLOT_EMPTY_BG),
            BorderColor::all(theme::STROKE),
            Pickable::default(),
            ChildOf(bar),
        ))
        .id();

    commands.spawn((
        HotbarIcon(i),
        ImageNode {
            color: Color::NONE,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(ICON_INSET),
            top: Val::Px(ICON_INSET),
            width: Val::Px(ICON_SIZE),
            height: Val::Px(ICON_SIZE),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(cell),
    ));

    let overlay = commands
        .spawn((
            HotbarCooldownOverlay(i),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
            ChildOf(cell),
        ))
        .id();
    commands.spawn((
        HotbarCooldownText(i),
        theme::label("", font.clone(), 13.0, theme::TEXT),
        ChildOf(overlay),
    ));

    commands.spawn((
        theme::label(format!("F{}", i + 1), font.clone(), 8.5, theme::TEXT_FAINT),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(2.0),
            left: Val::Px(3.0),
            ..default()
        },
        ChildOf(cell),
    ));

    commands.spawn((
        HotbarStackText(i),
        theme::label("", font.clone(), 9.0, theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(1.0),
            right: Val::Px(2.0),
            ..default()
        },
        ChildOf(cell),
    ));

    commands.entity(cell).observe(on_slot_click);
    commands.entity(cell).observe(on_slot_drag_start);
    commands.entity(cell).observe(on_slot_drag_drop);
    commands.entity(cell).observe(on_slot_hover_over);
    commands.entity(cell).observe(on_slot_hover_out);
}

/// The item/skill lookup context every hotbar system needs to render a slot:
/// the inventory (for item stacks), the skill catalog and item db (for icons and
/// names), and the asset server (to load those icons). Grouped so each of the
/// three hotbar systems takes one parameter for all four.
#[derive(SystemParam)]
struct HotbarItemLookup<'w> {
    inventory: Res<'w, Inventory>,
    catalog: Option<Res<'w, SkillCatalog>>,
    item_db: Option<Res<'w, ItemDb>>,
    asset_server: Res<'w, AssetServer>,
}

/// Reflects the bar state into every cell, writing each node only on change so a
/// cooling-down skill (this system runs each frame) doesn't churn the others.
// Still eight parameters after the lookup bundle (the five per-element view
// queries are irreducibly distinct); grouping those purely to satisfy the lint
// would be an artificial bundle, so the allow stays.
#[allow(clippy::too_many_arguments)]
fn update_hotbar(
    hotbar: Res<Hotbar>,
    lookup: HotbarItemLookup,
    cooldowns: Res<SkillCooldownTracker>,
    mut cells: Query<(&mut BackgroundColor, &mut BorderColor, &HotbarSlotUi)>,
    mut icons: Query<(&mut ImageNode, &HotbarIcon)>,
    mut overlays: Query<(&mut BackgroundColor, &HotbarCooldownOverlay), Without<HotbarSlotUi>>,
    mut cd_text: Query<(&mut Text, &HotbarCooldownText)>,
    mut stack_text: Query<(&mut Text, &HotbarStackText), Without<HotbarCooldownText>>,
) {
    let displays: Vec<SlotDisplay> = (0..SLOTS)
        .map(|i| {
            let slot = hotbar.get(i);
            let cooldown_secs = match slot {
                Some(HotbarSlot::Skill(id)) => cooldowns.remaining_secs(id),
                _ => None,
            };
            slot_display(
                slot,
                &lookup.inventory,
                lookup.catalog.as_deref(),
                lookup.item_db.as_deref(),
                cooldown_secs,
            )
        })
        .collect();

    for (mut bg, mut border, cell) in &mut cells {
        let Some(display) = displays.get(cell.0) else {
            continue;
        };
        let (bg_color, border_color) = match display.kind {
            SlotKind::Empty | SlotKind::DisabledItem => (SLOT_EMPTY_BG, theme::STROKE),
            SlotKind::Skill => (SLOT_SKILL_BG, theme::EMERALD.with_alpha(0.28)),
            SlotKind::Item => (SLOT_ITEM_BG, theme::GOLD.with_alpha(0.28)),
        };
        if bg.0 != bg_color {
            bg.0 = bg_color;
        }
        let new_border = BorderColor::all(border_color);
        if *border != new_border {
            *border = new_border;
        }
    }

    for (mut image, icon) in &mut icons {
        let Some(display) = displays.get(icon.0) else {
            continue;
        };
        if let Some(path) = &display.icon {
            let handle = lookup.asset_server.load(path);
            if image.image != handle {
                image.image = handle;
            }
        }
        let color = icon_color(display);
        if image.color != color {
            image.color = color;
        }
    }

    for (mut bg, overlay) in &mut overlays {
        let Some(display) = displays.get(overlay.0) else {
            continue;
        };
        let color = if display.cooldown_secs.is_some() {
            COOLDOWN_BG
        } else {
            Color::NONE
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }

    for (mut text, marker) in &mut cd_text {
        let value = displays
            .get(marker.0)
            .and_then(|d| d.cooldown_secs)
            .map(|s| s.to_string())
            .unwrap_or_default();
        set_text(&mut text, value);
    }

    for (mut text, marker) in &mut stack_text {
        let value = displays
            .get(marker.0)
            .and_then(|d| d.stack)
            .filter(|&n| n > 1)
            .map(|n| n.to_string())
            .unwrap_or_default();
        set_text(&mut text, value);
    }
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        *text = Text::new(value);
    }
}

/// Primary-click activates a filled slot through the shared seam; secondary-click
/// clears it. Empty slots do nothing.
fn on_slot_click(
    click: On<Pointer<Click>>,
    cells: Query<&HotbarSlotUi>,
    mut hotbar: ResMut<Hotbar>,
    mut activated: MessageWriter<HotbarSlotActivated>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if hotbar.get(cell.0).is_none() {
        return;
    }
    match click.button {
        PointerButton::Secondary => hotbar.clear(cell.0),
        PointerButton::Primary => {
            activated.write(HotbarSlotActivated { index: cell.0 });
        }
        _ => {}
    }
}

/// Hovering a filled slot spawns a name toast centered above it; empty or
/// unresolved slots show nothing.
fn on_slot_hover_over(
    over: On<Pointer<Over>>,
    cells: Query<&HotbarSlotUi>,
    hotbar: Res<Hotbar>,
    lookup: HotbarItemLookup,
    mut commands: Commands,
) {
    let Ok(cell) = cells.get(over.entity) else {
        return;
    };
    let Some(label) = slot_label(
        hotbar.get(cell.0),
        &lookup.inventory,
        lookup.catalog.as_deref(),
        lookup.item_db.as_deref(),
    ) else {
        return;
    };
    let font = lookup.asset_server.load(theme::FONT_BODY);

    let tooltip = commands
        .spawn((
            HotbarTooltip,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(SLOT_SIZE + 6.0),
                left: Val::Percent(50.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::new(Val::Percent(-50.0), Val::ZERO)),
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            GlobalZIndex(TOOLTIP_Z),
            Pickable::IGNORE,
            ChildOf(over.entity),
        ))
        .id();

    commands.spawn((
        theme::label(label, font, 11.0, theme::TEXT),
        ChildOf(tooltip),
    ));
}

/// Leaving a slot despawns any visible name toast (only one is shown at a time).
fn on_slot_hover_out(
    _: On<Pointer<Out>>,
    tooltips: Query<Entity, With<HotbarTooltip>>,
    mut commands: Commands,
) {
    for tooltip in &tooltips {
        commands.entity(tooltip).despawn();
    }
}

/// A filled slot is a drag source for swapping: its payload is its current slot.
fn on_slot_drag_start(
    drag: On<Pointer<DragStart>>,
    cells: Query<&HotbarSlotUi>,
    hotbar: Res<Hotbar>,
    mut state: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    if let Some(slot) = hotbar.get(cell.0) {
        state.payload = Some(slot);
        state.source = Some(cell.0);
    }
}

/// Drop target: a dropped slot swaps, anything else places the carried payload.
// NOTE: drag-hover highlight deferred — `update_hotbar` repaints the border
// every frame, so a `.drag` hover tint would need it to read picking hover state.
fn on_slot_drag_drop(
    drop: On<Pointer<DragDrop>>,
    cells: Query<&HotbarSlotUi>,
    mut state: ResMut<HotbarDrag>,
    mut hotbar: ResMut<Hotbar>,
) {
    let Ok(target) = cells.get(drop.entity) else {
        return;
    };
    state.dropped_on_slot = true;
    let dragged_slot = cells.get(drop.dropped).ok().map(|c| c.0);
    let Some(dropped) = drop_source(dragged_slot, state.payload) else {
        return;
    };
    apply_drop(&mut hotbar, target.0, dropped);
}

/// A drag that started on the bar and ended off it unregisters its source slot.
/// `DragDrop` fires before `DragEnd`, so `dropped_on_slot` is already set when a
/// drop landed on a slot.
fn unregister_target(source: Option<usize>, dropped_on_slot: bool) -> Option<usize> {
    source.filter(|_| !dropped_on_slot)
}

/// Unregisters the source slot if the drag ended off the bar, then clears the
/// transient drag state. Runs for every drag (including window-sourced ones,
/// whose `source` is `None`).
fn reset_drag(_: On<Pointer<DragEnd>>, mut state: ResMut<HotbarDrag>, mut hotbar: ResMut<Hotbar>) {
    if let Some(slot) = unregister_target(state.source, state.dropped_on_slot) {
        hotbar.clear(slot);
    }
    state.payload = None;
    state.source = None;
    state.dropped_on_slot = false;
}

/// The icon a dragged payload should show on the cursor ghost: the skill catalog
/// icon for a skill, or the inventory item's icon for an item (falling back to the
/// identified art when the source item is no longer held).
fn ghost_icon(
    payload: HotbarSlot,
    inventory: &Inventory,
    catalog: Option<&SkillCatalog>,
    item_db: Option<&ItemDb>,
) -> Option<String> {
    match payload {
        HotbarSlot::Skill(id) => catalog.and_then(|c| c.icon_path(id)),
        HotbarSlot::Item(item_id) => {
            let identified = inventory
                .iter()
                .find(|it| it.item_id == item_id)
                .map(|it| it.identified)
                .unwrap_or(true);
            item_db
                .and_then(|db| db.icon_resource(item_id, identified))
                .map(item_icon_path)
        }
    }
}

/// Drives the cursor-following drag ghost: spawns it on the first frame a drag
/// carries a resolvable icon, tracks the cursor while the drag is live, and
/// despawns it once the payload clears (or its icon can no longer be resolved).
fn update_drag_ghost(
    mut commands: Commands,
    drag: Res<HotbarDrag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    lookup: HotbarItemLookup,
    mut ghosts: Query<(Entity, &mut Node), With<HotbarDragGhost>>,
) {
    let ghost = ghosts.single_mut().ok();
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let icon = drag.payload.and_then(|p| {
        ghost_icon(
            p,
            &lookup.inventory,
            lookup.catalog.as_deref(),
            lookup.item_db.as_deref(),
        )
    });

    let (Some(cursor), Some(icon)) = (cursor, icon) else {
        if let Some((entity, _)) = ghost {
            commands.entity(entity).despawn();
        }
        return;
    };

    let cursor = viewport_to_ui(cursor, &ui_scale);
    let left = Val::Px(cursor.x - GHOST_SIZE / 2.0);
    let top = Val::Px(cursor.y - GHOST_SIZE / 2.0);
    match ghost {
        Some((_, mut node)) => {
            node.left = left;
            node.top = top;
        }
        None => spawn_ghost(&mut commands, &lookup.asset_server, icon, left, top),
    }
}

fn spawn_ghost(
    commands: &mut Commands,
    asset_server: &AssetServer,
    icon: String,
    left: Val,
    top: Val,
) {
    commands.spawn((
        HotbarDragGhost,
        ImageNode {
            image: asset_server.load(icon),
            color: Color::WHITE.with_alpha(GHOST_ALPHA),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left,
            top,
            width: Val::Px(GHOST_SIZE),
            height: Val::Px(GHOST_SIZE),
            ..default()
        },
        GlobalZIndex(GHOST_Z),
        Pickable::IGNORE,
        DespawnOnExit(GameState::InGame),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::inventory::Item;

    fn inventory_with(item_id: u32, amount: u16) -> Inventory {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 5,
            item_id,
            amount,
            identified: true,
            ..Default::default()
        });
        inv
    }

    #[test]
    fn empty_slot_is_empty() {
        let display = slot_display(None, &Inventory::default(), None, None, None);
        assert_eq!(display.kind, SlotKind::Empty);
        assert_eq!(display.stack, None);
        assert_eq!(display.cooldown_secs, None);
    }

    #[test]
    fn skill_slot_rounds_cooldown_seconds_up() {
        let display = slot_display(
            Some(HotbarSlot::Skill(42)),
            &Inventory::default(),
            None,
            None,
            Some(2.3),
        );
        assert_eq!(display.kind, SlotKind::Skill);
        assert_eq!(display.cooldown_secs, Some(3));
    }

    #[test]
    fn skill_slot_without_cooldown_has_no_seconds() {
        let display = slot_display(
            Some(HotbarSlot::Skill(42)),
            &Inventory::default(),
            None,
            None,
            None,
        );
        assert_eq!(display.kind, SlotKind::Skill);
        assert_eq!(display.cooldown_secs, None);
    }

    #[test]
    fn item_slot_present_shows_amount() {
        let inv = inventory_with(501, 7);
        let display = slot_display(Some(HotbarSlot::Item(501)), &inv, None, None, None);
        assert_eq!(display.kind, SlotKind::Item);
        assert_eq!(display.stack, Some(7));
    }

    #[test]
    fn item_slot_absent_is_disabled() {
        let display = slot_display(
            Some(HotbarSlot::Item(999)),
            &Inventory::default(),
            None,
            None,
            None,
        );
        assert_eq!(display.kind, SlotKind::DisabledItem);
        assert_eq!(display.stack, None);
    }

    fn text_of(app: &App, e: Entity) -> String {
        app.world().get::<Text>(e).unwrap().0.clone()
    }

    fn border_of(app: &App, e: Entity) -> BorderColor {
        *app.world().get::<BorderColor>(e).unwrap()
    }

    #[test]
    fn update_hotbar_reflects_item_amount_and_grays_absent() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Font>();

        let mut hotbar = Hotbar::default();
        hotbar.assign(0, HotbarSlot::Item(501));
        hotbar.assign(1, HotbarSlot::Item(999));
        app.insert_resource(hotbar);
        app.insert_resource(inventory_with(501, 5));
        app.init_resource::<SkillCooldownTracker>();

        let present_stack = spawn_test_slot(&mut app, 0);
        let absent_stack = spawn_test_slot(&mut app, 1);

        app.add_systems(Update, update_hotbar);
        app.update();

        assert_eq!(text_of(&app, present_stack.stack), "5");
        assert_eq!(text_of(&app, absent_stack.stack), "");

        let item_border = border_of(&app, present_stack.cell);
        let disabled_border = border_of(&app, absent_stack.cell);
        assert_ne!(item_border, disabled_border);
        assert_eq!(disabled_border, BorderColor::all(theme::STROKE));
    }

    struct TestSlot {
        cell: Entity,
        overlay: Entity,
        cooldown: Entity,
        stack: Entity,
    }

    fn spawn_test_slot(app: &mut App, i: usize) -> TestSlot {
        let cell = app
            .world_mut()
            .spawn((
                HotbarSlotUi(i),
                BackgroundColor(SLOT_EMPTY_BG),
                BorderColor::all(theme::STROKE),
            ))
            .id();
        app.world_mut().spawn((HotbarIcon(i), ImageNode::default()));
        let overlay = app
            .world_mut()
            .spawn((HotbarCooldownOverlay(i), BackgroundColor(Color::NONE)))
            .id();
        let cooldown = app
            .world_mut()
            .spawn((HotbarCooldownText(i), Text::new("")))
            .id();
        let stack = app
            .world_mut()
            .spawn((HotbarStackText(i), Text::new("")))
            .id();
        TestSlot {
            cell,
            overlay,
            cooldown,
            stack,
        }
    }

    #[test]
    fn update_hotbar_reflects_cooldown_seconds() {
        use game_engine::domain::skill::cooldown::apply_skill_cooldown;
        use net_contract::events::SkillCooldownSet;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.add_message::<SkillCooldownSet>();
        app.init_resource::<SkillCooldownTracker>();

        let mut hotbar = Hotbar::default();
        hotbar.assign(0, HotbarSlot::Skill(7));
        app.insert_resource(hotbar);
        app.insert_resource(Inventory::default());

        let slot = spawn_test_slot(&mut app, 0);

        app.world_mut()
            .resource_mut::<Messages<SkillCooldownSet>>()
            .write(SkillCooldownSet {
                skill_id: 7,
                tick: 2300,
            });

        app.add_systems(Update, (apply_skill_cooldown, update_hotbar).chain());
        app.update();

        assert_eq!(text_of(&app, slot.cooldown), "3");
        assert_eq!(
            app.world().get::<BackgroundColor>(slot.overlay).unwrap().0,
            COOLDOWN_BG
        );
    }

    fn click_event(target: Entity, window: Entity, button: PointerButton) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    fn activations(app: &App) -> Vec<usize> {
        app.world()
            .resource::<Messages<HotbarSlotActivated>>()
            .iter_current_update_messages()
            .map(|m| m.index)
            .collect()
    }

    #[test]
    fn click_filled_slot_activates_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<HotbarSlotActivated>();

        let mut hotbar = Hotbar::default();
        hotbar.assign(4, HotbarSlot::Skill(10));
        app.insert_resource(hotbar);

        let cell = app.world_mut().spawn(HotbarSlotUi(4)).id();
        app.world_mut().entity_mut(cell).observe(on_slot_click);
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        app.update();

        assert_eq!(activations(&app), vec![4]);
    }

    #[test]
    fn click_empty_slot_does_nothing() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<HotbarSlotActivated>();
        app.insert_resource(Hotbar::default());

        let cell = app.world_mut().spawn(HotbarSlotUi(4)).id();
        app.world_mut().entity_mut(cell).observe(on_slot_click);
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        app.update();

        assert!(activations(&app).is_empty());
    }

    #[test]
    fn right_click_clears_filled_slot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<HotbarSlotActivated>();

        let mut hotbar = Hotbar::default();
        hotbar.assign(4, HotbarSlot::Skill(10));
        app.insert_resource(hotbar);

        let cell = app.world_mut().spawn(HotbarSlotUi(4)).id();
        app.world_mut().entity_mut(cell).observe(on_slot_click);
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));
        app.update();

        assert_eq!(app.world().resource::<Hotbar>().get(4), None);
        assert!(activations(&app).is_empty());
    }

    #[test]
    fn drop_source_classifies_swap_and_place() {
        assert_eq!(drop_source(Some(3), None), Some(DropSource::Swap(3)));
        assert_eq!(
            drop_source(None, Some(HotbarSlot::Skill(7))),
            Some(DropSource::Place(HotbarSlot::Skill(7)))
        );
        assert_eq!(drop_source(None, None), None);
        assert_eq!(
            drop_source(Some(3), Some(HotbarSlot::Item(1))),
            Some(DropSource::Swap(3))
        );
    }

    #[test]
    fn unregister_off_bar_drag_clears_source() {
        assert_eq!(unregister_target(Some(4), false), Some(4));
    }

    #[test]
    fn unregister_drop_on_slot_keeps_source() {
        assert_eq!(unregister_target(Some(4), true), None);
    }

    #[test]
    fn unregister_window_drag_has_no_source() {
        assert_eq!(unregister_target(None, false), None);
        assert_eq!(unregister_target(None, true), None);
    }

    #[test]
    fn apply_drop_place_into_empty_assigns() {
        let mut bar = Hotbar::default();
        apply_drop(&mut bar, 2, DropSource::Place(HotbarSlot::Item(501)));
        assert_eq!(bar.get(2), Some(HotbarSlot::Item(501)));
    }

    #[test]
    fn apply_drop_place_over_filled_overwrites() {
        let mut bar = Hotbar::default();
        bar.assign(2, HotbarSlot::Skill(1));
        apply_drop(&mut bar, 2, DropSource::Place(HotbarSlot::Item(501)));
        assert_eq!(bar.get(2), Some(HotbarSlot::Item(501)));
    }

    #[test]
    fn apply_drop_swap_exchanges_slots() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        bar.assign(1, HotbarSlot::Item(2));
        apply_drop(&mut bar, 1, DropSource::Swap(0));
        assert_eq!(bar.get(0), Some(HotbarSlot::Item(2)));
        assert_eq!(bar.get(1), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn apply_drop_swap_with_empty_target_moves() {
        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(1));
        apply_drop(&mut bar, 5, DropSource::Swap(0));
        assert_eq!(bar.get(0), None);
        assert_eq!(bar.get(5), Some(HotbarSlot::Skill(1)));
    }

    #[test]
    fn apply_drop_out_of_range_is_safe() {
        let mut bar = Hotbar::default();
        apply_drop(&mut bar, 99, DropSource::Place(HotbarSlot::Skill(1)));
        apply_drop(&mut bar, 99, DropSource::Swap(0));
        assert!(bar.slots.iter().all(|s| s.is_none()));
    }

    fn drag_end_event(target: Entity, window: Entity) -> Pointer<DragEnd> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
            },
            target,
        )
    }

    #[test]
    fn drag_off_bar_unregisters_source_slot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut hotbar = Hotbar::default();
        hotbar.assign(2, HotbarSlot::Skill(10));
        app.insert_resource(hotbar);
        app.insert_resource(HotbarDrag {
            payload: Some(HotbarSlot::Skill(10)),
            source: Some(2),
            dropped_on_slot: false,
        });
        app.add_observer(reset_drag);
        let target = app.world_mut().spawn_empty().id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(drag_end_event(target, window));
        app.update();

        assert_eq!(app.world().resource::<Hotbar>().get(2), None);
        assert!(app.world().resource::<HotbarDrag>().source.is_none());
    }

    #[test]
    fn drag_dropped_on_slot_keeps_source_slot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut hotbar = Hotbar::default();
        hotbar.assign(2, HotbarSlot::Skill(10));
        app.insert_resource(hotbar);
        app.insert_resource(HotbarDrag {
            payload: Some(HotbarSlot::Skill(10)),
            source: Some(2),
            dropped_on_slot: true,
        });
        app.add_observer(reset_drag);
        let target = app.world_mut().spawn_empty().id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(drag_end_event(target, window));
        app.update();

        assert_eq!(
            app.world().resource::<Hotbar>().get(2),
            Some(HotbarSlot::Skill(10))
        );
    }

    #[test]
    fn hotbar_drag_resource_is_registered() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(HotbarWidgetPlugin);
        assert!(app.world().contains_resource::<HotbarDrag>());
    }

    fn ghost_db() -> ItemDb {
        use lifthrasir_data::{ItemData, ItemInfo};
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn ghost_icon_skill_without_catalog_is_none() {
        let icon = ghost_icon(HotbarSlot::Skill(42), &Inventory::default(), None, None);
        assert_eq!(icon, None);
    }

    #[test]
    fn ghost_icon_item_without_db_is_none() {
        let icon = ghost_icon(HotbarSlot::Item(501), &inventory_with(501, 3), None, None);
        assert_eq!(icon, None);
    }

    #[test]
    fn ghost_icon_item_resolves_from_db() {
        let db = ghost_db();
        let icon = ghost_icon(
            HotbarSlot::Item(501),
            &inventory_with(501, 3),
            None,
            Some(&db),
        );
        assert!(
            icon.is_some_and(|path| path.ends_with("RED_POTION.bmp")),
            "item ghost should resolve to the identified icon path"
        );
    }

    fn skill_catalog() -> SkillCatalog {
        use lifthrasir_data::{SkillData, SkillMeta};
        let mut data = SkillData::default();
        data.skills.insert(
            5,
            SkillMeta {
                name: "SM_BASH".to_string(),
                display_name: "Bash".to_string(),
                description: vec![],
                max_level: 10,
                sp_cost: vec![8],
                attack_range: vec![1],
            },
        );
        SkillCatalog::from_skill_data(data)
    }

    #[test]
    fn slot_label_empty_is_none() {
        assert_eq!(slot_label(None, &Inventory::default(), None, None), None);
    }

    #[test]
    fn slot_label_skill_uses_display_name() {
        let catalog = skill_catalog();
        let label = slot_label(
            Some(HotbarSlot::Skill(5)),
            &Inventory::default(),
            Some(&catalog),
            None,
        );
        assert_eq!(label.as_deref(), Some("Bash"));
    }

    #[test]
    fn slot_label_skill_without_catalog_is_none() {
        let label = slot_label(
            Some(HotbarSlot::Skill(5)),
            &Inventory::default(),
            None,
            None,
        );
        assert_eq!(label, None);
    }

    #[test]
    fn slot_label_item_resolves_name() {
        let db = ghost_db();
        let label = slot_label(
            Some(HotbarSlot::Item(501)),
            &inventory_with(501, 3),
            None,
            Some(&db),
        );
        assert_eq!(label.as_deref(), Some("Red Potion"));
    }

    #[test]
    fn slot_label_item_without_db_is_none() {
        let label = slot_label(
            Some(HotbarSlot::Item(501)),
            &inventory_with(501, 3),
            None,
            None,
        );
        assert_eq!(label, None);
    }
}
