//! The Skills window: one tab per job over its requirement tree, the selected skill's
//! details and the plan footer.
//!
//! [`SkillPanelUi`] and [`SkillPanelStaging`] are the UI state. [`shell`] spawns the
//! frame once. [`rebuild_tree`] respawns the tabs and the tree when the open job, the
//! filter or the tree's shape changes; [`looks`] and the `scene::paint_*` systems keep
//! the spawned tree current in place, so hovering and planning never respawn it.
//! [`rebuild_details`] and [`rebuild_footer`] respawn the read-outs when what they
//! show changes. Right-click opens the shared info modal
//! ([`crate::widgets::info_modal`]).

use std::collections::HashMap;
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::core::CharacterData;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::character::events::SkillLearnRequested;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::hotbar::HotbarSlot;
use lifthrasir_client::domain::skill::{SkillCastLevels, SkillCastRequested, SkillTreeState};
use lifthrasir_client::domain::vending::{VENDING_SKILL_ID, VendingIntent};

use crate::widgets::hotbar::HotbarDrag;
use crate::widgets::info_modal::{InfoContent, ShowInfoModal, view};
use crate::widgets::skills_window::details::{DetailView, FooterView};
use crate::widgets::skills_window::shell::{
    SkillsBody, SkillsDetails, SkillsFooter, SkillsSubtitle,
};

mod details;
pub(crate) mod layout;
mod looks;
mod scene;
pub mod shell;
#[cfg(test)]
mod tests;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Which skills of the open job stay in the foreground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkillFilter {
    #[default]
    All,
    Active,
    Passive,
}

impl SkillFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Active, Self::Passive];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "Active",
            Self::Passive => "Passive",
        }
    }
}

/// The open job tab, the filter, the selected skill and the one under the pointer.
/// `job` is `None` until a tab is picked; the last job's tab is open until then.
#[derive(Resource, Default)]
pub struct SkillPanelUi {
    job: Option<u32>,
    filter: SkillFilter,
    selected: Option<u32>,
    hovered: Option<u32>,
}

