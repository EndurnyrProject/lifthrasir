//! The Console's Skills tab: a faithful port of the skill window's body — the
//! branch/job tab strip, the skill grid with learn steppers, the prereq connector
//! layer, the selected-skill info panel, and the Skill-Points footer with the
//! Reset/Apply commit buttons — projected into the shell's [`SkillsTabBody`]
//! container instead of a standalone window.
//!
//! This file is deliberately self-contained: it defines its OWN UI-only types
//! ([`SkillPanelUi`], [`SkillPanelStaging`], [`SkillPanelTab`], [`SkillPanelCell`],
//! [`SkillPanelStepper`], [`SkillPanelCommitButton`], [`SkillPanelBank`],
//! [`LastSkillPanelClick`]) and the UI-only staging/ordering helpers ([`apply_order`],
//! [`SkillPanelStaging::can_raise`], `tab_ids`, `tab_label`, ...) so `skill_window`
//! can be deleted wholesale in the integration task with zero dangling references. It
//! reuses only the shared DOMAIN types + messages (`SkillTreeState`, `SkillNode`,
//! `SkillCatalog`, `layout`/`Placement`, `form`/`target`, `SkillCastRequested`,
//! `SkillLearnRequested`, `JobSpriteRegistry`, `CharacterStatus`,
//! `HotbarDrag`/`HotbarSlot`, `LocalPlayer`) and the chrome/theme helpers.
//!
//! Unlike the old window, the whole body is respawned on tree/ui/staging change (the
//! Bag-tab idiom), so the tab strip, grid, connectors, and info panel are baked from
//! live state each rebuild; only the footer point-bank / commit-dim stays patched in
//! place by [`update_skill_footer`] so it tracks `skill_point` between rebuilds. The
//! prereq connector layer is the one place that keeps imperative geometry (architecture
//! §7): its orthogonal segments are computed in Rust and embedded as `bsn!` children.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::character::events::SkillLearnRequested;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::hotbar::HotbarSlot;
use game_engine::domain::skill::{
    form, layout, target, Form, Placement, SkillCastRequested, SkillTreeState,
};
use game_engine::infrastructure::job::registry::JobSpriteRegistry;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::rich_text::parse_color_codes;
use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};
use crate::widgets::hotbar::HotbarDrag;
use crate::widgets::info_modal::{InfoTarget, ShowInfoModal};

use super::SkillsTabBody;

/// Pixel footprint of one grid cell, matching the old window's `CELL_W`/`CELL_H`.
const CELL_W: f32 = 62.0;
const CELL_H: f32 = 82.0;
/// Icon-centre offset within a cell, used to anchor connector segments.
const IC_X: f32 = 31.0;
const IC_Y: f32 = 26.0;
const TAB_STRIP_W: f32 = 86.0;
const INFO_WIDTH: f32 = 176.0;
/// Fixed height of the body row so the Console never grows with the tree; the grid
/// and info panel scroll internally instead.
const PANE_HEIGHT: f32 = 300.0;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Active tab (a `job_id`) and selected skill. The grid, connectors, and info panel
/// rebuild off changes here.
#[derive(Resource, Default)]
pub struct SkillPanelUi {
    tab: Option<u32>,
    selected: Option<u32>,
}

/// Last cell click, for the double-click cast window (own copy; see module docs).
#[derive(Resource, Default)]
pub struct LastSkillPanelClick {
    skill_id: u32,
    at: Duration,
}

