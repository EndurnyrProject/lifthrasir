//! The hotbar pinned bottom-centre of the HUD, after the `Hotbar` artboard of the
//! Lifthrasir Widgets design: SP and AP, one or two rows of twelve slots out of a bar
//! of three pages, and the pager with the row and lock switches.
//!
//! The slot cells are spawned once and never respawned, so a hover or a drag outlives
//! every redraw. [`sync_slots`] redraws a cell's [`SlotBody`] when its
//! [`view::SlotView`] changes, [`animate`] and [`play_fx`] move what changes every
//! frame, and [`sync_panel`] respawns the [`Region`]s around the cells: the vitals,
//! the buttons, the tooltip, the picker and the toast.
//!
//! A click uses a filled slot through `HotbarSlotActivated`, the seam the slot keys
//! use, and opens the picker on an empty one; a right-click empties a slot. Slots are
//! also `bevy_picking` drop targets: a `SkillCell` / `InventoryCell` dropped on one
//! is assigned, another slot swaps with it, and a slot dragged off the bar is
//! removed. The dragged skill or item travels in [`HotbarDrag`], set by the source
//! on `DragStart`.

use bevy::color::Mix;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::job::player_jobs::is_fourth_job;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::entities::character::components::core::CharacterData;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::character::states::status_icons::LocalStatuses;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::hotbar::{Hotbar, HotbarSlot, HotbarSlotActivated};
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_client::domain::settings::Keybinds;
use lifthrasir_client::domain::skill::{SkillCastLevels, SkillCooldownTracker, SkillTreeState};
use lifthrasir_ui::theme::ro_dark;
use net_contract::events::{CastCancelled, SkillCastStarted};

mod scene;
mod view;

#[cfg(test)]
mod tests;

use crate::screens::shared::{glow, keycap_label};
use crate::widgets::skills_window::skill_name;
use crate::worldspace::viewport_to_ui;
use view::{Edge, Facts, Filled, Live, Notice, PickTab, SlotView};

pub(crate) const HOTBAR_BOTTOM: f32 = 14.0;
/// The plate with both rows on show. The HUD keeps this much clear above
/// [`HOTBAR_BOTTOM`].
pub(crate) const HOTBAR_HEIGHT: f32 = scene::PLATE_HEIGHT;

/// The cursor-following drag ghost: a translucent icon shown while a skill/item is
/// being dragged. Sits above every window (`GHOST_Z`) and is `Pickable::IGNORE` so
/// it never steals the drop target's hit.
const GHOST_SIZE: f32 = 34.0;
const GHOST_Z: i32 = 2000;
const GHOST_ALPHA: f32 = 0.85;

/// How long a toast stays, and how long one that can be undone does.
const TOAST_SECS: f32 = 2.2;
const UNDO_SECS: f32 = 4.0;
/// SP and AP bars of the vitals.
const SP_BAR: Color = Color::srgb_u8(0x4a, 0x8a, 0xc8);
const AP_BAR: Color = Color::srgb_u8(0xe0, 0xb2, 0x57);

/// A slot's cell: `col` of the shown `row`. Which slot of the bar that is follows the
/// page on show.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotbarSlotUi {
    pub row: usize,
    pub col: usize,
}

/// What a cell last drew, so it is redrawn only when that changes.
#[derive(Component, Default, Clone)]
struct Drawn {
    view: Option<SlotView>,
    edge: Option<Edge>,
}

/// The part of a cell that is respawned when the slot's view changes.
#[derive(Component, Default, Clone)]
struct SlotBody;

/// The second row of cells, hidden while the bar shows one page.
#[derive(Component, Default, Clone)]
struct SecondRow;

/// The cooldown wedge over the icon of this skill.
#[derive(Component, Default, Clone)]
struct Sweep(u32);

/// The cast bar along the bottom edge of the slot being cast.
#[derive(Component, Default, Clone)]
struct CastFill;

/// Covers the screen while the picker is open, and closes it when clicked.
#[derive(Component, Default, Clone)]
struct Backdrop;

/// A part of the plate that [`sync_panel`] respawns.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Region {
    #[default]
    Vitals,
    Buttons,
    Tip,
    Picker,
    Toast,
}