/// Last cell click, for the double-click cast window.
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

    /// Whether the plan meets every requirement of `id`. `false` for an unknown skill.
    pub fn requirements_met(&self, id: u32, tree: &SkillTreeState) -> bool {
        tree.skills.get(&id).is_some_and(|node| {
            node.requires
                .iter()
                .all(|&(req_id, req_lv)| self.effective_level(req_id, tree) >= req_lv)
        })
    }

    /// Reconciles the server capability with effective local prerequisites while
    /// preserving every other neutral learning gate.
    pub fn can_raise(
        &self,
        id: u32,
        tree: &SkillTreeState,
        status: &CharacterStatus,
        skill_point: u32,
    ) -> bool {
        self.can_raise_with_gates(id, tree, status.base_level, status.job_level, skill_point)
    }

    pub(super) fn can_raise_with_gates(
        &self,
        id: u32,
        tree: &SkillTreeState,
        base_level: u32,
        job_level: u32,
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
        if base_level < node.req_base_level || job_level < node.req_job_level {
            return false;
        }
        let server_prerequisites_met = node.requires.iter().all(|&(req_id, req_lv)| {
            tree.skills
                .get(&req_id)
                .map_or(0, |required| required.level)
                >= req_lv
        });

        self.requirements_met(id, tree) && (node.upgradable || !server_prerequisites_met)
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

    /// Rejects a staged refund that would break another staged skill's requirements.
    pub fn can_lower(&self, id: u32, tree: &SkillTreeState) -> bool {
        if self.staged(id) == 0 {
            return false;
        }
        self.pending.keys().all(|&staged_id| {
            staged_id == id
                || tree.skills.get(&staged_id).is_some_and(|node| {
                    node.requires.iter().all(|&(req_id, req_level)| {
                        let resulting_level = self
                            .effective_level(req_id, tree)
                            .saturating_sub(u32::from(req_id == id));
                        resulting_level >= req_level
                    })
                })
        })
    }

    pub fn lower(&mut self, id: u32, tree: &SkillTreeState) {
        if !self.can_lower(id, tree) {
            return;
        }
        let staged = self.staged(id);
        if staged == 1 {
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

/// A skill of the tree, carrying its id so clicks can select and cast it.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelCell(pub u32);

/// A plan button with the skill it adjusts and its direction.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelStepper {
    skill_id: u32,
    raise: bool,
}

/// A "Cast at" button with the skill it adjusts and its direction.
#[derive(Component, Clone, Copy, Default)]
pub struct CastStep {
    skill_id: u32,
    up: bool,
}

/// A requirement's name in the details, carrying the skill it opens.
#[derive(Component, Clone, Copy, Default)]
pub struct RequirementLink(pub u32);

/// A job tab.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillJobTab(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub struct SkillFilterButton(pub SkillFilter);

/// The player values that gate learning, copied so that an HP tick on
/// `CharacterStatus` does not repaint the tree.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SkillGateSnapshot {
    values: Option<SkillGates>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SkillGates {
    base_level: u32,
    job_level: u32,
    skill_point: u32,
}

impl SkillGateSnapshot {
    fn can_raise(&self, id: u32, tree: &SkillTreeState, staging: &SkillPanelStaging) -> bool {
        self.values.is_some_and(|gates| {
            staging.can_raise_with_gates(
                id,
                tree,
                gates.base_level,
                gates.job_level,
                gates.skill_point,
            )
        })
    }

    /// Why `id` cannot be raised, and whether that deserves a warning tone. `None`
    /// when it can be raised or is already mastered.
    fn block_reason(
        &self,
        id: u32,
        tree: &SkillTreeState,
        staging: &SkillPanelStaging,
    ) -> Option<(String, bool)> {
        let node = tree.skills.get(&id)?;
        let gates = self.values?;
        if staging.effective_level(id, tree) >= node.max_level {
            return None;
        }
        if !staging.requirements_met(id, tree) {
            return Some(("Requirements not met yet".to_string(), true));
        }
        if gates.base_level < node.req_base_level {
            return Some((format!("Needs Base Lv {}", node.req_base_level), true));
        }
        if gates.job_level < node.req_job_level {
            return Some((format!("Needs Job Lv {}", node.req_job_level), true));
        }
        // With points to spare, only the server's `upgradable` flag can still refuse.
        let learnable =
            staging.can_raise_with_gates(id, tree, gates.base_level, gates.job_level, u32::MAX);
        if !learnable {
            return Some(("Not learned with skill points".to_string(), false));
        }
        if staging.points_left(gates.skill_point) == 0 {
            return Some(("No skill points left".to_string(), true));
        }
        None
    }
}

/// What the spawned tabs and tree were built from.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
struct SkillStructureFingerprint {
    job: Option<u32>,
    filter: SkillFilter,
    tabs: Vec<(u32, String)>,
    skills: Vec<SkillStructure>,
}

/// A skill of the open job as the tree draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillStructure {
    skill_id: u32,
    requires: Vec<(u32, u32)>,
    max_level: u32,
    name: String,
    icon: Option<String>,
}

/// Shared with [`crate::widgets::info_modal::view`], which builds the same label
/// for the info modal's header/chips.
pub(crate) fn skill_name(skill_id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(skill_id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{skill_id}"))
}

fn job_label(job_id: u32, registry: Option<&JobSpriteRegistry>) -> String {
    registry
        .and_then(|registry| registry.try_display_name(job_id))
        .map(str::to_string)
        .unwrap_or_else(|| format!("Job #{job_id}"))
}

/// The job whose tab is open: the picked one while the tree still has it, else the
/// last job, which is the character's current one.
fn active_job(ui: &SkillPanelUi, tree: &SkillTreeState) -> Option<u32> {
    let jobs = layout::jobs(tree);
    ui.job
        .filter(|job| jobs.contains(job))
        .or(jobs.last().copied())
}

/// The catalogs that name skills and jobs. Either may still be loading.
#[derive(SystemParam)]
struct SkillNames<'w> {
    catalog: Option<Res<'w, SkillCatalog>>,
    registry: Option<Res<'w, JobSpriteRegistry>>,
}

impl SkillNames<'_> {
    fn is_changed(&self) -> bool {
        self.catalog.as_ref().is_some_and(|c| c.is_changed())
            || self.registry.as_ref().is_some_and(|r| r.is_changed())
    }
}