fn is_cast_double_click(last: &LastSkillPanelClick, skill_id: u32, now: Duration) -> bool {
    last.skill_id == skill_id && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

/// Locally staged skill-point spends this session: `skill_id -> staged +levels`.
/// A skill point is a flat 1 per level — no cost curve, unlike the status window.
#[derive(Resource, Default)]
pub struct SkillPanelStaging {
    pending: HashMap<u32, u32>,
}

impl SkillPanelStaging {
    pub fn staged(&self, id: u32) -> u32 {
        self.pending.get(&id).copied().unwrap_or(0)
    }

    pub fn spent(&self) -> u32 {
        self.pending.values().sum()
    }

    pub fn points_left(&self, skill_point: u32) -> u32 {
        skill_point.saturating_sub(self.spent())
    }

    /// Server level plus staged levels — drives live prereq evaluation.
    pub fn effective_level(&self, id: u32, tree: &SkillTreeState) -> u32 {
        let base = tree.skills.get(&id).map(|n| n.level).unwrap_or(0);
        base + self.staged(id)
    }

    /// Fully client-evaluated raise gate: a prereq staged earlier in the same batch
    /// unlocks its dependent immediately.
    pub fn can_raise(
        &self,
        id: u32,
        tree: &SkillTreeState,
        status: &CharacterStatus,
        skill_point: u32,
    ) -> bool {
        let Some(node) = tree.skills.get(&id) else {
            return false;
        };
        if self.points_left(skill_point) == 0 {
            return false;
        }
        if self.effective_level(id, tree) >= node.max_level {
            return false;
        }
        if status.base_level < node.req_base_level || status.job_level < node.req_job_level {
            return false;
        }
        node.requires
            .iter()
            .all(|&(req_id, req_lv)| self.effective_level(req_id, tree) >= req_lv)
    }

    pub fn raise(
        &mut self,
        id: u32,
        tree: &SkillTreeState,
        status: &CharacterStatus,
        skill_point: u32,
    ) {
        if self.can_raise(id, tree, status, skill_point) {
            *self.pending.entry(id).or_insert(0) += 1;
        }
    }

    pub fn lower(&mut self, id: u32) {
        let staged = self.staged(id);
        if staged <= 1 {
            self.pending.remove(&id);
        } else {
            self.pending.insert(id, staged - 1);
        }
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Flattens `pending` into one `skill_id` per staged level, ordered by ascending
/// prerequisite depth over the FULL `requires` graph (cross-tab included), so a
/// staged prereq is always emitted before a staged dependent. Cycle-guarded.
pub fn apply_order(pending: &HashMap<u32, u32>, tree: &SkillTreeState) -> Vec<u32> {
    let mut depths = HashMap::new();
    let mut ids: Vec<u32> = pending.keys().copied().collect();
    ids.sort_unstable_by_key(|&id| {
        (
            prereq_depth(id, tree, &mut depths, &mut Vec::new()).unwrap_or(0),
            id,
        )
    });
    ids.into_iter()
        .flat_map(|id| std::iter::repeat_n(id, pending[&id] as usize))
        .collect()
}

/// Longest prerequisite-chain depth over the full `requires` graph. Returns `None`
/// on a cycle (degrades to depth 0 at the call site).
fn prereq_depth(
    id: u32,
    tree: &SkillTreeState,
    depths: &mut HashMap<u32, u32>,
    stack: &mut Vec<u32>,
) -> Option<u32> {
    if let Some(&d) = depths.get(&id) {
        return Some(d);
    }
    if stack.contains(&id) {
        return None;
    }
    stack.push(id);
    let mut result = Some(0);
    if let Some(node) = tree.skills.get(&id) {
        for &(prereq, _) in &node.requires {
            match prereq_depth(prereq, tree, depths, stack) {
                Some(d) => result = result.map(|r| r.max(d + 1)),
                None => result = None,
            }
        }
    }
    stack.pop();
    if let Some(d) = result {
        depths.insert(id, d);
    }
    result
}

/// Marks a tab button with the `job_id` it selects.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelTab(pub u32);

/// Marks a grid cell with the `skill_id` it shows so clicks can select/cast it.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelCell(pub u32);

/// Marks a `◄`/`►` stepper button with the skill it adjusts and its direction.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelStepper {
    skill_id: u32,
    raise: bool,
}

/// Marks the Reset/Apply footer buttons so their dim-state tracks staging.
#[derive(Component, Default, Clone)]
pub struct SkillPanelCommitButton;

/// Marks the "Skill Points" footer value text.
#[derive(Component, Default, Clone)]
pub struct SkillPanelBank;

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested).
// ---------------------------------------------------------------------------

/// The distinct `job_id`s present in the tree, ascending — one per tab.
fn tab_ids(tree: &SkillTreeState) -> Vec<u32> {
    let mut ids: Vec<u32> = tree.skills.values().map(|n| n.job_id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Tab label for a `job_id`: the job registry's display name, else an ordinal
/// `"Tier N"` (1-based by ascending position among the present tabs).
fn tab_label(job_id: u32, ordinal: usize, registry: Option<&JobSpriteRegistry>) -> String {
    registry
        .and_then(|r| r.try_display_name(job_id))
        .map(str::to_string)
        .unwrap_or_else(|| format!("Tier {}", ordinal + 1))
}

/// The `lv/max` text shown under a cell and in the info panel.
fn format_level(level: u32, max: u32) -> String {
    format!("{level}/{max}")
}

fn cell_icon_color(learned: bool, maxed: bool) -> Color {
    if maxed {
        theme::GOLD
    } else if learned {
        theme::EMERALD_BRI
    } else {
        theme::TEXT_FAINT
    }
}

fn skill_name(skill_id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(skill_id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{skill_id}"))
}

fn form_label(inf: u32) -> String {
    match form(inf) {
        Form::Passive => "Passive",
        Form::Active => "Active",
        Form::Supportive => "Supportive",
    }
    .to_string()
}

fn target_label(inf: u32) -> String {
    use game_engine::domain::skill::Target;
    match target(inf) {
        Target::None => "\u{2014}",
        Target::Enemy => "Enemy",
        Target::Ground => "Ground",
        Target::SelfTarget => "Self",
        Target::Ally => "Ally",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Systems.
// ---------------------------------------------------------------------------

/// Seed the active tab with the first present `job_id` once the tree arrives.
pub fn ensure_default_tab(tree: Res<SkillTreeState>, mut ui: ResMut<SkillPanelUi>) {
    if !tree.is_changed() {
        return;
    }
    let ids = tab_ids(&tree);
    let still_valid = ui.tab.is_some_and(|t| ids.contains(&t));
    if !still_valid {
        ui.tab = ids.first().copied();
        ui.selected = None;
    }
}

/// Rebuilds the [`SkillsTabBody`]'s children on every tree/[`SkillPanelUi`]/
/// [`SkillPanelStaging`] change, and once when the body container is first spawned
/// (the shell mounts it deferred). Despawns the old children and respawns the projected
/// body scene. Mirrors `skill_window`'s `rebuild_grid`/`rebuild_info_panel`/
/// `rebuild_tab_strip` collapsed into one respawn (the Bag-tab idiom).
#[allow(clippy::too_many_arguments)]
pub fn rebuild_skills_body(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillPanelUi>,
    staging: Res<SkillPanelStaging>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    catalog: Option<Res<SkillCatalog>>,
    registry: Option<Res<JobSpriteRegistry>>,
    bodies: Query<(Entity, Option<&Children>, Ref<SkillsTabBody>)>,
) {
    let Ok((body_entity, children, body_ref)) = bodies.single() else {
        return;
    };
    if !tree.is_changed() && !ui.is_changed() && !staging.is_changed() && !body_ref.is_added() {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let status = player.single().ok();
    commands
        .spawn_scene(body(
            &tree,
            &ui,
            &staging,
            status,
            catalog.as_deref(),
            registry.as_deref(),
        ))
        .insert(ChildOf(body_entity));
}

/// Reflects the remaining skill points (server points minus staged spend) into the
/// footer value and dims Reset/Apply when nothing is staged. Change-detected writes,
/// so it tracks `skill_point` even between whole-body rebuilds.
pub fn update_skill_footer(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    staging: Res<SkillPanelStaging>,
    mut bank: Query<&mut Text, With<SkillPanelBank>>,
    mut commit: Query<&mut BackgroundColor, With<SkillPanelCommitButton>>,
) {
    let Ok(status) = player.single() else {
        return;
    };
    if let Ok(mut text) = bank.single_mut() {
        let value = staging.points_left(status.skill_point).to_string();
        if text.0 != value {
            *text = Text::new(value);
        }
    }
    let alpha = if staging.is_empty() { 0.3 } else { 1.0 };
    for mut bg in &mut commit {
        if bg.0.alpha() != alpha {
            bg.0.set_alpha(alpha);
        }
    }
}

/// Reset to the default tab/selection and discard staging when leaving the game.
pub fn reset(mut ui: ResMut<SkillPanelUi>, mut staging: ResMut<SkillPanelStaging>) {
    *ui = SkillPanelUi::default();
    staging.clear();
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// Tab click: set the active tab and clear the current selection.
fn on_tab_click(
    click: On<Pointer<Click>>,
    tabs: Query<&SkillPanelTab>,
    mut ui: ResMut<SkillPanelUi>,
) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = Some(tab.0);
    ui.selected = None;
}

/// `◄`/`►` observer: stages or unstages a level via [`SkillPanelStaging`]. Reads the
/// player status so `can_raise`'s point/level gates are evaluated from the source.
fn on_stepper(
    click: On<Pointer<Click>>,
    steppers: Query<&SkillPanelStepper>,
    tree: Res<SkillTreeState>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<SkillPanelStaging>,
) {
    let Ok(stepper) = steppers.get(click.entity) else {
        return;
    };
    if !stepper.raise {
        staging.lower(stepper.skill_id);
        return;
    }
    let Ok(status) = player.single() else {
        return;
    };
    staging.raise(stepper.skill_id, &tree, status, status.skill_point);
}

/// Cell click: select the skill; a double-click within the cast window emits
/// [`SkillCastRequested`]. Secondary-click opens the info modal instead.
fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&SkillPanelCell>,
    mut ui: ResMut<SkillPanelUi>,
    time: Res<Time>,
    mut last: ResMut<LastSkillPanelClick>,
    mut cast_writer: MessageWriter<SkillCastRequested>,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        info_writer.write(ShowInfoModal {
            target: InfoTarget::Skill(cell.0),
        });
        return;
    }
    ui.selected = Some(cell.0);
    let now = time.elapsed();
    if is_cast_double_click(&last, cell.0, now) {
        cast_writer.write(SkillCastRequested { skill_id: cell.0 });
    }
    *last = LastSkillPanelClick {
        skill_id: cell.0,
        at: now,
    };
}

/// Dragging a skill cell arms the hotbar with that skill so a slot drop assigns it. A
/// plain click still goes through [`on_cell_click`] since `bevy_picking` only emits
/// `DragStart` after a press-and-move.
fn on_cell_drag_start(
    drag: On<Pointer<DragStart>>,
    cells: Query<&SkillPanelCell>,
    mut hotbar_drag: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    hotbar_drag.payload = Some(HotbarSlot::Skill(cell.0));
}

/// Reset: discard all staged levels without contacting the server.
fn on_reset(_: On<Pointer<Click>>, mut staging: ResMut<SkillPanelStaging>) {
    staging.clear();
}

/// Apply: emit one [`SkillLearnRequested`] per staged level in prereq-first order, then
/// clear staging. The resent `SkillList` reconciles the grid. No-op when empty.
fn on_apply(
    _: On<Pointer<Click>>,
    mut staging: ResMut<SkillPanelStaging>,
    tree: Res<SkillTreeState>,
    mut writer: MessageWriter<SkillLearnRequested>,
) {
    for skill_id in apply_order(&staging.pending, &tree) {
        writer.write(SkillLearnRequested { skill_id });
    }
    staging.clear();
}

// ---------------------------------------------------------------------------
// Body: tab strip | grid + connectors | info panel, over the footer. Projected from
// live state; `bsn!` scenes own their data, so every view-model is prepared as owned
// values before entering a `bsn!` block.
// ---------------------------------------------------------------------------

/// One grid cell's owned view-model.
struct CellView {
    skill_id: u32,
    col: u32,
    row: u32,
    icon: Option<String>,
    level: u32,
    max_level: u32,
    name: String,
    learned: bool,
    icon_color: Color,
    can_raise: bool,
    can_lower: bool,
    selected: bool,
}

/// One prereq connector segment (imperative geometry, architecture §7).
struct Seg {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    color: Color,
}

/// One "Requires" row's owned view-model.
struct ReqView {
    name: String,
    min_level: u32,
    met: bool,
}

/// The selected skill's owned view-model for the info panel.
struct InfoView {
    name: String,
    level_line: String,
    form: String,
    sp: Option<String>,
    target: String,
    requires: Vec<ReqView>,
    description: Vec<String>,
}

fn cell_views(
    tab: u32,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    status: Option<&CharacterStatus>,
    selected: Option<u32>,
    catalog: Option<&SkillCatalog>,
    placements: &HashMap<u32, Placement>,
) -> Vec<CellView> {
    let mut views: Vec<CellView> = placements
        .iter()
        .filter(|(_, p)| p.tab == tab)
        .filter_map(|(&skill_id, placement)| {
            let node = tree.skills.get(&skill_id)?;
            let effective = staging.effective_level(skill_id, tree);
            let learned = effective > 0;
            let maxed = effective >= node.max_level && node.max_level > 0;
            Some(CellView {
                skill_id,
                col: placement.col,
                row: placement.row,
                icon: catalog.and_then(|c| c.icon_path(skill_id)),
                level: effective,
                max_level: node.max_level,
                name: skill_name(skill_id, catalog),
                learned,
                icon_color: cell_icon_color(learned, maxed),
                can_raise: status
                    .is_some_and(|s| staging.can_raise(skill_id, tree, s, s.skill_point)),
                can_lower: staging.staged(skill_id) > 0,
                selected: selected == Some(skill_id),
            })
        })
        .collect();
    views.sort_unstable_by_key(|v| v.skill_id);
    views
}

/// `(start, length)` of a 1D span between two coordinates, length clamped to 0.
fn ordered_span(a: f32, b: f32) -> (f32, f32) {
    (a.min(b), (a - b).abs())
}

/// One orthogonal (vertical then horizontal) connector per in-tab `requires` edge,
/// colored met/unmet from the effective (server + staged) levels.
fn connector_segments(
    tab: u32,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    placements: &HashMap<u32, Placement>,
) -> Vec<Seg> {
    let mut segs = Vec::new();
    for (&skill_id, placement) in placements {
        if placement.tab != tab {
            continue;
        }
        let Some(node) = tree.skills.get(&skill_id) else {
            continue;
        };
        for &(prereq, min_level) in &node.requires {
            let Some(from) = placements.get(&prereq) else {
                continue;
            };
            if from.tab != tab {
                continue;
            }
            let met = staging.effective_level(prereq, tree) >= min_level;
            let color = if met {
                theme::EMERALD.with_alpha(0.32)
            } else {
                theme::STROKE
            };
            let x1 = from.col as f32 * CELL_W + IC_X;
            let y1 = from.row as f32 * CELL_H + IC_Y;
            let x2 = placement.col as f32 * CELL_W + IC_X;
            let y2 = placement.row as f32 * CELL_H + IC_Y;
            let (top, height) = ordered_span(y1, y2);
            segs.push(Seg {
                left: x1,
                top,
                width: 1.0,
                height,
                color,
            });
            let (left, width) = ordered_span(x1, x2);
            segs.push(Seg {
                left,
                top: y2,
                width: width.max(1.0),
                height: 1.0,
                color,
            });
        }
    }
    segs
}

fn info_view(
    skill_id: u32,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
    catalog: Option<&SkillCatalog>,
) -> Option<InfoView> {
    let node = tree.skills.get(&skill_id)?;
    let passive = form(node.inf_type) == Form::Passive;
    let effective = staging.effective_level(skill_id, tree);
    let level_label = if passive { "Max Lv" } else { "Lv" };
    Some(InfoView {
        name: skill_name(skill_id, catalog),
        level_line: format!("{level_label} {}", format_level(effective, node.max_level)),
        form: form_label(node.inf_type),
        sp: (!passive && node.sp > 0).then(|| node.sp.to_string()),
        target: target_label(node.inf_type),
        requires: node
            .requires
            .iter()
            .map(|&(prereq, min_level)| ReqView {
                name: skill_name(prereq, catalog),
                min_level,
                met: staging.effective_level(prereq, tree) >= min_level,
            })
            .collect(),
        description: catalog
            .and_then(|c| c.get(skill_id))
            .map(|m| m.description.clone())
            .unwrap_or_default(),
    })
}

/// The whole swappable body: the tab strip / grid+connectors / info-panel row over the
/// Skill-Points footer.
fn body(
    tree: &SkillTreeState,
    ui: &SkillPanelUi,
    staging: &SkillPanelStaging,
    status: Option<&CharacterStatus>,
    catalog: Option<&SkillCatalog>,
    registry: Option<&JobSpriteRegistry>,
) -> impl Scene {
    let placements = layout(tree);
    let tabs: Vec<_> = tab_ids(tree)
        .into_iter()
        .enumerate()
        .map(|(ordinal, job_id)| {
            tab_button(
                job_id,
                tab_label(job_id, ordinal, registry),
                ui.tab == Some(job_id),
            )
        })
        .collect();

    let cells = ui
        .tab
        .map(|tab| {
            cell_views(
                tab,
                tree,
                staging,
                status,
                ui.selected,
                catalog,
                &placements,
            )
        })
        .unwrap_or_default();
    let segs = ui
        .tab
        .map(|tab| connector_segments(tab, tree, staging, &placements))
        .unwrap_or_default();
    let info = ui
        .selected
        .and_then(|skill_id| info_view(skill_id, tree, staging, catalog));

    let points_left = status
        .map(|s| staging.points_left(s.skill_point))
        .unwrap_or(0);

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [
            body_row(tabs, cells, segs, info),
            footer(points_left, staging.is_empty()),
        ]
    }
}

fn body_row(
    tabs: Vec<impl Scene>,
    cells: Vec<CellView>,
    segs: Vec<Seg>,
    info: Option<InfoView>,
) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, height: px(PANE_HEIGHT) }
        ignore_picking()
        Children [ tab_strip(tabs), grid_pane(cells, segs), info_panel(info) ]
    }
}

fn tab_strip(tabs: Vec<impl Scene>) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(2),
            width: px(TAB_STRIP_W),
            flex_shrink: 0.0,
            padding: {UiRect::axes(px(4), px(8))},
            border: {UiRect { right: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ {tabs} ]
    }
}