/// What a button of the bar does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
enum HotbarAction {
    #[default]
    PagePrev,
    PageNext,
    ToggleRows,
    ToggleLock,
    Undo,
    ClosePicker,
    Tab(PickTab),
    Pick(HotbarSlot),
}

/// A brief effect on a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FxKind {
    /// The slot was used.
    Press,
    /// The slot was used and nothing can happen.
    Refuse,
    /// The slot's skill came off cooldown.
    Ready,
}

impl FxKind {
    fn secs(self) -> f32 {
        match self {
            Self::Press => 0.25,
            Self::Refuse => 0.35,
            Self::Ready => 0.5,
        }
    }
}

/// The effect playing on a cell and the seconds it has run.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Fx {
    kind: FxKind,
    age: f32,
}

impl Fx {
    fn new(kind: FxKind) -> Self {
        Self { kind, age: 0.0 }
    }
}

/// The list an empty slot is being filled from.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Picker {
    row: usize,
    col: usize,
    /// The slot of the bar it fills.
    index: usize,
    tab: PickTab,
}

#[derive(Clone, Debug, PartialEq)]
struct Toast {
    notice: Notice,
    /// The slot to refill if the removal is undone.
    undo: Option<(usize, HotbarSlot)>,
    /// Seconds until it goes away.
    left: f32,
}

/// The skill the hero is casting and how far the cast has come.
struct Casting {
    skill_id: u32,
    timer: Timer,
}

/// What the bar is showing besides its slots.
#[derive(Resource, Default)]
struct HotbarUi {
    /// The cell under the pointer.
    hover: Option<Entity>,
    /// The cell a drag would drop on.
    drop_target: Option<Entity>,
    picker: Option<Picker>,
    toast: Option<Toast>,
    casting: Option<Casting>,
}

impl HotbarUi {
    fn say(&mut self, notice: Notice) {
        self.toast = Some(Toast {
            notice,
            undo: None,
            left: TOAST_SECS,
        });
    }