/// The resources the details panel reads.
#[derive(SystemParam)]
struct SkillSources<'w> {
    tree: Res<'w, SkillTreeState>,
    staging: Res<'w, SkillPanelStaging>,
    gates: Res<'w, SkillGateSnapshot>,
    cast_levels: Res<'w, SkillCastLevels>,
    names: SkillNames<'w>,
}

impl SkillSources<'_> {
    fn is_changed(&self) -> bool {
        self.tree.is_changed()
            || self.staging.is_changed()
            || self.gates.is_changed()
            || self.cast_levels.is_changed()
            || self.names.is_changed()
    }

    fn get(&self) -> details::Sources<'_> {
        details::Sources {
            tree: &self.tree,
            staging: &self.staging,
            gates: &self.gates,
            cast_levels: &self.cast_levels,
            catalog: self.names.catalog.as_deref(),
            registry: self.names.registry.as_deref(),
        }
    }
}

fn reconcile_authoritative_tree(
    tree: Res<SkillTreeState>,
    mut ui: ResMut<SkillPanelUi>,
    mut staging: ResMut<SkillPanelStaging>,
) {
    if !tree.is_changed() {
        return;
    }
    if !staging.is_empty() {
        staging.clear();
    }
    if ui.selected.is_some_and(|id| !tree.skills.contains_key(&id)) {
        ui.selected = None;
    }
    if ui.hovered.is_some_and(|id| !tree.skills.contains_key(&id)) {
        ui.hovered = None;
    }
}

fn sync_hovered_skill(
    hover_map: Option<Res<HoverMap>>,
    parents: Query<&ChildOf>,
    cells: Query<&SkillPanelCell>,
    mut ui: ResMut<SkillPanelUi>,
) {
    let hovered = hover_map
        .as_deref()
        .and_then(|hover_map| hover_map.get(&PointerId::Mouse))
        .and_then(|entities| {
            entities
                .iter()
                .filter_map(|(&entity, hit)| {
                    let mut current = entity;
                    let mut distance = 0_u32;
                    let mut visited = std::collections::HashSet::new();
                    loop {
                        if !visited.insert(current) {
                            return None;
                        }
                        if let Ok(cell) = cells.get(current) {
                            return Some((hit.depth, distance, cell.0, entity.to_bits()));
                        }
                        current = parents.get(current).ok()?.parent();
                        distance += 1;
                    }
                })
                .min_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                        .then_with(|| left.2.cmp(&right.2))
                        .then_with(|| left.3.cmp(&right.3))
                })
                .map(|(_, _, skill_id, _)| skill_id)
        });
    if ui.hovered != hovered {
        ui.hovered = hovered;
    }
}

fn sync_skill_gates(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut snapshot: ResMut<SkillGateSnapshot>,
) {
    let values = player.single().ok().map(|status| SkillGates {
        base_level: status.base_level,
        job_level: status.job_level,
        skill_point: status.skill_point,
    });
    if snapshot.values != values {
        snapshot.values = values;
    }
}