fn tab_button(job_id: u32, label: String, active: bool) -> impl Scene {
    let (bg, fg) = if active {
        (theme::EMERALD_INK, theme::EMERALD_BRI)
    } else {
        (Color::NONE, theme::TEXT_FAINT)
    };
    bsn! {
        template_value(SkillPanelTab(job_id))
        Node {
            padding: {UiRect::axes(px(5), px(9))},
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(bg)
        Pickable
        on(on_tab_click)
        Children [ chrome_text(label, 10.0, fg) ]
    }
}

/// The bordered grid: a fixed-height, wheel-scrollable viewport whose content canvas is
/// sized to the tree so scrolling has extent. Connector segments render first (behind)
/// and cells second (on top). The `#grid` id wires the scrollbar to the viewport.
fn grid_pane(cells: Vec<CellView>, segs: Vec<Seg>) -> impl Scene {
    let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0);
    let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0);
    let content_w = (max_col as f32 + 1.0) * CELL_W;
    let content_h = (max_row as f32 + 1.0) * CELL_H;
    let empty = cells.is_empty();
    let segments: Vec<_> = segs.into_iter().map(connector).collect();
    let tiles: Vec<_> = cells.into_iter().map(cell).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("No skills.".to_string())));
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            position_type: PositionType::Relative,
        }
        ignore_picking()
        Children [
            (
                #grid
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll()},
                    padding: px(7),
                }
                ScrollArea
                Pickable
                Children [
                    (
                        Node {
                            position_type: PositionType::Relative,
                            width: {Val::Px(content_w)},
                            height: {Val::Px(content_h)},
                            flex_shrink: 0.0,
                        }
                        ignore_picking()
                        Children [ {segments}, {tiles}, {empty_msg} ]
                    ),
                ]
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