    /// Says a slot was emptied and offers to refill it. `cell` is where it sat on
    /// show, if it was not dragged away.
    fn say_removed(&mut self, index: usize, slot: HotbarSlot, cell: Option<(usize, usize)>) {
        self.toast = Some(Toast {
            notice: Notice::Removed(slot, cell),
            undo: Some((index, slot)),
            left: UNDO_SECS,
        });
    }
}

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
        app.init_resource::<HotbarDrag>()
            .init_resource::<HotbarUi>()
            .add_observer(reset_drag)
            .add_systems(
                Update,
                (
                    (track_cast, age_toast, answer_use, sync_slots, play_fx).chain(),
                    animate,
                    sync_panel,
                    update_drag_ghost,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// Mounts the hotbar on the HUD.
pub fn spawn_hotbar(commands: &mut Commands, hud: Entity) {
    commands.spawn_scene(scene::root()).insert(ChildOf(hud));
}

/// What the slots are read from, besides the bar itself.
#[derive(SystemParam)]
struct Sources<'w, 's> {
    inventory: Res<'w, Inventory>,
    catalog: Option<Res<'w, SkillCatalog>>,
    items: Option<Res<'w, ItemDb>>,
    tree: Res<'w, SkillTreeState>,
    cast_levels: Res<'w, SkillCastLevels>,
    cooldowns: Res<'w, SkillCooldownTracker>,
    statuses: Res<'w, LocalStatuses>,
    keybinds: Option<Res<'w, Keybinds>>,
    time: Res<'w, Time>,
    real_time: Option<Res<'w, Time<Real>>>,
    companion: Option<Res<'w, lifthrasir_client::domain::homunculus::HomunculusState>>,
    hero: Query<'w, 's, (&'static CharacterStatus, &'static CharacterData), With<LocalPlayer>>,
}

impl Sources<'_, '_> {
    fn facts(&self) -> Facts<'_> {
        Facts {
            inventory: &self.inventory,
            catalog: self.catalog.as_deref(),
            items: self.items.as_deref(),
            tree: &self.tree,
            cast_levels: &self.cast_levels,
            sp: self.hero.single().map_or(u32::MAX, |(status, _)| status.sp),
        }
    }

    /// The cooldown and the timed status of a skill, and whether it is `casting`.
    fn live(&self, slot: Option<HotbarSlot>, casting: Option<u32>) -> Live {
        let Some(HotbarSlot::Skill(id)) = slot else {
            return Live::default();
        };
        let now = self.time.elapsed();
        Live {
            cooldown: self.cooldowns.remaining_secs(id),
            buff: self
                .statuses
                .active
                .values()
                .filter(|status| status.source_skill_id == Some(id))
                .filter_map(|status| status.expires_at)
                .map(|at| at.saturating_sub(now))
                .max(),
            casting: casting == Some(id),
        }
    }

    /// The key that uses the cell at `col` of `row`: the slot's bind, after `S+` on
    /// the second row, which Shift fires.
    fn key(&self, row: usize, col: usize) -> String {
        let bind = self
            .keybinds
            .as_deref()
            .and_then(|keybinds| keybinds.hotbar.get(col))
            .map(|binds| &binds.primary)
            .filter(|bind| bind.is_bound());
        match (bind, row) {
            (None, _) => String::new(),
            (Some(bind), 0) => keycap_label(bind),
            (Some(bind), _) => format!("S+{}", keycap_label(bind)),
        }
    }

    fn slot(&self, hotbar: &Hotbar, cell: HotbarSlotUi, casting: Option<u32>) -> SlotView {
        let slot = hotbar.get(hotbar.index(cell.row, cell.col));
        if let Some(HotbarSlot::HomunculusSkill(id)) = slot {
            return SlotView {
                key: self.key(cell.row, cell.col),
                filled: Some(self.filled(HotbarSlot::HomunculusSkill(id))),
            };
        }
        view::slot_view(
            slot,
            self.key(cell.row, cell.col),
            &self.facts(),
            self.live(slot, casting),
        )
    }

    /// The skill or item as a slot shows it at rest.
    fn filled(&self, slot: HotbarSlot) -> Filled {
        if let HotbarSlot::HomunculusSkill(id) = slot {
            let live = self
                .companion
                .as_deref()
                .filter(|s| s.controlled_gid().is_some());
            let level = live
                .and_then(|s| {
                    s.snapshot()?
                        .skills
                        .iter()
                        .find(|skill| skill.skill_id == id && skill.level > 0)
                })
                .map(|s| s.level);
            let cooldown = live
                .and_then(|s| {
                    s.snapshot()?
                        .cooldowns
                        .iter()
                        .find(|c| c.skill_id == id)
                        .map(|c| {
                            s.remaining(
                                c.remaining_ms,
                                self.real_time.as_ref().map_or_default(|t| t.elapsed()),
                            )
                        })
                })
                .filter(|ms| *ms > 0)
                .map(|ms| ms as f32 / 1000.0);
            return view::companion_skill(id, self.catalog.as_deref(), level, cooldown);
        }
        view::filled(slot, &self.facts(), Live::default())
    }

    /// SP, and AP for the jobs that build it.
    fn vitals(&self) -> ((u32, u32), Option<(u32, u32)>) {
        let Ok((status, data)) = self.hero.single() else {
            return ((0, 0), None);
        };
        let has_ap = is_fourth_job(data.job_id.into()) && status.max_ap > 0;
        (
            (status.sp, status.max_sp),
            has_ap.then_some((status.ap, status.max_ap)),
        )
    }
}

/// Follows the hero's cast, for the cast bar on its slot.
fn track_cast(
    time: Res<Time>,
    mut started: MessageReader<SkillCastStarted>,
    mut cancelled: MessageReader<CastCancelled>,
    registry: Res<EntityRegistry>,
    mut ui: ResMut<HotbarUi>,
) {
    let hero = registry.local_player_entity();
    let is_hero = |id: u32| hero.is_some() && registry.get_entity(id) == hero;
    for cast in started.read() {
        if cast.cast_time == 0 || !is_hero(cast.src_id) {
            continue;
        }
        ui.casting = Some(Casting {
            skill_id: cast.skill_id,
            timer: Timer::from_seconds(cast.cast_time as f32 / 1000.0, TimerMode::Once),
        });
    }
    if cancelled.read().filter(|cast| is_hero(cast.gid)).count() > 0 {
        ui.casting = None;
    }
    let Some(casting) = &mut ui.casting else {
        return;
    };
    if casting.timer.tick(time.delta()).is_finished() {
        ui.casting = None;
    }
}

fn age_toast(time: Res<Time>, mut ui: ResMut<HotbarUi>) {
    let Some(toast) = &mut ui.toast else {
        return;
    };
    toast.left -= time.delta_secs();
    if toast.left <= 0.0 {
        ui.toast = None;
    }
}

/// Answers a slot being used, by key or by click: its cell is pressed, or shakes
/// while the toast says why nothing happens.
fn answer_use(
    mut commands: Commands,
    mut used: MessageReader<HotbarSlotActivated>,
    hotbar: Res<Hotbar>,
    sources: Sources,
    mut ui: ResMut<HotbarUi>,
    cells: Query<(Entity, &HotbarSlotUi)>,
) {
    for use_ in used.read() {
        let shown = cells.iter().find(|(_, cell)| {
            cell.row < hotbar.rows() && hotbar.index(cell.row, cell.col) == use_.index
        });
        let Some((entity, cell)) = shown else {
            continue;
        };
        let casting = ui.casting.as_ref().map(|casting| casting.skill_id);
        let Some(filled) = sources.slot(&hotbar, *cell, casting).filled else {
            continue;
        };
        let casting = casting.map(|id| skill_name(id, sources.catalog.as_deref()));
        let kind = match view::refusal(&filled, casting.as_deref()) {
            Some(why) => {
                ui.say(Notice::Refused(why));
                FxKind::Refuse
            }
            None => FxKind::Press,
        };
        commands.entity(entity).insert(Fx::new(kind));
    }
}

type Cell<'a> = (
    Entity,
    &'a HotbarSlotUi,
    &'a mut Drawn,
    &'a mut BorderColor,
    &'a mut BoxShadow,
    Option<&'a Children>,
);

