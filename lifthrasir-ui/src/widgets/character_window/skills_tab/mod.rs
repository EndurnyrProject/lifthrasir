//! The Console's Skills tab state, staging rules, and interactions.
//!
//! [`scene`] owns the declarative all-job horizontal canvas and its live projection.
//! Static entities rebuild only when topology or presentation metadata changes.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::character::events::SkillLearnRequested;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::hotbar::HotbarSlot;
use game_engine::domain::skill::{SkillCastRequested, SkillTreeState};
use game_engine::infrastructure::job::registry::JobSpriteRegistry;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;
use crate::widgets::hotbar::HotbarDrag;
use crate::widgets::info_modal::{InfoTarget, ShowInfoModal};

use super::SkillsTabBody;

pub(crate) mod layout;
mod scene;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Persistent skill selection. Hover state is added with focused-chain projection in Task 9.
#[derive(Resource, Default)]
pub struct SkillPanelUi {
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
        let effective_prerequisites_met = node
            .requires
            .iter()
            .all(|&(req_id, req_lv)| self.effective_level(req_id, tree) >= req_lv);

        effective_prerequisites_met && (node.upgradable || !server_prerequisites_met)
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

/// Marks a grid cell with the `skill_id` it shows so clicks can select/cast it.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelCell(pub u32);

/// Marks a `◄`/`►` stepper button with the skill it adjusts and its direction.
#[derive(Component, Clone, Copy, Default)]
pub struct SkillPanelStepper {
    skill_id: u32,
    raise: bool,
}

/// Marks Reset/Apply so live projection can update style and pickability.
#[derive(Component, Default, Clone, Copy)]
pub struct SkillPanelCommitButton {
    apply: bool,
}

/// Marks the remaining skill-point value text.
#[derive(Component, Default, Clone)]
pub struct SkillPanelBank;

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

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
struct SkillStructureFingerprint {
    skills: Vec<SkillStructure>,
    jobs: Vec<(u32, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillStructure {
    skill_id: u32,
    job_id: u32,
    requires: Vec<(u32, u32)>,
    name: String,
    icon: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested).
// ---------------------------------------------------------------------------

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

/// Shared with [`crate::widgets::info_modal::view`], which builds the same label
/// for the info modal's header/chips.
pub(crate) fn skill_name(skill_id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(skill_id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{skill_id}"))
}

// ---------------------------------------------------------------------------
// Systems.
// ---------------------------------------------------------------------------

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

fn job_labels(tree: &SkillTreeState, registry: Option<&JobSpriteRegistry>) -> HashMap<u32, String> {
    tree.skills
        .values()
        .filter_map(|node| {
            registry
                .and_then(|registry| registry.try_display_name(node.job_id))
                .map(|label| (node.job_id, label.to_string()))
        })
        .collect()
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
    catalog: Option<&SkillCatalog>,
    labels: &HashMap<u32, String>,
) -> SkillStructureFingerprint {
    let mut skills: Vec<_> = tree
        .skills
        .iter()
        .map(|(&skill_id, node)| {
            let mut requires = node.requires.clone();
            requires.sort_unstable();
            SkillStructure {
                skill_id,
                job_id: node.job_id,
                requires,
                name: skill_name(skill_id, catalog),
                icon: catalog.and_then(|catalog| catalog.icon_path(skill_id)),
            }
        })
        .collect();
    skills.sort_unstable_by_key(|skill| skill.skill_id);
    let mut jobs: Vec<_> = tree
        .skills
        .values()
        .map(|node| (node.job_id, labels.get(&node.job_id).cloned()))
        .collect();
    jobs.sort_unstable_by_key(|(job_id, _)| *job_id);
    jobs.dedup_by_key(|(job_id, _)| *job_id);
    SkillStructureFingerprint { skills, jobs }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_skills_body(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    catalog: Option<Res<SkillCatalog>>,
    registry: Option<Res<JobSpriteRegistry>>,
    bodies: Query<(Entity, Option<&Children>, Ref<SkillsTabBody>)>,
    mut fingerprint: ResMut<SkillStructureFingerprint>,
) {
    let Ok((body_entity, children, body_ref)) = bodies.single() else {
        return;
    };
    let labels = job_labels(&tree, registry.as_deref());
    let next = structure_fingerprint(&tree, catalog.as_deref(), &labels);
    if !body_ref.is_added() && *fingerprint == next {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    *fingerprint = next;
    let layout = layout::TreeLayout::new(&tree, &labels);
    commands
        .spawn_scene(scene::body(layout, catalog.as_deref()))
        .insert(ChildOf(body_entity));
}

/// Register the Skills tab's resources and ordered update pipeline into
/// [`CharacterWindowPlugin`](super::CharacterWindowPlugin).
pub(super) fn register(app: &mut App) {
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<SkillPanelStaging>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<SkillGateSnapshot>();
    app.init_resource::<SkillStructureFingerprint>();
    app.add_systems(
        Update,
        (
            reconcile_authoritative_tree,
            sync_skill_gates,
            rebuild_skills_body,
            ApplyDeferred,
            scene::project_live.run_if(should_project_skills),
        )
            .chain()
            .after(game_engine::domain::skill::apply_skill_list)
            .after(
                game_engine::domain::entities::character::systems::update_character_status_system,
            )
            .run_if(in_state(GameState::InGame)),
    );
}

/// Reset selection and discard staging when leaving the game.
pub fn reset(mut ui: ResMut<SkillPanelUi>, mut staging: ResMut<SkillPanelStaging>) {
    *ui = SkillPanelUi::default();
    staging.clear();
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// `◄`/`►` observer: stages or unstages a level via [`SkillPanelStaging`]. Reads the
/// player status so `can_raise`'s point/level gates are evaluated from the source.
fn on_stepper(
    mut click: On<Pointer<Click>>,
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
    if !stepper.raise {
        staging.lower(stepper.skill_id, &tree);
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::entities::character::events::StatusParameterChanged;
    use game_engine::domain::entities::character::systems::PendingStatusParams;
    use game_engine::domain::entities::registry::EntityRegistry;
    use game_engine::domain::skill::SkillNode;
    use net_contract::events::{ParamChanged, SkillListReceived, ZoneSkillInfo};

    #[test]
    fn register_initializes_skills_tab_resources() {
        let mut app = App::new();
        register(&mut app);

        assert!(app.world().contains_resource::<SkillPanelUi>());
        assert!(app.world().contains_resource::<SkillPanelStaging>());
        assert!(app.world().contains_resource::<LastSkillPanelClick>());
        assert!(app.world().contains_resource::<SkillGateSnapshot>());
        assert!(app.world().contains_resource::<SkillStructureFingerprint>());
    }

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

    fn with_upgradable(mut n: SkillNode, upgradable: bool) -> SkillNode {
        n.upgradable = upgradable;
        n
    }

    fn catalog(internal_name: &str, display_name: &str) -> SkillCatalog {
        let mut data = lifthrasir_data::SkillData::default();
        data.skills.insert(
            1,
            lifthrasir_data::SkillMeta {
                name: internal_name.to_string(),
                display_name: display_name.to_string(),
                description: vec![],
                max_level: 5,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );
        SkillCatalog::from_skill_data(data)
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
    fn false_upgradable_is_overridden_only_by_staged_prerequisites() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (
                2,
                with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
            ),
        ]);
        let mut staging = SkillPanelStaging::default();

        assert!(!staging.can_raise(2, &t, &status(100, 50), 99));
        staging.raise(1, &t, &status(100, 50), 99);
        assert!(staging.can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn false_upgradable_stays_blocked_when_server_prerequisites_pass() {
        let t = tree(&[
            (1, node(1, 5, 7)),
            (
                2,
                with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
            ),
        ]);

        assert!(!SkillPanelStaging::default().can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn false_upgradable_does_not_override_other_neutral_gates() {
        let mut t = tree(&[
            (1, node(0, 5, 7)),
            (
                2,
                with_upgradable(
                    with_levels(with_requires(node(0, 5, 7), vec![(1, 1)]), 50, 20),
                    false,
                ),
            ),
        ]);
        let mut staging = SkillPanelStaging::default();
        staging.raise(1, &t, &status(100, 50), 99);

        assert!(!staging.can_raise(2, &t, &status(49, 20), 99));
        assert!(!staging.can_raise(2, &t, &status(50, 19), 99));
        assert!(!staging.can_raise(2, &t, &status(50, 20), 1));

        t.skills.get_mut(&2).unwrap().level = 5;
        assert!(!staging.can_raise(2, &t, &status(50, 20), 99));
    }

    #[test]
    fn lowering_a_staged_prerequisite_requires_lowering_dependents_first() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (
                2,
                with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
            ),
        ]);
        let mut staging = SkillPanelStaging::default();
        staging.raise(1, &t, &status(100, 50), 99);
        staging.raise(2, &t, &status(100, 50), 99);

        assert!(!staging.can_lower(1, &t));
        staging.lower(1, &t);
        assert_eq!(staging.staged(1), 1);

        staging.lower(2, &t);
        assert!(staging.can_lower(1, &t));
        staging.lower(1, &t);
        assert!(staging.is_empty());
    }

    #[test]
    fn lower_clamps_at_zero_and_removes_entry() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let mut staging = SkillPanelStaging::default();
        staging.lower(1, &t);
        assert_eq!(staging.staged(1), 0);
        assert!(staging.is_empty());

        staging.raise(1, &t, &status(100, 50), 99);
        staging.raise(1, &t, &status(100, 50), 99);
        assert_eq!(staging.staged(1), 2);
        staging.lower(1, &t);
        assert_eq!(staging.staged(1), 1);
        staging.lower(1, &t);
        assert_eq!(staging.staged(1), 0);
        assert!(staging.is_empty());
    }

    #[test]
    fn lower_never_changes_the_authoritative_level() {
        let t = tree(&[(1, node(2, 5, 7))]);
        let mut staging = SkillPanelStaging::default();
        staging.raise(1, &t, &status(100, 50), 99);

        staging.lower(1, &t);

        assert_eq!(staging.effective_level(1, &t), 2);
        assert!(!staging.can_lower(1, &t));
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
        app.add_message::<SkillListReceived>();
        app.add_message::<ParamChanged>();
        app.add_message::<StatusParameterChanged>();
        app.init_resource::<EntityRegistry>();
        app.init_resource::<PendingStatusParams>();
        app.init_resource::<SkillPanelUi>();
        app.init_resource::<SkillPanelStaging>();
        app.init_resource::<SkillGateSnapshot>();
        app.init_resource::<SkillStructureFingerprint>();
        app.init_resource::<ProjectionRunCount>();
        app.add_systems(
            Update,
            (
                game_engine::domain::skill::apply_skill_list,
                game_engine::domain::entities::character::systems::update_character_status_system,
            ),
        );
        app.add_systems(
            Update,
            (
                reconcile_authoritative_tree,
                sync_skill_gates,
                rebuild_skills_body,
                ApplyDeferred,
                scene::project_live
                    .pipe(record_projection_run)
                    .run_if(should_project_skills),
            )
                .chain()
                .after(game_engine::domain::skill::apply_skill_list)
                .after(
                    game_engine::domain::entities::character::systems::update_character_status_system,
                ),
        );
        app
    }

    #[derive(Resource, Default)]
    struct GateChangeCount(u32);

    #[derive(Resource, Default)]
    struct ProjectionRunCount(u32);

    fn record_gate_changes(snapshot: Res<SkillGateSnapshot>, mut count: ResMut<GateChangeCount>) {
        if snapshot.is_changed() {
            count.0 += 1;
        }
    }

    fn record_projection_run(_: In<()>, mut count: ResMut<ProjectionRunCount>) {
        count.0 += 1;
    }

    fn cell_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<(), With<SkillPanelCell>>()
            .iter(world)
            .count()
    }

    fn node_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&Node>().iter(world).count()
    }

    #[test]
    fn selecting_a_skill_spawns_no_inline_info_panel() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        let unselected = node_count(&mut app);

        app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(1);
        app.update();
        let selected = node_count(&mut app);

        assert_eq!(
            selected, unselected,
            "the skills tab no longer renders a selection info panel"
        );
    }

    #[test]
    fn authoritative_messages_reconcile_and_project_in_the_same_update() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn((
            CharacterStatus {
                skill_point: 1,
                ..default()
            },
            LocalPlayer,
        ));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        app.world_mut()
            .resource_mut::<SkillPanelStaging>()
            .pending
            .insert(1, 1);

        app.world_mut().write_message(SkillListReceived {
            skills: vec![ZoneSkillInfo {
                skill_id: 1,
                type_: 0,
                level: 2,
                sp: 0,
                range: 0,
                name: "Skill".to_string(),
                upgradable: true,
                max_level: 5,
                requires: vec![],
                req_base_level: 0,
                req_job_level: 0,
                job_id: 7,
                splash_radius: 0,
            }],
        });
        app.world_mut()
            .write_message(ParamChanged { var: 12, value: 5 });

        app.update();

        assert!(app.world().resource::<SkillPanelStaging>().is_empty());
        assert_eq!(app.world().resource::<SkillTreeState>().skills[&1].level, 2);
        assert_eq!(
            app.world().resource::<SkillGateSnapshot>().values,
            Some(SkillGates {
                base_level: 1,
                job_level: 1,
                skill_point: 5,
            })
        );
        let world = app.world_mut();
        assert!(
            world
                .query::<(&scene::SkillNodeLevel, &Text)>()
                .iter(world)
                .any(|(marker, text)| marker.0 == 1 && text.0 == "2/5")
        );
        assert_eq!(
            world
                .query_filtered::<&Text, With<SkillPanelBank>>()
                .single(world)
                .unwrap()
                .0,
            "5"
        );
    }

    #[test]
    fn unrelated_status_changes_do_not_change_gate_snapshot() {
        let mut app = skills_app();
        app.init_resource::<GateChangeCount>();
        app.add_systems(Update, record_gate_changes.after(sync_skill_gates));
        let player = app
            .world_mut()
            .spawn((CharacterStatus::default(), LocalPlayer))
            .id();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        assert_eq!(app.world().resource::<GateChangeCount>().0, 1);
        assert_eq!(app.world().resource::<ProjectionRunCount>().0, 1);
        let viewport = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(viewport)
            .insert(ScrollPosition(Vec2::new(13.0, 8.0)));

        app.world_mut()
            .get_mut::<CharacterStatus>(player)
            .unwrap()
            .hp -= 1;
        app.update();
        assert_eq!(app.world().resource::<GateChangeCount>().0, 1);
        assert_eq!(app.world().resource::<ProjectionRunCount>().0, 1);

        app.world_mut()
            .get_mut::<CharacterStatus>(player)
            .unwrap()
            .skill_point += 1;
        app.update();
        assert_eq!(app.world().resource::<GateChangeCount>().0, 2);
        assert_eq!(app.world().resource::<ProjectionRunCount>().0, 2);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
                .single(app.world())
                .unwrap(),
            viewport
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(viewport).unwrap().0,
            Vec2::new(13.0, 8.0)
        );
    }

    #[test]
    fn staging_projects_without_replacing_or_resetting_viewport() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn((
            CharacterStatus {
                base_level: 1,
                job_level: 1,
                skill_point: 2,
                ..default()
            },
            LocalPlayer,
        ));
        app.world_mut().spawn(SkillsTabBody);
        app.update();

        let viewport = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .expect("one viewport");
        app.world_mut()
            .entity_mut(viewport)
            .insert(ScrollPosition(Vec2::new(23.0, 17.0)));
        app.world_mut()
            .resource_mut::<SkillPanelStaging>()
            .pending
            .insert(1, 1);

        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
                .single(app.world())
                .expect("one viewport"),
            viewport
        );
        assert_eq!(
            app.world()
                .get::<ScrollPosition>(viewport)
                .expect("viewport scroll")
                .0,
            Vec2::new(23.0, 17.0)
        );
    }