fn connector(seg: Seg) -> impl Scene {
    let Seg {
        left,
        top,
        width,
        height,
        color,
    } = seg;
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Px(left)},
            top: {Val::Px(top)},
            width: {Val::Px(width)},
            height: {Val::Px(height)},
        }
        BackgroundColor(color)
        ignore_picking()
    }
}

/// One grid cell: an absolute-positioned icon well + level badge + name + `◄ lv/max ►`
/// stepper, carrying the `skill_id` for select/cast/drag.
fn cell(view: CellView) -> impl Scene {
    let bg = if view.selected {
        theme::EMERALD_INK
    } else {
        Color::NONE
    };
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let badge = view
        .learned
        .then(|| EntityScene(level_badge(view.level.to_string(), view.icon_color)));
    bsn! {
        template_value(SkillPanelCell(view.skill_id))
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Px(view.col as f32 * CELL_W)},
            top: {Val::Px(view.row as f32 * CELL_H)},
            width: px(CELL_W),
            height: px(CELL_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(3),
            padding: {UiRect::axes(px(1), px(5))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(bg)
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        Children [
            (
                Node {
                    position_type: PositionType::Relative,
                    width: px(42),
                    height: px(42),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(9)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(if view.learned { theme::EMERALD } else { theme::STROKE })
                ignore_picking()
                Children [ {icon}, {badge} ]
            ),
            chrome_text(view.name, 8.5, view.icon_color),
            stepper_row(
                view.skill_id,
                format_level(view.level, view.max_level),
                view.can_raise,
                view.can_lower,
            ),
        ]
    }
}

fn cell_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node { width: px(28), height: px(28) }
        ignore_picking()
    }
}