/// Shows the second row while the bar has two, restyles the outline of a cell whose
/// state changed and redraws the body of one whose view did.
fn sync_slots(
    mut commands: Commands,
    hotbar: Res<Hotbar>,
    sources: Sources,
    ui: Res<HotbarUi>,
    mut cells: Query<Cell>,
    bodies: Query<(), With<SlotBody>>,
    mut second_rows: Query<&mut Node, With<SecondRow>>,
) {
    let display = if hotbar.two_rows {
        Display::Flex
    } else {
        Display::None
    };
    for mut row in &mut second_rows {
        if row.display != display {
            row.display = display;
        }
    }

    let casting = ui.casting.as_ref().map(|casting| casting.skill_id);
    for (entity, cell, mut drawn, mut border, mut shadow, children) in &mut cells {
        let view = sources.slot(&hotbar, *cell, casting);
        let edge = view::edge(
            &view,
            ui.hover == Some(entity),
            ui.drop_target == Some(entity),
        );
        if drawn.edge != Some(edge) {
            let (stroke, halo) = edge.style();
            *border = BorderColor::all(stroke);
            *shadow = halo;
            drawn.edge = Some(edge);
        }
        if drawn.view.as_ref() == Some(&view) {
            continue;
        }
        if view::cooled_down(drawn.view.as_ref(), &view) {
            commands.entity(entity).insert(Fx::new(FxKind::Ready));
        }
        for &child in children.into_iter().flatten() {
            if bodies.contains(child) {
                commands.entity(child).despawn();
            }
        }
        commands
            .spawn_scene(scene::body(view.clone()))
            .insert(ChildOf(entity));
        drawn.view = Some(view);
    }
}

/// Turns the cooldown wedges and grows the cast bar.
fn animate(
    cooldowns: Res<SkillCooldownTracker>,
    ui: Res<HotbarUi>,
    mut sweeps: Query<(&Sweep, &mut BackgroundGradient)>,
    mut cast_fills: Query<&mut Node, With<CastFill>>,
) {
    for (sweep, mut wedge) in &mut sweeps {
        *wedge = scene::sweep(cooldowns.fraction_remaining(sweep.0).unwrap_or(0.0));
    }
    let cast = ui
        .casting
        .as_ref()
        .map_or(0.0, |casting| casting.timer.fraction());
    for mut fill in &mut cast_fills {
        fill.width = Val::Percent(cast * 100.0);
    }
}

type FxCell<'a> = (
    Entity,
    &'a mut Fx,
    &'a mut UiTransform,
    &'a mut BorderColor,
    &'a mut BoxShadow,
    &'a mut Drawn,
);