fn should_project_skills(
    tree: Res<SkillTreeState>,
    ui: Res<SkillPanelUi>,
    staging: Res<SkillPanelStaging>,
    gates: Res<SkillGateSnapshot>,
    added: Query<(), Added<scene::SkillCanvasViewport>>,
) -> bool {
    tree.is_changed()
        || ui.is_changed()
        || staging.is_changed()
        || gates.is_changed()
        || !added.is_empty()
}

fn structure_fingerprint(
    tree: &SkillTreeState,
    ui: &SkillPanelUi,
    catalog: Option<&SkillCatalog>,
    registry: Option<&JobSpriteRegistry>,
) -> SkillStructureFingerprint {
    let job = active_job(ui, tree);
    let mut skills: Vec<_> = tree
        .skills
        .iter()
        .filter(|(_, node)| Some(node.job_id) == job)
        .map(|(&skill_id, node)| {
            let mut requires = node.requires.clone();
            requires.sort_unstable();
            SkillStructure {
                skill_id,
                requires,
                max_level: node.max_level,
                name: skill_name(skill_id, catalog),
                icon: catalog.and_then(|catalog| catalog.icon_path(skill_id)),
            }
        })
        .collect();
    skills.sort_unstable_by_key(|skill| skill.skill_id);
    let tabs = layout::jobs(tree)
        .into_iter()
        .map(|job_id| (job_id, job_label(job_id, registry)))
        .collect();
    SkillStructureFingerprint {
        job,
        filter: ui.filter,
        tabs,
        skills,
    }
}

fn tree_view(
    tree: &SkillTreeState,
    shape: &SkillStructureFingerprint,
    scroll: Vec2,
) -> scene::TreeView {
    let layout = shape
        .job
        .map(|job| layout::job_tree(tree, job))
        .unwrap_or_default();
    let skills: HashMap<u32, &SkillStructure> = shape
        .skills
        .iter()
        .map(|skill| (skill.skill_id, skill))
        .collect();
    let nodes = layout
        .nodes
        .iter()
        .map(|&placement| {
            let skill = skills[&placement.skill_id];
            scene::NodeView {
                placement,
                icon: skill.icon.clone(),
                name: skill.name.clone(),
                max_level: skill.max_level,
            }
        })
        .collect();
    let tabs = shape
        .tabs
        .iter()
        .map(|(job_id, label)| scene::TabView {
            job_id: *job_id,
            label: label.clone(),
            active: Some(*job_id) == shape.job,
        })
        .collect();
    scene::TreeView {
        tabs,
        filter: shape.filter,
        nodes,
        links: layout.links,
        rows: layout.rows,
        scroll,
    }
}

/// Respawns the tabs and the tree when what they are built from changes, and once when
/// the mount first appears. The tree keeps its scroll offset while the job stays open.
fn rebuild_tree(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillPanelUi>,
    names: SkillNames,
    bodies: Query<(Entity, Option<&Children>, Ref<SkillsBody>)>,
    viewports: Query<&ScrollPosition, With<scene::SkillCanvasViewport>>,
    mut shown: ResMut<SkillStructureFingerprint>,
) {
    let Ok((body, children, body_ref)) = bodies.single() else {
        return;
    };
    let stale = body_ref.is_added() || tree.is_changed() || ui.is_changed() || names.is_changed();
    if !stale {
        return;
    }
    let next = structure_fingerprint(
        &tree,
        &ui,
        names.catalog.as_deref(),
        names.registry.as_deref(),
    );
    if !body_ref.is_added() && *shown == next {
        return;
    }
    let scroll = if shown.job == next.job {
        viewports
            .single()
            .map(|scroll| scroll.0)
            .unwrap_or_default()
    } else {
        Vec2::ZERO
    };
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands
        .spawn_scene(scene::body(tree_view(&tree, &next, scroll)))
        .insert(ChildOf(body));
    *shown = next;
}