fn level_badge(text: String, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: {body_font()},
            font_size: {bevy::text::FontSize::Px(9.0)},
        }
        TextColor(color)
        Node { position_type: PositionType::Absolute, right: px(3), bottom: px(2) }
        ignore_picking()
    }
}

/// `◄ lv/max ►` stepper row. The arrows stage/unstage a level; each dims when its
/// direction is unavailable.
fn stepper_row(skill_id: u32, level_text: String, can_raise: bool, can_lower: bool) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(1) }
        ignore_picking()
        Children [
            stepper_arrow(skill_id, false, can_lower),
            chrome_text(level_text, 9.5, theme::TEXT_FAINT),
            stepper_arrow(skill_id, true, can_raise),
        ]
    }
}

fn stepper_arrow(skill_id: u32, raise: bool, enabled: bool) -> impl Scene {
    let glyph = if raise { "\u{25BA}" } else { "\u{25C4}" };
    let color = if enabled {
        theme::EMERALD_BRI
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        template_value(SkillPanelStepper { skill_id, raise })
        Node { align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        Pickable
        on(on_stepper)
        Children [ chrome_text(glyph.to_string(), 9.0, color) ]
    }
}

/// The selected-skill info panel: a fixed-width bordered box whose content scrolls
/// internally. The `#info` id wires the scrollbar to the scrollable viewport.
fn info_panel(info: Option<InfoView>) -> impl Scene {
    let content = info.map(|view| EntityScene(info_content(view)));
    let empty_msg = content
        .is_none()
        .then(|| EntityScene(muted_text("Select a skill\nto view details".to_string())));
    bsn! {
        Node {
            width: px(INFO_WIDTH),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            border: {UiRect { left: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            (
                #info
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: {UiRect { left: Val::Px(13.0), right: Val::Px(15.0), top: Val::Px(13.0), bottom: Val::Px(13.0) }},
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
    let sp_row = view
        .sp
        .map(|sp| EntityScene(meta_row("SP Cost".to_string(), sp)));
    let requires = (!view.requires.is_empty()).then(|| EntityScene(requires_block(view.requires)));
    let description: Vec<_> = view.description.into_iter().map(colored_line).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [
            chrome_text(view.name, 12.5, theme::EMERALD_BRI),
            chrome_text(view.level_line, 10.0, theme::TEXT_FAINT),
            meta_row("Form".to_string(), view.form),
            {sp_row},
            meta_row("Target".to_string(), view.target),
            {requires},
            {description},
        ]
    }
}

/// A `label : value` row (info panel meta).
fn meta_row(label: String, value: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            chrome_text(label, 9.0, theme::TEXT_FAINT),
            chrome_text(value, 10.5, theme::TEXT),
        ]
    }
}

/// "Requires" list: one row per prereq with a met/unmet dot, name, and `Lv n`.
fn requires_block(requires: Vec<ReqView>) -> impl Scene {
    let rows: Vec<_> = requires.into_iter().map(requires_row).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
        ignore_picking()
        Children [ chrome_text("Requires".to_string(), 9.0, theme::TEXT_FAINT), {rows} ]
    }
}

fn requires_row(req: ReqView) -> impl Scene {
    let dot_color = if req.met {
        theme::EMERALD
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(7) }
        ignore_picking()
        Children [
            chrome_text("\u{25CF}".to_string(), 6.0, dot_color),
            (
                Node { flex_grow: 1.0 }
                ignore_picking()
                Children [ chrome_text(req.name, 10.0, theme::TEXT_FAINT) ]
            ),
            chrome_text(format!("Lv {}", req.min_level), 9.5, dot_color),
        ]
    }
}

/// A description line, split into `^RRGGBB`-colored runs.
fn colored_line(text: String) -> impl Scene {
    let mut runs = parse_color_codes(&text, theme::TEXT_DIM).into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((theme::TEXT_DIM, String::new()));
    let spans: Vec<_> = runs.map(|(color, seg)| colored_span(color, seg)).collect();
    bsn! {
        Text(first_text)
        TextFont {
            font: {body_font()},
            font_size: {bevy::text::FontSize::Px(10.0)},
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
            font: {body_font()},
            font_size: {bevy::text::FontSize::Px(10.0)},
        }
        TextColor(color)
    }
}

fn muted_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: {body_font()},
            font_size: {bevy::text::FontSize::Px(10.5)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

fn body_font() -> bevy::text::FontSourceTemplate {
    bevy::text::FontSourceTemplate::Handle("fonts/manrope.ttf".into())
}

/// Footer: "Skill Points" label + value, plus the Reset/Apply buttons (dimmed when
/// nothing is staged).
fn footer(points_left: u32, empty: bool) -> impl Scene {
    let alpha = if empty { 0.3 } else { 1.0 };
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            height: px(42),
            padding: {UiRect::horizontal(px(14))},
            column_gap: px(9),
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            chrome_text("Skill Points".to_string(), 10.0, theme::TEXT_FAINT),
            bank_text(points_left.to_string()),
            (
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(7),
                }
                ignore_picking()
                Children [ reset_button(theme::FIELD.with_alpha(alpha)), apply_button(theme::EMERALD.with_alpha(alpha)) ]
            ),
        ]
    }
}