/// Plays the effect on each cell that has one: a press springs back, a refusal
/// shakes and a skill coming off cooldown flashes white. When it ends the cell's
/// outline is styled afresh.
fn play_fx(time: Res<Time>, mut commands: Commands, mut cells: Query<FxCell>) {
    for (entity, mut fx, mut transform, mut border, mut shadow, mut drawn) in &mut cells {
        fx.age += time.delta_secs();
        let t = fx.age / fx.kind.secs();
        *transform = UiTransform::IDENTITY;
        if t >= 1.0 {
            drawn.edge = None;
            commands.entity(entity).remove::<Fx>();
            continue;
        }
        match fx.kind {
            FxKind::Press => transform.scale = Vec2::splat(0.9 + 0.1 * t),
            FxKind::Refuse => {
                let sway = (t * std::f32::consts::TAU * 2.0).sin() * 3.0 * (1.0 - t);
                transform.translation = Val2::px(sway, 0.0);
            }
            FxKind::Ready => {
                let flash = 1.0 - t;
                *border = BorderColor::all(ro_dark::SLOT_STROKE.mix(&ro_dark::STRONG, flash));
                *shadow = glow(Color::WHITE.with_alpha(0.7 * flash));
            }
        }
    }
}

/// What the regions around the cells show.
#[derive(Default, PartialEq)]
struct PanelView {
    sp: (u32, u32),
    ap: Option<(u32, u32)>,
    pages: String,
    two_rows: bool,
    locked: bool,
    tip: Option<(view::TipView, Vec2)>,
    picker: Option<(String, PickTab, Vec<view::Choice>, Vec2)>,
    toast: Option<(String, view::Tone, bool)>,
}

impl PanelView {
    fn differs(&self, other: &Self, region: Region) -> bool {
        match region {
            Region::Vitals => (self.sp, self.ap) != (other.sp, other.ap),
            Region::Buttons => {
                (&self.pages, self.two_rows, self.locked)
                    != (&other.pages, other.two_rows, other.locked)
            }
            Region::Tip => self.tip != other.tip,
            Region::Picker => self.picker != other.picker,
            Region::Toast => self.toast != other.toast,
        }
    }
}

/// Where the panel's popups come from: the bar's state and the drag in flight.
#[derive(SystemParam)]
struct PanelState<'w, 's> {
    hotbar: Res<'w, Hotbar>,
    ui: Res<'w, HotbarUi>,
    drag: Res<'w, HotbarDrag>,
    cells: Query<'w, 's, &'static HotbarSlotUi>,
}

fn panel_view(sources: &Sources, state: &PanelState) -> PanelView {
    let (hotbar, ui) = (&*state.hotbar, &*state.ui);
    let rows = hotbar.rows();
    let casting = ui.casting.as_ref().map(|casting| casting.skill_id);
    let (sp, ap) = sources.vitals();

    let hovered = ui
        .hover
        .filter(|_| ui.picker.is_none() && state.drag.payload.is_none())
        .and_then(|entity| state.cells.get(entity).ok())
        .filter(|cell| cell.row < rows);
    let tip = hovered.and_then(|cell| {
        let tip = view::tip(&sources.slot(hotbar, *cell, casting), hotbar.locked)?;
        Some((tip, scene::anchor(cell.row, cell.col, rows)))
    });

    let picker = ui.picker.map(|picker| {
        (
            sources.key(picker.row, picker.col),
            picker.tab,
            view::choices(picker.tab, hotbar, &sources.facts()),
            scene::anchor(picker.row, picker.col, rows),
        )
    });

    let toast = ui.toast.as_ref().map(|toast| {
        let (name, key) = toast.notice.subject().map_or_else(default, |(slot, cell)| {
            let key = cell.map(|(row, col)| sources.key(row, col));
            (sources.filled(slot).name, key.unwrap_or_default())
        });
        (
            toast.notice.text(&name, &key),
            toast.notice.tone(),
            toast.undo.is_some(),
        )
    });

    PanelView {
        sp,
        ap,
        pages: view::page_label(hotbar),
        two_rows: hotbar.two_rows,
        locked: hotbar.locked,
        tip,
        picker,
        toast,
    }
}