    #[test]
    fn live_projection_updates_every_dynamic_marker_in_place() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]));
        app.world_mut().spawn((
            CharacterStatus {
                base_level: 10,
                job_level: 10,
                skill_point: 2,
                ..default()
            },
            LocalPlayer,
        ));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        let viewport = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();

        app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(1);
        app.world_mut()
            .resource_mut::<SkillPanelStaging>()
            .pending
            .insert(1, 1);
        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
                .single(app.world())
                .unwrap(),
            viewport
        );
        let world = app.world_mut();
        let selected_background = world
            .query::<(&SkillPanelCell, &BackgroundColor)>()
            .iter(world)
            .find_map(|(cell, background)| (cell.0 == 1).then_some(background.0))
            .unwrap();
        assert_eq!(selected_background, theme::EMERALD_INK);
        let (level, name_color, frame_color) = (
            world
                .query::<(&scene::SkillNodeLevel, &Text)>()
                .iter(world)
                .find_map(|(marker, text)| (marker.0 == 1).then_some(text.0.clone()))
                .unwrap(),
            world
                .query::<(&scene::SkillNodeName, &TextColor)>()
                .iter(world)
                .find_map(|(marker, color)| (marker.0 == 1).then_some(color.0))
                .unwrap(),
            world
                .query::<(&scene::SkillNodeFrame, &BorderColor)>()
                .iter(world)
                .find_map(|(marker, color)| (marker.0 == 1).then_some(color.top))
                .unwrap(),
        );
        assert_eq!(level, "1/5");
        assert_eq!(name_color, theme::EMERALD_BRI);
        assert_eq!(frame_color, theme::EMERALD);
        let controls: HashMap<_, _> = world
            .query::<(&SkillPanelStepper, &Pickable)>()
            .iter(world)
            .map(|(stepper, pickable)| ((stepper.skill_id, stepper.raise), *pickable))
            .collect();
        assert_eq!(controls[&(1, false)], Pickable::default());
        assert_eq!(controls[&(2, true)], Pickable::default());
        let connector = world
            .query::<(&scene::SkillConnector, &Node, &BackgroundColor)>()
            .iter(world)
            .find(|(connector, _, _)| connector.source == 1 && connector.segment == 0)
            .unwrap();
        assert_eq!(connector.1.height, px(2));
        assert_eq!(connector.2.0, theme::EMERALD.with_alpha(0.45));
        assert!(
            world
                .query::<(&scene::SkillJobPointText, &Text)>()
                .iter(world)
                .any(|(_, text)| text.0 == "1 points")
        );
        assert_eq!(
            world
                .query_filtered::<&Text, With<SkillPanelBank>>()
                .single(world)
                .unwrap()
                .0,
            "1"
        );
        assert_eq!(
            world
                .query_filtered::<&Text, With<scene::SkillPanelStagedCount>>()
                .single(world)
                .unwrap()
                .0,
            "1 change staged"
        );
        assert!(
            world
                .query::<(&SkillPanelCommitButton, &Pickable, &BackgroundColor)>()
                .iter(world)
                .all(|(button, pickable, background)| {
                    *pickable == Pickable::default()
                        && background.0.alpha() == 1.0
                        && if button.apply {
                            background.0 == theme::EMERALD
                        } else {
                            background.0 == theme::FIELD
                        }
                })
        );
        assert!(
            world
                .query::<(&scene::SkillNodeDimmer, &Visibility, &Pickable)>()
                .iter(world)
                .all(|(_, visibility, pickable)| {
                    *visibility == Visibility::Hidden && *pickable == Pickable::IGNORE
                })
        );
    }

    #[test]
    fn level_only_authority_refresh_preserves_viewport_and_clears_stale_plan() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7)), (2, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        let viewport = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .expect("one viewport");
        app.world_mut()
            .entity_mut(viewport)
            .insert(ScrollPosition(Vec2::new(31.0, 19.0)));
        app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(1);
        app.world_mut()
            .resource_mut::<SkillPanelStaging>()
            .pending
            .insert(2, 1);

        {
            let mut authoritative = app.world_mut().resource_mut::<SkillTreeState>();
            let first = authoritative.skills.get_mut(&1).unwrap();
            first.level = 1;
            first.upgradable = false;
            first.sp = 12;
            first.range = 4;
        }
        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
                .single(app.world())
                .expect("one viewport"),
            viewport
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(viewport).unwrap().0,
            Vec2::new(31.0, 19.0)
        );
        assert!(app.world().resource::<SkillPanelStaging>().is_empty());
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(1));
    }

    #[test]
    fn topology_changes_replace_viewport_and_clear_only_disappeared_selection() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7)), (2, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        let first = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .expect("one viewport");
        app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(1);

        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .get_mut(&2)
            .unwrap()
            .job_id = 9;
        app.update();
        let second = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        assert_ne!(second, first);
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(1));

        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .get_mut(&2)
            .unwrap()
            .requires = vec![(1, 1)];
        app.update();
        let third = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        assert_ne!(third, second);
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(1));

        app.insert_resource(tree(&[(2, node(0, 5, 9))]));
        app.update();
        let fourth = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        assert_ne!(fourth, third);
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, None);
    }

    #[test]
    fn skill_and_job_presentation_metadata_changes_replace_viewport() {
        let mut app = skills_app();
        let mut jobs = lifthrasir_data::JobData::default();
        jobs.display_names.insert(7, "Knight".to_string());
        app.insert_resource(JobSpriteRegistry::from_job_data(jobs));
        app.insert_resource(catalog("SM_BASH", "Bash"));
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        let first = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();

        app.insert_resource(catalog("SM_MAGNUM", "Magnum Break"));
        app.update();
        let second = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        assert_ne!(second, first);
        {
            let world = app.world_mut();
            assert!(
                world
                    .query::<(&scene::SkillNodeLevel, &Text)>()
                    .iter(world)
                    .any(|(marker, text)| marker.0 == 1 && text.0 == "0/5")
            );
        }

        let mut jobs = lifthrasir_data::JobData::default();
        jobs.display_names.insert(7, "Lord Knight".to_string());
        app.insert_resource(JobSpriteRegistry::from_job_data(jobs));
        app.update();
        let third = app
            .world_mut()
            .query_filtered::<Entity, With<scene::SkillCanvasViewport>>()
            .single(app.world())
            .unwrap();
        assert_ne!(third, second);
    }

    #[test]
    fn rebuild_renders_every_runtime_skill() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, node(0, 5, 7)),
            (2, node(0, 5, 7)),
            (3, node(0, 5, 9)),
        ]));
        app.world_mut().spawn(SkillsTabBody);

        app.update();

        assert_eq!(cell_count(&mut app), 3);

        let world = app.world_mut();
        let mut skill_ids: Vec<_> = world
            .query::<&SkillPanelCell>()
            .iter(world)
            .map(|cell| cell.0)
            .collect();
        skill_ids.sort_unstable();
        assert_eq!(skill_ids, vec![1, 2, 3]);
        assert_eq!(world.query::<&scene::SkillJobBand>().iter(world).count(), 2);
    }

    #[test]
    fn renders_same_job_and_cross_job_connector_segments() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, node(1, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 2)])),
            (3, with_requires(node(0, 5, 9), vec![(2, 1)])),
        ]));
        app.world_mut().spawn(SkillsTabBody);

        app.update();

        let world = app.world_mut();
        let connectors: Vec<_> = world
            .query::<&scene::SkillConnector>()
            .iter(world)
            .copied()
            .collect();
        assert_eq!(connectors.len(), 6);
        for (source, target, minimum_level) in [(1, 2, 2), (2, 3, 1)] {
            let mut segment_ids: Vec<_> = connectors
                .iter()
                .filter(|connector| connector.source == source && connector.target == target)
                .map(|connector| connector.segment)
                .collect();
            segment_ids.sort_unstable();
            assert_eq!(segment_ids, vec![0, 1, 2]);
            assert!(connectors.iter().any(|connector| {
                connector.source == source
                    && connector.target == target
                    && connector.minimum_level == minimum_level
                    && !connector.backlink
            }));
        }
    }

    #[test]
    fn staged_prerequisite_changes_connector_from_thin_unmet_to_thick_met() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();

        let connector_style = |app: &mut App| {
            let world = app.world_mut();
            world
                .query::<(&scene::SkillConnector, &Node, &BackgroundColor, &ZIndex)>()
                .iter(world)
                .find_map(|(connector, node, color, z)| {
                    (connector.source == 1 && connector.target == 2 && connector.segment == 0)
                        .then_some((node.height, color.0, z.0))
                })
                .expect("first connector segment")
        };
        assert_eq!(connector_style(&mut app), (px(1), theme::GOLD_FAINT, 1));

        app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);
        app.update();

        assert_eq!(
            connector_style(&mut app),
            (px(2), theme::EMERALD.with_alpha(0.45), 1)
        );
        let world = app.world_mut();
        assert!(
            world
                .query_filtered::<&ZIndex, With<SkillPanelCell>>()
                .iter(world)
                .all(|z| z.0 > 1)
        );
    }

    #[test]
    fn missing_prerequisite_renders_no_connector_and_stays_unavailable() {
        let mut app = skills_app();
        let missing = tree(&[(2, with_requires(node(0, 5, 7), vec![(999, 1)]))]);
        app.insert_resource(missing);
        app.world_mut().spawn(SkillsTabBody);

        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&scene::SkillConnector>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(!app.world().resource::<SkillPanelStaging>().can_raise(
            2,
            app.world().resource::<SkillTreeState>(),
            &status(100, 50),
            99,
        ));
    }

    #[test]
    fn backward_connector_is_dashed_and_muted() {
        let mut app = skills_app();
        app.insert_resource(tree(&[
            (1, with_requires(node(0, 5, 7), vec![(2, 1)])),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]));
        app.world_mut().spawn(SkillsTabBody);

        app.update();

        let world = app.world_mut();
        let pieces: Vec<_> = world
            .query::<(&scene::SkillConnector, &BackgroundColor)>()
            .iter(world)
            .filter(|(connector, _)| connector.backlink)
            .map(|(connector, color)| (*connector, color.0))
            .collect();
        assert!(pieces.len() > 3, "backlinks render as multiple dash pieces");
        assert!(pieces.iter().any(|(connector, _)| connector.dash > 0));
        assert!(
            pieces
                .iter()
                .all(|(_, color)| *color == theme::TEXT_FAINT.with_alpha(0.38))
        );
    }

    #[test]
    fn canvas_has_two_scrollbar_orientations_with_one_target() {
        use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar};

        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();

        let world = app.world_mut();
        let scroll_areas: Vec<_> = world
            .query_filtered::<Entity, With<ScrollArea>>()
            .iter(world)
            .collect();
        assert_eq!(scroll_areas.len(), 1);
        let viewport = scroll_areas[0];
        let orientations: Vec<_> = world
            .query::<&Scrollbar>()
            .iter(world)
            .map(|scrollbar| {
                assert_eq!(scrollbar.target, viewport);
                scrollbar.orientation
            })
            .collect();
        assert_eq!(orientations.len(), 2);
        assert!(orientations.contains(&ControlOrientation::Horizontal));
        assert!(orientations.contains(&ControlOrientation::Vertical));

        let frame = world
            .query_filtered::<&Node, With<scene::SkillCanvasFrame>>()
            .single(world)
            .expect("one canvas frame");
        assert_eq!(frame.height, px(300));
        let viewport_node = world
            .query_filtered::<&Node, With<scene::SkillCanvasViewport>>()
            .single(world)
            .expect("one canvas viewport");
        assert_eq!(viewport_node.height, px(300));
    }

    #[test]
    fn empty_tree_and_missing_metadata_use_stable_fallbacks() {
        let mut empty_app = skills_app();
        empty_app.insert_resource(SkillTreeState::default());
        empty_app.world_mut().spawn(SkillsTabBody);
        empty_app.update();
        let empty_world = empty_app.world_mut();
        assert_eq!(
            empty_world
                .query_filtered::<(), With<scene::SkillEmptyMessage>>()
                .iter(empty_world)
                .count(),
            1
        );
        assert!(
            empty_world
                .query::<&Text>()
                .iter(empty_world)
                .any(|text| text.0 == "No skills.")
        );

        let mut fallback_app = skills_app();
        fallback_app.insert_resource(tree(&[(77, node(0, 5, 42))]));
        fallback_app.world_mut().spawn(SkillsTabBody);
        fallback_app.update();
        let fallback_world = fallback_app.world_mut();
        let texts: HashSet<_> = fallback_world
            .query::<&Text>()
            .iter(fallback_world)
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains("Job #42"));
        assert!(texts.contains("#77"));
        assert_eq!(
            fallback_world
                .query::<&ImageNode>()
                .iter(fallback_world)
                .count(),
            0
        );
    }

    #[test]
    fn registry_job_name_toolbar_and_footer_are_rendered() {
        let mut app = skills_app();
        let mut jobs = lifthrasir_data::JobData::default();
        jobs.display_names.insert(7, "Knight".to_string());
        app.insert_resource(JobSpriteRegistry::from_job_data(jobs));
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();

        let world = app.world_mut();
        let texts: HashSet<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        for expected in [
            "Requirements flow  →  left to right",
            "Skill Points",
            "Reset Plan",
            "0 changes staged",
            "Apply",
            "Knight",
        ] {
            assert!(texts.contains(expected), "missing {expected:?}");
        }
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

    fn drag_start_event(target: Entity, window: Entity) -> Pointer<DragStart> {
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
            DragStart {
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
            },
            target,
        )
    }

    #[test]
    fn rendered_cell_preserves_cast_modal_and_hotbar_interactions() {
        let mut app = skills_app();
        app.add_message::<SkillCastRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<LastSkillPanelClick>();
        app.init_resource::<Time>();
        app.init_resource::<HotbarDrag>();
        app.insert_resource(tree(&[(42, node(1, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        let window = app.world_mut().spawn(Window::default()).id();
        app.update();

        let cell = app
            .world_mut()
            .query_filtered::<Entity, With<SkillPanelCell>>()
            .single(app.world())
            .expect("one rendered skill cell");
        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        assert_eq!(
            app.world()
                .resource::<Messages<SkillCastRequested>>()
                .iter_current_update_messages()
                .count(),
            1
        );

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));
        assert!(
            app.world()
                .resource::<Messages<ShowInfoModal>>()
                .iter_current_update_messages()
                .any(|message| message.target == InfoTarget::Skill(42))
        );

        app.world_mut().trigger(drag_start_event(cell, window));
        assert_eq!(
            app.world().resource::<HotbarDrag>().payload,
            Some(HotbarSlot::Skill(42))
        );
    }

    #[test]
    fn rendered_reset_plan_discards_staging_without_a_request() {
        let mut app = skills_app();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn(SkillsTabBody);
        app.update();
        app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);

        let reset = {
            let world = app.world_mut();
            let mut query = world.query::<(&Text, &ChildOf)>();
            query
                .iter(world)
                .find_map(|(text, parent)| (text.0 == "Reset Plan").then_some(parent.parent()))
                .expect("rendered Reset Plan button")
        };
        let window = app.world_mut().spawn(Window::default()).id();
        app.world_mut()
            .trigger(click_event(reset, window, PointerButton::Primary));

        assert!(app.world().resource::<SkillPanelStaging>().is_empty());
    }

    #[test]
    fn disabled_stepper_arrows_are_not_pickable() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(scene::stepper(1, true, false))
            .expect("disabled arrow spawns");
        app.world_mut()
            .spawn_scene(scene::stepper(2, true, true))
            .expect("enabled arrow spawns");
        app.update();

        let world = app.world_mut();
        let arrows: HashMap<_, _> = world
            .query::<(&SkillPanelStepper, &Pickable)>()
            .iter(world)
            .map(|(stepper, pickable)| (stepper.skill_id, *pickable))
            .collect();
        assert_eq!(arrows[&1], Pickable::IGNORE);
        assert_eq!(arrows[&2], Pickable::default());
    }

    #[test]
    fn secondary_stepper_click_bubbles_to_cell_without_changing_staging() {
        let mut app = App::new();
        app.add_message::<SkillCastRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<SkillPanelStaging>();
        app.init_resource::<SkillPanelUi>();
        app.init_resource::<LastSkillPanelClick>();
        app.init_resource::<Time>();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);
        let cell = app
            .world_mut()
            .spawn(SkillPanelCell(1))
            .observe(on_cell_click)
            .id();
        let stepper = app
            .world_mut()
            .spawn((
                SkillPanelStepper {
                    skill_id: 1,
                    raise: false,
                },
                ChildOf(cell),
            ))
            .observe(on_stepper)
            .id();
        let window = app.world_mut().spawn(Window::default()).id();
        app.world_mut().flush();

        app.world_mut()
            .trigger(click_event(stepper, window, PointerButton::Secondary));

        assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);
        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        assert!(
            messages
                .iter_current_update_messages()
                .any(|message| message.target == InfoTarget::Skill(1))
        );
    }

    #[test]
    fn primary_stepper_click_changes_staging_without_reaching_the_cell() {
        let mut app = App::new();
        app.add_message::<SkillCastRequested>();
        app.add_message::<ShowInfoModal>();
        app.init_resource::<SkillPanelStaging>();
        app.init_resource::<SkillPanelUi>();
        app.insert_resource(LastSkillPanelClick {
            skill_id: 1,
            at: Duration::ZERO,
        });
        app.init_resource::<Time>();
        app.insert_resource(tree(&[(1, node(0, 5, 7))]));
        app.world_mut().spawn((
            CharacterStatus {
                base_level: 1,
                job_level: 1,
                skill_point: 1,
                ..default()
            },
            LocalPlayer,
        ));
        let cell = app
            .world_mut()
            .spawn(SkillPanelCell(1))
            .observe(on_cell_click)
            .id();
        let stepper = app
            .world_mut()
            .spawn((
                SkillPanelStepper {
                    skill_id: 1,
                    raise: true,
                },
                ChildOf(cell),
            ))
            .observe(on_stepper)
            .id();
        let window = app.world_mut().spawn(Window::default()).id();
        app.world_mut().flush();

        app.world_mut()
            .trigger(click_event(stepper, window, PointerButton::Primary));

        assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);
        assert_eq!(app.world().resource::<SkillPanelUi>().selected, None);
        assert_eq!(
            app.world()
                .resource::<Messages<SkillCastRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Messages<ShowInfoModal>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    #[test]
    fn decrement_observer_rejects_prerequisite_until_dependent_is_removed() {
        let mut app = App::new();
        app.init_resource::<SkillPanelStaging>();
        app.insert_resource(tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]));
        app.world_mut().resource_mut::<SkillPanelStaging>().pending =
            HashMap::from([(1, 1), (2, 1)]);
        let prerequisite = app
            .world_mut()
            .spawn(SkillPanelStepper {
                skill_id: 1,
                raise: false,
            })
            .observe(on_stepper)
            .id();
        let dependent = app
            .world_mut()
            .spawn(SkillPanelStepper {
                skill_id: 2,
                raise: false,
            })
            .observe(on_stepper)
            .id();
        let window = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(click_event(prerequisite, window, PointerButton::Primary));
        assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);

        app.world_mut()
            .trigger(click_event(dependent, window, PointerButton::Primary));
        app.world_mut()
            .trigger(click_event(prerequisite, window, PointerButton::Primary));
        assert!(app.world().resource::<SkillPanelStaging>().is_empty());
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