fn bank_text(value: String) -> impl Scene {
    bsn! {
        SkillPanelBank
        Text(value)
        TextFont {
            font: {body_font()},
            font_size: {bevy::text::FontSize::Px(16.0)},
        }
        TextColor(theme::EMERALD_BRI)
        ignore_picking()
    }
}

fn reset_button(bg: Color) -> impl Scene {
    bsn! {
        SkillPanelCommitButton
        Node {
            height: px(28),
            padding: {UiRect::axes(px(15), px(0))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(bg)
        Pickable
        on(on_reset)
        Children [ chrome_text("Reset".to_string(), 11.5, theme::TEXT_DIM) ]
    }
}

fn apply_button(bg: Color) -> impl Scene {
    bsn! {
        SkillPanelCommitButton
        Node {
            height: px(28),
            padding: {UiRect::axes(px(15), px(0))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(bg)
        Pickable
        on(on_apply)
        Children [ chrome_text("Apply".to_string(), 11.5, theme::EMERALD_INK) ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::skill::SkillNode;

    fn node(level: u32, max_level: u32, job_id: u32) -> SkillNode {
        SkillNode {
            level,
            max_level,
            upgradable: true,
            requires: vec![],
            req_base_level: 0,
            req_job_level: 0,
            sp: 0,
            range: 0,
            inf_type: 0,
            job_id,
            splash_radius: 0,
        }
    }

    fn tree(entries: &[(u32, SkillNode)]) -> SkillTreeState {
        let mut state = SkillTreeState::default();
        for (id, n) in entries {
            state.skills.insert(
                *id,
                SkillNode {
                    level: n.level,
                    max_level: n.max_level,
                    upgradable: n.upgradable,
                    requires: n.requires.clone(),
                    req_base_level: n.req_base_level,
                    req_job_level: n.req_job_level,
                    sp: n.sp,
                    range: n.range,
                    inf_type: n.inf_type,
                    job_id: n.job_id,
                    splash_radius: n.splash_radius,
                },
            );
        }
        state
    }

    fn with_requires(mut n: SkillNode, requires: Vec<(u32, u32)>) -> SkillNode {
        n.requires = requires;
        n
    }

    fn with_levels(mut n: SkillNode, base: u32, job: u32) -> SkillNode {
        n.req_base_level = base;
        n.req_job_level = job;
        n
    }

    fn status(base_level: u32, job_level: u32) -> CharacterStatus {
        CharacterStatus {
            base_level,
            job_level,
            ..default()
        }
    }

    #[test]
    fn double_click_same_skill_within_window_is_true() {
        let last = LastSkillPanelClick {
            skill_id: 5,
            at: Duration::from_millis(100),
        };
        assert!(is_cast_double_click(&last, 5, Duration::from_millis(350)));
    }

    #[test]
    fn click_different_skill_is_false() {
        let last = LastSkillPanelClick {
            skill_id: 5,
            at: Duration::from_millis(100),
        };
        assert!(!is_cast_double_click(&last, 6, Duration::from_millis(200)));
    }

    #[test]
    fn double_click_too_far_apart_is_false() {
        let last = LastSkillPanelClick {
            skill_id: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_cast_double_click(&last, 2, Duration::from_millis(500)));
    }

    #[test]
    fn can_raise_blocked_without_points() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let staging = SkillPanelStaging::default();
        assert!(!staging.can_raise(1, &t, &status(100, 50), 0));
        assert!(staging.can_raise(1, &t, &status(100, 50), 1));
    }

    #[test]
    fn can_raise_blocked_at_max_level() {
        let t = tree(&[(1, node(5, 5, 7))]);
        let staging = SkillPanelStaging::default();
        assert!(!staging.can_raise(1, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_blocked_when_prereq_unmet() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let staging = SkillPanelStaging::default();
        assert!(!staging.can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_allowed_when_prereq_staged_in_same_batch() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let mut staging = SkillPanelStaging::default();
        staging.raise(1, &t, &status(100, 50), 99);
        assert!(staging.can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_blocked_when_base_or_job_level_too_low() {
        let t = tree(&[(1, with_levels(node(0, 5, 7), 50, 20))]);
        let staging = SkillPanelStaging::default();
        assert!(!staging.can_raise(1, &t, &status(49, 99), 99));
        assert!(!staging.can_raise(1, &t, &status(99, 19), 99));
        assert!(staging.can_raise(1, &t, &status(50, 20), 99));
    }

    #[test]
    fn lower_clamps_at_zero_and_removes_entry() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let mut staging = SkillPanelStaging::default();
        staging.lower(1);
        assert_eq!(staging.staged(1), 0);
        assert!(staging.is_empty());

        staging.raise(1, &t, &status(100, 50), 99);
        staging.raise(1, &t, &status(100, 50), 99);
        assert_eq!(staging.staged(1), 2);
        staging.lower(1);
        assert_eq!(staging.staged(1), 1);
        staging.lower(1);
        assert_eq!(staging.staged(1), 0);
        assert!(staging.is_empty());
    }

    #[test]
    fn points_left_is_plain_subtraction() {
        let t = tree(&[(1, node(0, 5, 7)), (2, node(0, 5, 7))]);
        let mut staging = SkillPanelStaging::default();
        staging.raise(1, &t, &status(100, 50), 10);
        staging.raise(1, &t, &status(100, 50), 10);
        staging.raise(2, &t, &status(100, 50), 10);
        assert_eq!(staging.spent(), 3);
        assert_eq!(staging.points_left(10), 7);
        assert_eq!(staging.points_left(2), 0);
    }

    #[test]
    fn apply_order_emits_prereq_before_dependent() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let pending = HashMap::from([(1, 1), (2, 1)]);
        assert_eq!(apply_order(&pending, &t), vec![1, 2]);
    }

    #[test]
    fn apply_order_repeats_per_staged_level() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let pending = HashMap::from([(1, 3)]);
        assert_eq!(apply_order(&pending, &t), vec![1, 1, 1]);
    }

    #[test]
    fn apply_order_does_not_hang_on_cycle() {
        let t = tree(&[
            (1, with_requires(node(0, 5, 7), vec![(2, 1)])),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let pending = HashMap::from([(1, 1), (2, 1)]);
        assert_eq!(apply_order(&pending, &t).len(), 2);
    }

    #[test]
    fn tab_ids_are_sorted_and_deduped() {
        let t = tree(&[(1, node(0, 5, 9)), (2, node(0, 5, 7)), (3, node(0, 5, 7))]);
        assert_eq!(tab_ids(&t), vec![7, 9]);
    }

    #[test]
    fn tab_label_falls_back_to_ordinal_when_unresolved() {
        assert_eq!(tab_label(999, 0, None), "Tier 1");
        assert_eq!(tab_label(999, 2, None), "Tier 3");
    }

    #[test]
    fn cell_icon_color_tracks_state() {
        assert_eq!(cell_icon_color(false, false), theme::TEXT_FAINT);
        assert_eq!(cell_icon_color(true, false), theme::EMERALD_BRI);
        assert_eq!(cell_icon_color(true, true), theme::GOLD);
    }

    fn skills_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<SkillPanelUi>();
        app.init_resource::<SkillPanelStaging>();
        app.add_systems(Update, (ensure_default_tab, rebuild_skills_body).chain());
        app
    }

    fn cell_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<SkillPanelCell>>()
            .iter(world)
            .count()
    }

    #[test]
    fn rebuild_renders_one_cell_per_active_tab_skill() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, node(0, 5, 7)),
            (2, node(0, 5, 7)),
            (3, node(0, 5, 9)),
        ]));
        app.world_mut().spawn(SkillsTabBody);

        app.update();
        assert_eq!(
            cell_count(&mut app),
            2,
            "default tab 7 shows its two skills"
        );

        app.world_mut().resource_mut::<SkillPanelUi>().tab = Some(9);
        app.update();
        assert_eq!(
            cell_count(&mut app),
            1,
            "switching to tab 9 shows its one skill"
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
                duration: Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    #[test]
    fn on_apply_emits_ordered_events_and_clears() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<SkillLearnRequested>();
        app.init_resource::<SkillPanelStaging>();

        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        app.insert_resource(t);
        app.world_mut().resource_mut::<SkillPanelStaging>().pending =
            HashMap::from([(1, 2), (2, 1)]);

        let button = app.world_mut().spawn_empty().observe(on_apply).id();
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .trigger(click_event(button, window, PointerButton::Primary));
        app.update();

        let messages = app.world().resource::<Messages<SkillLearnRequested>>();
        let mut reader = messages.get_cursor();
        let learned: Vec<u32> = reader.read(messages).map(|m| m.skill_id).collect();

        assert_eq!(learned, vec![1, 1, 2]);
        assert!(app.world().resource::<SkillPanelStaging>().is_empty());
    }

    #[test]
    fn secondary_click_on_cell_opens_info_modal_without_selecting() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<SkillCastRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<SkillPanelUi>();
        app.init_resource::<LastSkillPanelClick>();
        app.init_resource::<Time>();

        let cell = app
            .world_mut()
            .spawn(SkillPanelCell(42))
            .observe(on_cell_click)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let targets: Vec<InfoTarget> = reader.read(messages).map(|m| m.target).collect();
        assert_eq!(targets, vec![InfoTarget::Skill(42)]);
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, None);
    }

    #[test]
    fn primary_click_on_cell_still_selects_and_does_not_open_the_modal() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<SkillCastRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<SkillPanelUi>();
        app.init_resource::<LastSkillPanelClick>();
        app.init_resource::<Time>();

        let cell = app
            .world_mut()
            .spawn(SkillPanelCell(42))
            .observe(on_cell_click)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));

        assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(42));
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }
}
