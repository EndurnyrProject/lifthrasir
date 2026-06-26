//! Hotbar: a fixed, always-visible 12-slot quick-bar pinned bottom-center.
//!
//! Renders the `Hotbar` resource every frame (skill/item/empty styling, icon,
//! stack count, and a cooldown overlay + seconds for skills) and activates a
//! filled slot on click by writing `HotbarSlotActivated` — the same seam the
//! F1..F12 keys use (Task 4).
//!
//! Slots are also `bevy_picking` drag targets: dropping a `SkillCell` /
//! `InventoryCell` assigns it, dropping another slot swaps the two, and a
//! right-click clears. The dragged payload (a skill/item) is carried in the
//! `HotbarDrag` resource, set by the source cells on `DragStart` and reset on
//! `DragEnd`; a slot↔slot swap is detected from the `DragDrop` dragged entity.
//!
//! `SkillCooldownTracker` only exposes the remaining seconds (not the original
//! duration), so the cooldown render is a darkening overlay plus the rounded-up
//! seconds rather than a proportional sweep (design D7's accepted fallback).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_engine::core::state::GameState;
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::hotbar::{Hotbar, HotbarSlot, HotbarSlotActivated};
use game_engine::domain::inventory::Inventory;
use game_engine::domain::skill::SkillCooldownTracker;
use game_engine::infrastructure::item::ItemDb;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;

const SLOTS: usize = 12;
const SLOT_SIZE: f32 = 37.0;
const ICON_SIZE: f32 = 21.0;
const ICON_INSET: f32 = (SLOT_SIZE - ICON_SIZE) / 2.0;

/// The cursor-following drag ghost: a translucent icon shown while a skill/item is
/// being dragged. Sits above every window (`GHOST_Z`) and is `Pickable::IGNORE` so
/// it never steals the drop target's hit.
const GHOST_SIZE: f32 = 30.0;
const GHOST_Z: i32 = 2000;
const GHOST_ALPHA: f32 = 0.85;

const BAR_BG: Color = Color::srgba(0.043, 0.067, 0.059, 0.78);
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

/// The skill/item currently being dragged onto the bar. Set by the source cells
/// (skill window, inventory, or a filled slot) on `DragStart`, consumed by a
/// slot's `DragDrop`, and reset on `DragEnd`.
#[derive(Resource, Default)]
pub struct HotbarDrag {
    pub payload: Option<HotbarSlot>,
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

/// Builds the bottom-center bar under `parent`: a centered row of 12 cells, each
/// with an F-key label, icon, cooldown overlay + seconds, and a stack count.
pub fn spawn_hotbar(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font = asset_server.load(theme::FONT_BODY);

    let wrapper = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(14.0),
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
                    top: Val::Px(5.0),
                    bottom: Val::Px(7.0),
                },
                border: UiRect {
                    bottom: Val::Px(1.0),
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
}

/// Reflects the bar state into every cell, writing each node only on change so a
/// cooling-down skill (this system runs each frame) doesn't churn the others.
#[allow(clippy::too_many_arguments)]
fn update_hotbar(
    hotbar: Res<Hotbar>,
    inventory: Res<Inventory>,
    catalog: Option<Res<SkillCatalog>>,
    item_db: Option<Res<ItemDb>>,
    cooldowns: Res<SkillCooldownTracker>,
    asset_server: Res<AssetServer>,
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
                &inventory,
                catalog.as_deref(),
                item_db.as_deref(),
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
            let handle = asset_server.load(path);
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
    }
}

/// Drop target: a dropped slot swaps, anything else places the carried payload.
// NOTE: drag-hover highlight deferred — `update_hotbar` repaints the border
// every frame, so a `.drag` hover tint would need it to read picking hover state.
fn on_slot_drag_drop(
    drop: On<Pointer<DragDrop>>,
    cells: Query<&HotbarSlotUi>,
    state: Res<HotbarDrag>,
    mut hotbar: ResMut<Hotbar>,
) {
    let Ok(target) = cells.get(drop.entity) else {
        return;
    };
    let dragged_slot = cells.get(drop.dropped).ok().map(|c| c.0);
    let Some(dropped) = drop_source(dragged_slot, state.payload) else {
        return;
    };
    apply_drop(&mut hotbar, target.0, dropped);
}

/// Clears the carried payload once any drag finishes.
fn reset_drag(_: On<Pointer<DragEnd>>, mut state: ResMut<HotbarDrag>) {
    state.payload = None;
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
#[allow(clippy::too_many_arguments)]
fn update_drag_ghost(
    mut commands: Commands,
    drag: Res<HotbarDrag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inventory: Res<Inventory>,
    catalog: Option<Res<SkillCatalog>>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    mut ghosts: Query<(Entity, &mut Node), With<HotbarDragGhost>>,
) {
    let ghost = ghosts.single_mut().ok();
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let icon = drag
        .payload
        .and_then(|p| ghost_icon(p, &inventory, catalog.as_deref(), item_db.as_deref()));

    let (Some(cursor), Some(icon)) = (cursor, icon) else {
        if let Some((entity, _)) = ghost {
            commands.entity(entity).despawn();
        }
        return;
    };

    let left = Val::Px(cursor.x - GHOST_SIZE / 2.0);
    let top = Val::Px(cursor.y - GHOST_SIZE / 2.0);
    match ghost {
        Some((_, mut node)) => {
            node.left = left;
            node.top = top;
        }
        None => spawn_ghost(&mut commands, &asset_server, icon, left, top),
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
        use game_engine::infrastructure::networking::zone_messages::SkillCooldownSet;

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
}