/// Respawns the details when the selected skill or anything shown about it changes.
/// A newly selected skill starts scrolled to the top.
fn rebuild_details(
    mut commands: Commands,
    sources: SkillSources,
    ui: Res<SkillPanelUi>,
    mut mounts: Query<(Entity, Option<&Children>, &mut ScrollPosition), With<SkillsDetails>>,
    mut shown: Local<Option<(Entity, Option<DetailView>)>>,
) {
    let Ok((mount, children, mut scroll)) = mounts.single_mut() else {
        return;
    };
    let last = shown
        .as_ref()
        .filter(|(entity, _)| *entity == mount)
        .map(|(_, view)| view);
    if last.is_some() && !sources.is_changed() && !ui.is_changed() {
        return;
    }
    let view = ui
        .selected
        .and_then(|skill_id| details::detail_view(skill_id, &sources.get()));
    if last == Some(&view) {
        return;
    }
    let last_skill = last
        .and_then(|view| view.as_ref())
        .map(|view| view.skill_id);
    if last_skill != view.as_ref().map(|view| view.skill_id) {
        scroll.0 = Vec2::ZERO;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands
        .spawn_scene(details::details(view.clone()))
        .insert(ChildOf(mount));
    *shown = Some((mount, view));
}

/// Respawns the point bank and the plan buttons when their values change.
fn rebuild_footer(
    mut commands: Commands,
    staging: Res<SkillPanelStaging>,
    gates: Res<SkillGateSnapshot>,
    mounts: Query<(Entity, Option<&Children>), With<SkillsFooter>>,
    mut shown: Local<Option<(Entity, FooterView)>>,
) {
    let Ok((mount, children)) = mounts.single() else {
        return;
    };
    let view = FooterView {
        points_left: gates
            .values
            .map_or(0, |gates| staging.points_left(gates.skill_point)),
        staged: staging.spent(),
    };
    if *shown == Some((mount, view)) {
        return;
    }
    *shown = Some((mount, view));
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands
        .spawn_scene(details::footer(view))
        .insert(ChildOf(mount));
}

/// Keeps the title bar's "Knight · Job Lv 32" current. `CharacterStatus` changes on
/// every HP tick, so the shown values are compared instead of trusting change detection.
fn sync_subtitle(
    player: Query<(&CharacterData, &CharacterStatus), With<LocalPlayer>>,
    registry: Option<Res<JobSpriteRegistry>>,
    mut labels: Query<(Entity, &mut Text), With<SkillsSubtitle>>,
    mut shown: Local<Option<(Entity, u16, u32, bool)>>,
) {
    let (Ok((data, status)), Ok((label, mut text))) = (player.single(), labels.single_mut()) else {
        return;
    };
    let key = (label, data.job_id, status.job_level, registry.is_some());
    if *shown == Some(key) {
        return;
    }
    *shown = Some(key);
    let job = job_label(u32::from(data.job_id), registry.as_deref());
    text.0 = format!("{job} \u{b7} Job Lv {}", status.job_level);
}

pub struct SkillsWindowPlugin;

impl Plugin for SkillsWindowPlugin {
    fn build(&self, app: &mut App) {
        register(app);
        app.add_systems(
            Update,
            shell::toggle_skills_window.run_if(
                in_state(GameState::InGame)
                    .and_then(lifthrasir_client::domain::input::ui_unfocused),
            ),
        );
        app.add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Register the window's resources and ordered update pipeline.
fn register(app: &mut App) {
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<SkillPanelStaging>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<SkillGateSnapshot>();
    app.init_resource::<SkillStructureFingerprint>();
    app.init_resource::<looks::SkillLooks>();
    app.add_systems(
        Update,
        pipeline()
            .after(lifthrasir_client::domain::skill::apply_skill_list)
            .after(
                lifthrasir_client::domain::entities::character::systems::update_character_status_system,
            )
            .run_if(in_state(GameState::InGame)),
    );
}

/// State sync, then the rebuilds, then the in-place repaint of the spawned tree.
fn pipeline() -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
    (
        reconcile_authoritative_tree,
        sync_skill_gates,
        sync_hovered_skill,
        (rebuild_tree, rebuild_details, rebuild_footer, sync_subtitle),
        ApplyDeferred,
        (
            looks::compute,
            (
                scene::paint_frames,
                scene::paint_labels,
                scene::paint_pips,
                scene::paint_overlays,
                scene::paint_lines,
                scene::paint_tab_dots,
            ),
        )
            .chain()
            .run_if(should_project_skills),
    )
        .chain()
}

/// Reset the window's state and discard the plan when leaving the game.
pub fn reset(
    mut ui: ResMut<SkillPanelUi>,
    mut staging: ResMut<SkillPanelStaging>,
    mut cast_levels: ResMut<SkillCastLevels>,
) {
    *ui = SkillPanelUi::default();
    staging.clear();
    cast_levels.clear();
}

/// Stages or unstages one level. Reads the player status so `can_raise`'s point and
/// level gates are evaluated from the source.
fn stage(
    stepper: &SkillPanelStepper,
    tree: &SkillTreeState,
    player: &Query<&CharacterStatus, With<LocalPlayer>>,
    staging: &mut SkillPanelStaging,
) {
    if !stepper.raise {
        staging.lower(stepper.skill_id, tree);
        return;
    }
    let Ok(status) = player.single() else {
        return;
    };
    staging.raise(stepper.skill_id, tree, status, status.skill_point);
}

/// A tree stepper: a primary click stages without selecting the skill under it.
fn on_stepper(
    mut click: On<PointerClick>,
    steppers: Query<&SkillPanelStepper>,
    tree: Res<SkillTreeState>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<SkillPanelStaging>,
) {
    let Ok(stepper) = steppers.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    click.propagate(false);
    stage(stepper, &tree, &player, &mut staging);
}

/// A "Plan levels" button of the details panel.
fn on_plan_step(
    activate: On<Activate>,
    steppers: Query<&SkillPanelStepper>,
    tree: Res<SkillTreeState>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<SkillPanelStaging>,
) {
    let Ok(stepper) = steppers.get(activate.entity) else {
        return;
    };
    stage(stepper, &tree, &player, &mut staging);
}

/// A "Cast at" button: moves the level the skill is cast at by one.
fn on_cast_step(
    activate: On<Activate>,
    steps: Query<&CastStep>,
    tree: Res<SkillTreeState>,
    mut cast_levels: ResMut<SkillCastLevels>,
) {
    let Ok(step) = steps.get(activate.entity) else {
        return;
    };
    let Some(node) = tree.skills.get(&step.skill_id) else {
        return;
    };
    let level = cast_levels.level(step.skill_id, node.level);
    let next = if step.up {
        level + 1
    } else {
        level.saturating_sub(1)
    };
    cast_levels.set(step.skill_id, next.clamp(1, node.level.max(1)), node.level);
}

/// Opens a required skill: its job tab and its details.
fn on_requirement_click(
    click: On<PointerClick>,
    links: Query<&RequirementLink>,
    tree: Res<SkillTreeState>,
    mut ui: ResMut<SkillPanelUi>,
) {
    let Ok(link) = links.get(click.entity) else {
        return;
    };
    let Some(node) = tree.skills.get(&link.0) else {
        return;
    };
    ui.job = Some(node.job_id);
    ui.selected = Some(link.0);
}

/// Opens a job tab and selects its first skill.
fn pick_job(
    click: On<PointerClick>,
    tabs: Query<&SkillJobTab>,
    tree: Res<SkillTreeState>,
    mut ui: ResMut<SkillPanelUi>,
) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.job = Some(tab.0);
    ui.selected = tree
        .skills
        .iter()
        .filter(|(_, node)| node.job_id == tab.0)
        .map(|(&skill_id, _)| skill_id)
        .min();
}

fn pick_filter(
    click: On<PointerClick>,
    buttons: Query<&SkillFilterButton>,
    mut ui: ResMut<SkillPanelUi>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    ui.filter = button.0;
}

/// Cell click: select the skill; a double-click within the cast window emits
/// [`SkillCastRequested`]. Secondary-click opens the info modal instead.
fn on_cell_click(
    click: On<PointerClick>,
    cells: Query<&SkillPanelCell>,
    mut ui: ResMut<SkillPanelUi>,
    mut clicks: CellClicks,
    mut actions: CellActions,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        actions.inspect(cell.0);
        return;
    }
    if ui.selected != Some(cell.0) {
        ui.selected = Some(cell.0);
    }
    if clicks.record(cell.0) {
        actions.cast(cell.0);
    }
}