/// Respawns the regions whose view changed, and lays the backdrop out while the
/// picker is open.
fn sync_panel(
    mut commands: Commands,
    sources: Sources,
    state: PanelState,
    regions: Query<(Entity, Ref<Region>, Option<&Children>)>,
    mut backdrops: Query<&mut Node, With<Backdrop>>,
    mut shown: Local<PanelView>,
) {
    let now = panel_view(&sources, &state);
    for (host, region, children) in &regions {
        if !region.is_added() && !now.differs(&shown, *region) {
            continue;
        }
        for &child in children.into_iter().flatten() {
            commands.entity(child).despawn();
        }
        let mut hang = |scene: Box<dyn Scene>| {
            commands.spawn_scene(scene).insert(ChildOf(host));
        };
        match *region {
            Region::Vitals => {
                hang(Box::new(scene::vital("SP", view::SP_INK, SP_BAR, now.sp)));
                if let Some(ap) = now.ap {
                    hang(Box::new(scene::vital(
                        "AP",
                        ro_dark::BRONZE_LIGHT,
                        AP_BAR,
                        ap,
                    )));
                }
            }
            Region::Buttons => hang(Box::new(scene::buttons(
                now.pages.clone(),
                now.two_rows,
                now.locked,
            ))),
            Region::Tip => {
                if let Some((tip, at)) = now.tip.clone() {
                    hang(Box::new(scene::tip(tip, at)));
                }
            }
            Region::Picker => {
                if let Some((key, tab, choices, at)) = now.picker.clone() {
                    hang(Box::new(scene::picker(key, tab, choices, at)));
                }
            }
            Region::Toast => {
                if let Some((text, tone, undo)) = now.toast.clone() {
                    hang(Box::new(scene::toast(text, tone, undo)));
                }
            }
        }
    }

    let display = if now.picker.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for mut backdrop in &mut backdrops {
        if backdrop.display != display {
            backdrop.display = display;
        }
    }
    *shown = now;
}

fn on_action(
    activate: On<Activate>,
    actions: Query<&HotbarAction>,
    mut hotbar: ResMut<Hotbar>,
    mut ui: ResMut<HotbarUi>,
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        HotbarAction::PagePrev | HotbarAction::PageNext => {
            hotbar.turn(action == HotbarAction::PageNext);
            ui.picker = None;
        }
        HotbarAction::ToggleRows => {
            hotbar.two_rows = !hotbar.two_rows;
            ui.picker = None;
            ui.hover = None;
        }
        HotbarAction::ToggleLock => {
            hotbar.locked = !hotbar.locked;
            ui.say(Notice::LockSet(hotbar.locked));
        }
        HotbarAction::Undo => {
            if let Some((index, slot)) = ui.toast.take().and_then(|toast| toast.undo) {
                hotbar.assign(index, slot);
            }
        }
        HotbarAction::ClosePicker => ui.picker = None,
        HotbarAction::Tab(tab) => {
            if let Some(picker) = &mut ui.picker {
                picker.tab = tab;
            }
        }
        HotbarAction::Pick(slot) => {
            let Some(picker) = ui.picker.take() else {
                return;
            };
            hotbar.assign(picker.index, slot);
            ui.say(Notice::Placed(slot, (picker.row, picker.col)));
        }
    }
}

/// A click uses a filled slot through the shared seam and opens the picker on an
/// empty one; a right-click empties a filled slot. A locked bar is only used.
fn on_slot_click(
    click: On<PointerClick>,
    cells: Query<&HotbarSlotUi>,
    mut hotbar: ResMut<Hotbar>,
    mut ui: ResMut<HotbarUi>,
    mut activated: MessageWriter<HotbarSlotActivated>,
) {
    let Ok(&HotbarSlotUi { row, col }) = cells.get(click.entity) else {
        return;
    };
    let index = hotbar.index(row, col);
    match (click.button, hotbar.get(index)) {
        (PointerButton::Primary, Some(_)) => {
            ui.picker = None;
            activated.write(HotbarSlotActivated { index });
        }
        (PointerButton::Primary, None) | (PointerButton::Secondary, Some(_)) if hotbar.locked => {
            ui.say(Notice::Locked);
        }
        (PointerButton::Primary, None) => {
            ui.picker = Some(Picker {
                row,
                col,
                index,
                tab: PickTab::default(),
            });
        }
        (PointerButton::Secondary, Some(slot)) => {
            hotbar.clear(index);
            ui.say_removed(index, slot, Some((row, col)));
        }
        _ => {}
    }
}