/// The double-click window of the tree's cells.
#[derive(SystemParam)]
struct CellClicks<'w> {
    time: Res<'w, Time>,
    last: ResMut<'w, LastSkillPanelClick>,
}

impl CellClicks<'_> {
    /// Records a click on `skill_id` and tells whether it completes a double-click.
    fn record(&mut self, skill_id: u32) -> bool {
        let now = self.time.elapsed();
        let double = is_cast_double_click(&self.last, skill_id, now);
        *self.last = LastSkillPanelClick { skill_id, at: now };
        double
    }
}

/// What a cell click can ask for: a cast, or the info modal.
#[derive(SystemParam)]
struct CellActions<'w, 's> {
    catalog: Option<Res<'w, SkillCatalog>>,
    tree: Res<'w, SkillTreeState>,
    staging: Res<'w, SkillPanelStaging>,
    player: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
    cast: MessageWriter<'w, SkillCastRequested>,
    vending: MessageWriter<'w, VendingIntent>,
    info: MessageWriter<'w, ShowInfoModal>,
}

impl CellActions<'_, '_> {
    /// The vending skill reveals an existing setup/monitor instead of recasting.
    fn cast(&mut self, skill_id: u32) {
        if skill_id == VENDING_SKILL_ID {
            self.vending.write(VendingIntent::EnterSeller);
            return;
        }
        self.cast.write(SkillCastRequested { skill_id });
    }