fn on_slot_over(over: On<PointerOver>, mut ui: ResMut<HotbarUi>) {
    ui.hover = Some(over.entity);
}

fn on_slot_out(out: On<PointerOut>, mut ui: ResMut<HotbarUi>) {
    if ui.hover == Some(out.entity) {
        ui.hover = None;
    }
}

/// A filled slot is a drag source: it carries what it holds and where it sat. A
/// locked bar lets nothing go.
fn on_slot_drag_start(
    drag: On<PointerDragStart>,
    cells: Query<&HotbarSlotUi>,
    hotbar: Res<Hotbar>,
    mut state: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    if hotbar.locked {
        return;
    }
    let index = hotbar.index(cell.row, cell.col);
    if let Some(slot) = hotbar.get(index) {
        state.payload = Some(slot);
        state.source = Some(index);
    }
}

fn on_slot_drag_enter(
    enter: On<PointerDragEnter>,
    drag: Res<HotbarDrag>,
    mut ui: ResMut<HotbarUi>,
) {
    if drag.payload.is_some() {
        ui.drop_target = Some(enter.entity);
    }
}

fn on_slot_drag_leave(leave: On<PointerDragLeave>, mut ui: ResMut<HotbarUi>) {
    if ui.drop_target == Some(leave.entity) {
        ui.drop_target = None;
    }
}

/// A resolved drop: either swap with another slot or place a fresh payload.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DropSource {
    Swap(usize),
    Place(HotbarSlot),
}

/// Classifies a drop: a drag that started on the bar swaps with its source slot,
/// otherwise the carried payload (if any) is placed.
fn drop_source(source: Option<usize>, payload: Option<HotbarSlot>) -> Option<DropSource> {
    match source {
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

/// Drop target: a dragged slot swaps, anything else places the carried payload. A
/// locked bar takes neither.
fn on_slot_drag_drop(
    drop: On<PointerDragDrop>,
    cells: Query<&HotbarSlotUi>,
    mut state: ResMut<HotbarDrag>,
    mut hotbar: ResMut<Hotbar>,
    mut ui: ResMut<HotbarUi>,
) {
    let Ok(cell) = cells.get(drop.entity) else {
        return;
    };
    state.dropped_on_slot = true;
    let Some(dropped) = drop_source(state.source, state.payload) else {
        return;
    };
    if hotbar.locked {
        ui.say(Notice::Locked);
        return;
    }
    let target = hotbar.index(cell.row, cell.col);
    apply_drop(&mut hotbar, target, dropped);
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
fn reset_drag(
    _: On<PointerDragEnd>,
    mut state: ResMut<HotbarDrag>,
    mut hotbar: ResMut<Hotbar>,
    mut ui: ResMut<HotbarUi>,
) {
    let removed = unregister_target(state.source, state.dropped_on_slot)
        .and_then(|index| Some((index, hotbar.get(index)?)));
    if let Some((index, slot)) = removed {
        hotbar.clear(index);
        ui.say_removed(index, slot, None);
    }
    *state = HotbarDrag::default();
    ui.drop_target = None;
}

/// Drives the cursor-following drag ghost: spawns it on the first frame a drag
/// carries a resolvable icon, tracks the cursor while the drag is live, and
/// despawns it once the payload clears (or its icon can no longer be resolved).
fn update_drag_ghost(
    mut commands: Commands,
    drag: Res<HotbarDrag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    sources: Sources,
    asset_server: Res<AssetServer>,
    mut ghosts: Query<(Entity, &mut Node), With<HotbarDragGhost>>,
) {
    let ghost = ghosts.single_mut().ok();
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let icon = drag.payload.and_then(|slot| sources.filled(slot).icon);

    let (Some(cursor), Some(icon)) = (cursor, icon) else {
        if let Some((entity, _)) = ghost {
            commands.entity(entity).despawn();
        }
        return;
    };

    let cursor = viewport_to_ui(cursor, &ui_scale);
    let left = Val::Px(cursor.x - GHOST_SIZE / 2.0);
    let top = Val::Px(cursor.y - GHOST_SIZE / 2.0);
    if let Some((_, mut node)) = ghost {
        node.left = left;
        node.top = top;
        return;
    }
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