    fn inspect(&mut self, skill_id: u32) {
        let Some(view) = view::build_skill_view(
            skill_id,
            self.catalog.as_deref(),
            &self.tree,
            &self.staging,
            self.player.single().ok(),
        ) else {
            warn!("skills: skill #{skill_id} not in the tree, ignoring inspect");
            return;
        };
        self.info.write(ShowInfoModal {
            content: InfoContent::Skill {
                view,
                raise: Some(skill_id),
            },
        });
    }
}

/// Dragging a skill cell arms the hotbar with that skill so a slot drop assigns it. A
/// plain click still goes through [`on_cell_click`] since `bevy_picking` only emits
/// `DragStart` after a press-and-move.
fn on_cell_drag_start(
    drag: On<PointerDragStart>,
    cells: Query<&SkillPanelCell>,
    mut hotbar_drag: ResMut<HotbarDrag>,
) {
    let Ok(cell) = cells.get(drag.entity) else {
        return;
    };
    hotbar_drag.payload = Some(HotbarSlot::Skill(cell.0));
}

/// Reset: discard all staged levels without contacting the server.
fn on_reset(_: On<Activate>, mut staging: ResMut<SkillPanelStaging>) {
    staging.clear();
}

/// Apply: emit one [`SkillLearnRequested`] per staged level in prereq-first order, then
/// clear staging. The resent `SkillList` reconciles the tree. No-op when empty.
fn on_apply(
    _: On<Activate>,
    mut staging: ResMut<SkillPanelStaging>,
    tree: Res<SkillTreeState>,
    mut writer: MessageWriter<SkillLearnRequested>,
) {
    for skill_id in apply_order(&staging.pending, &tree) {
        writer.write(SkillLearnRequested { skill_id });
    }
    staging.clear();
}
