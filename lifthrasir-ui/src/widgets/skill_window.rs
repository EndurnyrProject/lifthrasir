//! Skills window: renders the server's authoritative skill tree and stages
//! skill-point spends locally before committing them.
//!
//! Mirrors `status_window.rs` (glass panel, draggable titlebar, Alt-chord toggle)
//! but its body is data-driven: the tab strip, grid, connector overlay, and info
//! panel are rebuilt from `SkillTreeState` (+ `SkillCatalog` for icon/name/desc and
//! the job registry for tab labels) whenever the tree, selection, or `SkillStaging`
//! changes. The `+`/`-` steppers stage levels into `SkillStaging` (fully
//! client-evaluated prereq gates so a staged prereq unlocks its dependent in the
//! same batch); Reset discards them; Apply emits one `SkillLearnRequested` per
//! staged level in prereq-first order, then clears. The server resends the full
//! tree to reconcile. The footer shows points remaining after the staged spend.

use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::character::events::SkillLearnRequested;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::skill::{
    form, layout, target, Form, Placement, SkillCastRequested, SkillTreeState,
};
use game_engine::infrastructure::job::registry::JobSpriteRegistry;
use game_engine::infrastructure::skill::SkillCatalog;
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashMap;
use std::time::Duration;

use crate::rich_text::spawn_colored_text;
use crate::theme;
use crate::widgets::draggable::make_draggable;

/// Pixel footprint of one grid cell, matching the mock's `CELL_W`/`CELL_H`.
const CELL_W: f32 = 62.0;
const CELL_H: f32 = 82.0;
/// Icon-centre offset within a cell, used to anchor connector segments.
const IC_X: f32 = 31.0;
const IC_Y: f32 = 26.0;

/// Marks the skill-window root so the toggle/close systems can flip its visibility.
#[derive(Component)]
pub struct SkillWindowRoot;

/// Marks the vertical tab-strip container; rebuilt from the tree's `job_id`s.
#[derive(Component)]
struct SkillTabStrip;

/// Marks the grid container; rebuilt with one cell per skill in the active tab.
#[derive(Component)]
struct SkillGrid;

/// Marks the absolutely-positioned connector overlay sibling of the grid.
#[derive(Component)]
struct SkillConnectorLayer;

/// Marks the collapsible right info panel; rebuilt for the selected skill.
#[derive(Component)]
struct SkillInfoPanel;

/// Marks the "Skill Points" footer value text.
#[derive(Component)]
struct SkillPointBank;

/// Marks a tab button with the `job_id` it selects.
#[derive(Component, Clone, Copy)]
struct SkillTab(u32);

/// Marks a grid cell with the `skill_id` it shows so clicks can select it.
#[derive(Component, Clone, Copy)]
struct SkillCell(u32);

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Resource, Default)]
struct LastSkillClick {
    skill_id: u32,
    at: Duration,
}

fn is_cast_double_click(last: &LastSkillClick, skill_id: u32, now: Duration) -> bool {
    last.skill_id == skill_id && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

/// Marks a `◄`/`►` stepper button with the skill it adjusts and its direction.
#[derive(Component, Clone, Copy)]
struct Stepper {
    skill_id: u32,
    raise: bool,
}

/// Marks the Reset/Apply footer buttons so their dim-state tracks staging.
#[derive(Component)]
struct CommitButton;

/// Active tab (a `job_id`) and selected skill. Read-only this task; the grid and
/// info panel rebuild off changes here.
#[derive(Resource, Default)]
struct SkillUi {
    tab: Option<u32>,
    selected: Option<u32>,
}

/// Locally staged skill-point spends this session: `skill_id -> staged +levels`.
/// A skill point is a flat 1 per level — no cost curve, unlike the status window.
#[derive(Resource, Default)]
pub struct SkillStaging {
    pending: HashMap<u32, u32>,
}

impl SkillStaging {
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

    /// Fully client-evaluated raise gate (design D5): a prereq staged earlier in
    /// the same batch unlocks its dependent immediately.
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

/// Longest prerequisite-chain depth over the full `requires` graph. Returns
/// `None` on a cycle (degrades to depth 0 at the call site).
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

pub struct SkillWindowPlugin;

impl Plugin for SkillWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillUi>();
        app.init_resource::<SkillStaging>();
        app.init_resource::<LastSkillClick>();
        app.add_systems(
            Update,
            toggle_skill_window.run_if(in_state(GameState::InGame).and(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                ensure_default_tab,
                rebuild_tab_strip,
                rebuild_grid,
                rebuild_info_panel,
                update_skill_footer,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), reset_ui);
    }
}

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

/// Builds the skills-window shell under `parent`: a glass panel with a draggable
/// titlebar (rune + "Skills" + Skill-Info toggle + close), a body row (tab strip /
/// grid+connectors / collapsible info panel), and a Skill-Points footer with the
/// (visual-only) Reset/Apply buttons. Hidden by default.
pub fn spawn_skill_window(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let root = commands
        .spawn((
            SkillWindowRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(260.0),
                top: Val::Px(100.0),
                width: Val::Px(672.0),
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
    spawn_body(commands, root);
    spawn_footer(commands, root, &font_body);
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
        theme::icon(asset_server, "rune", 16.0, theme::GOLD),
        ChildOf(titlebar),
    ));
    commands.spawn((
        theme::label("Skills", font_title.clone(), 15.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(titlebar),
    ));

    spawn_info_toggle(commands, asset_server, titlebar, font_title);
    spawn_close(commands, asset_server, titlebar);

    make_draggable(commands, titlebar, root);
}

/// "Skill Info" toggle: collapses/expands the right info panel.
fn spawn_info_toggle(
    commands: &mut Commands,
    asset_server: &AssetServer,
    titlebar: Entity,
    font: &Handle<Font>,
) {
    let toggle = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                height: Val::Px(21.0),
                padding: UiRect::axes(Val::Px(9.0), Val::ZERO),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::EMERALD),
            Pickable::default(),
            ChildOf(titlebar),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "info", 11.0, theme::EMERALD_BRI),
        ChildOf(toggle),
    ));
    commands.spawn((
        theme::label("Skill Info", font.clone(), 9.5, theme::EMERALD_BRI),
        ChildOf(toggle),
    ));
    commands.entity(toggle).observe(on_info_toggle);
}

fn on_info_toggle(_: On<Pointer<Click>>, mut panel: Query<&mut Visibility, With<SkillInfoPanel>>) {
    let Ok(mut visibility) = panel.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

fn spawn_close(commands: &mut Commands, asset_server: &AssetServer, titlebar: Entity) {
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
        |_: On<Pointer<Click>>, mut window: Query<&mut Visibility, With<SkillWindowRoot>>| {
            if let Ok(mut visibility) = window.single_mut() {
                *visibility = Visibility::Hidden;
            }
        },
    );
}

/// Body row: vertical tab strip / grid + connector overlay / collapsible info panel.
fn spawn_body(commands: &mut Commands, root: Entity) {
    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                min_height: Val::Px(280.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        SkillTabStrip,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(2.0),
            width: Val::Px(86.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(8.0)),
            border: UiRect {
                right: Val::Px(1.0),
                ..default()
            },
            ..default()
        },
        BorderColor::all(theme::STROKE),
        Pickable::IGNORE,
        ChildOf(body),
    ));

    spawn_grid_wrap(commands, body);
    spawn_info_panel(commands, body);
}

/// Grid scroll area hosting the connector overlay (z-behind) and the cell grid.
fn spawn_grid_wrap(commands: &mut Commands, body: Entity) {
    let wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                padding: UiRect::all(Val::Px(7.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();

    let stack = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(wrap),
        ))
        .id();

    commands.spawn((
        SkillConnectorLayer,
        Node {
            position_type: PositionType::Absolute,
            left: Val::ZERO,
            top: Val::ZERO,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(stack),
    ));

    commands.spawn((
        SkillGrid,
        Node {
            position_type: PositionType::Relative,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(stack),
    ));
}

fn spawn_info_panel(commands: &mut Commands, body: Entity) {
    commands.spawn((
        SkillInfoPanel,
        Node {
            width: Val::Px(190.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(13.0)),
            border: UiRect {
                left: Val::Px(1.0),
                ..default()
            },
            ..default()
        },
        BorderColor::all(theme::STROKE),
        Pickable::IGNORE,
        ChildOf(body),
    ));
}

/// Footer: "Skill Points" label + value, plus the (visual-only) Reset/Apply buttons.
fn spawn_footer(commands: &mut Commands, root: Entity, font: &Handle<Font>) {
    let foot = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                height: Val::Px(42.0),
                padding: UiRect::axes(Val::Px(14.0), Val::ZERO),
                column_gap: Val::Px(9.0),
                border: UiRect {
                    top: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        theme::label("Skill Points", font.clone(), 10.0, theme::TEXT_FAINT),
        ChildOf(foot),
    ));
    commands.spawn((
        theme::label("0", font.clone(), 16.0, theme::EMERALD_BRI),
        SkillPointBank,
        ChildOf(foot),
    ));

    let actions = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(7.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(foot),
        ))
        .id();
    spawn_footer_button(
        commands,
        actions,
        "Reset",
        theme::FIELD,
        theme::TEXT_DIM,
        font,
        on_reset,
    );
    spawn_footer_button(
        commands,
        actions,
        "Apply",
        theme::EMERALD,
        theme::EMERALD_INK,
        font,
        on_apply,
    );
}

/// A footer action button. Its click behaviour comes from `observer`; the
/// `CommitButton` marker lets `update_skill_footer` dim it when nothing is staged.
fn spawn_footer_button<M>(
    commands: &mut Commands,
    actions: Entity,
    text: &str,
    bg: Color,
    fg: Color,
    font: &Handle<Font>,
    observer: impl IntoObserverSystem<Pointer<Click>, (), M>,
) {
    let button = commands
        .spawn((
            Node {
                height: Val::Px(28.0),
                padding: UiRect::axes(Val::Px(15.0), Val::ZERO),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(bg),
            CommitButton,
            Pickable::default(),
            ChildOf(actions),
        ))
        .id();
    commands.spawn((theme::label(text, font.clone(), 11.5, fg), ChildOf(button)));
    commands.entity(button).observe(observer);
}

/// Reset: discard all staged levels without contacting the server.
fn on_reset(_: On<Pointer<Click>>, mut staging: ResMut<SkillStaging>) {
    staging.clear();
}

/// Apply: emit one `SkillLearnRequested` per staged level in prereq-first order,
/// then clear staging. The resent `SkillList` reconciles the grid. No-op when empty.
fn on_apply(
    _: On<Pointer<Click>>,
    mut staging: ResMut<SkillStaging>,
    tree: Res<SkillTreeState>,
    mut writer: MessageWriter<SkillLearnRequested>,
) {
    for skill_id in apply_order(&staging.pending, &tree) {
        writer.write(SkillLearnRequested { skill_id });
    }
    staging.clear();
}

/// Seed the active tab with the first present `job_id` once the tree arrives.
fn ensure_default_tab(tree: Res<SkillTreeState>, mut ui: ResMut<SkillUi>) {
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

/// Rebuilds the vertical tab strip (one button per `job_id`) on tree/selection change.
fn rebuild_tab_strip(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillUi>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    asset_server: Res<AssetServer>,
    strip: Query<(Entity, Option<&Children>), With<SkillTabStrip>>,
) {
    if !tree.is_changed() && !ui.is_changed() {
        return;
    }
    let Ok((strip, children)) = strip.single() else {
        return;
    };
    despawn_children(&mut commands, children);

    let font = asset_server.load(theme::FONT_BODY);
    let registry = job_registry.as_deref();
    for (ordinal, job_id) in tab_ids(&tree).into_iter().enumerate() {
        let active = ui.tab == Some(job_id);
        let color = if active {
            theme::EMERALD_BRI
        } else {
            theme::TEXT_FAINT
        };
        let tab = commands
            .spawn((
                SkillTab(job_id),
                Node {
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(9.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(if active {
                    theme::EMERALD_INK
                } else {
                    Color::NONE
                }),
                Pickable::default(),
                ChildOf(strip),
            ))
            .id();
        commands.spawn((
            theme::label(
                tab_label(job_id, ordinal, registry),
                font.clone(),
                10.0,
                color,
            ),
            ChildOf(tab),
        ));
        commands.entity(tab).observe(on_tab_click);
    }
}

fn on_tab_click(click: On<Pointer<Click>>, tabs: Query<&SkillTab>, mut ui: ResMut<SkillUi>) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = Some(tab.0);
    ui.selected = None;
}

/// Rebuilds the grid cells for the active tab on tree/selection/staging change.
#[allow(clippy::too_many_arguments)]
fn rebuild_grid(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillUi>,
    staging: Res<SkillStaging>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    catalog: Option<Res<SkillCatalog>>,
    asset_server: Res<AssetServer>,
    grid: Query<(Entity, Option<&Children>), With<SkillGrid>>,
) {
    if !tree.is_changed() && !ui.is_changed() && !staging.is_changed() {
        return;
    }
    let Ok((grid, children)) = grid.single() else {
        return;
    };
    despawn_children(&mut commands, children);

    let Some(tab) = ui.tab else {
        return;
    };
    let status = player.single().ok();
    let font = asset_server.load(theme::FONT_BODY);
    let placements = layout(&tree);
    let catalog = catalog.as_deref();
    for (&skill_id, placement) in &placements {
        if placement.tab != tab {
            continue;
        }
        spawn_cell(
            &mut commands,
            grid,
            skill_id,
            placement,
            &tree,
            &staging,
            status,
            ui.selected == Some(skill_id),
            catalog,
            &asset_server,
            &font,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_cell(
    commands: &mut Commands,
    grid: Entity,
    skill_id: u32,
    placement: &Placement,
    tree: &SkillTreeState,
    staging: &SkillStaging,
    status: Option<&CharacterStatus>,
    selected: bool,
    catalog: Option<&SkillCatalog>,
    asset_server: &AssetServer,
    font: &Handle<Font>,
) {
    let Some(node) = tree.skills.get(&skill_id) else {
        return;
    };
    let effective = staging.effective_level(skill_id, tree);
    let learned = effective > 0;
    let maxed = effective >= node.max_level && node.max_level > 0;
    let icon_color = cell_icon_color(learned, maxed);

    let cell = commands
        .spawn((
            SkillCell(skill_id),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(placement.col as f32 * CELL_W),
                top: Val::Px(placement.row as f32 * CELL_H),
                width: Val::Px(CELL_W),
                height: Val::Px(CELL_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(1.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(if selected {
                theme::EMERALD_INK
            } else {
                Color::NONE
            }),
            Pickable::default(),
            ChildOf(grid),
        ))
        .id();

    spawn_cell_icon(
        commands,
        cell,
        skill_id,
        effective,
        learned,
        icon_color,
        catalog,
        asset_server,
        font,
    );

    let name = skill_name(skill_id, catalog);
    commands.spawn((
        theme::label(name, font.clone(), 8.5, icon_color),
        ChildOf(cell),
    ));

    let can_raise = status.is_some_and(|s| staging.can_raise(skill_id, tree, s, s.skill_point));
    let can_lower = staging.staged(skill_id) > 0;
    spawn_cell_stepper(
        commands,
        cell,
        skill_id,
        effective,
        node.max_level,
        can_raise,
        can_lower,
        font,
    );

    commands.entity(cell).observe(on_cell_click);
}

/// Icon tile: the skill icon (when the catalog resolves it) plus a level badge.
#[allow(clippy::too_many_arguments)]
fn spawn_cell_icon(
    commands: &mut Commands,
    cell: Entity,
    skill_id: u32,
    level: u32,
    learned: bool,
    icon_color: Color,
    catalog: Option<&SkillCatalog>,
    asset_server: &AssetServer,
    font: &Handle<Font>,
) {
    let tile = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(42.0),
                height: Val::Px(42.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(if learned {
                theme::EMERALD
            } else {
                theme::STROKE
            }),
            Pickable::IGNORE,
            ChildOf(cell),
        ))
        .id();

    if let Some(path) = catalog.and_then(|c| c.icon_path(skill_id)) {
        commands.spawn((
            ImageNode::new(asset_server.load(path)),
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(tile),
        ));
    }

    if learned {
        commands.spawn((
            theme::label(level.to_string(), font.clone(), 9.0, icon_color),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(3.0),
                bottom: Val::Px(2.0),
                ..default()
            },
            ChildOf(tile),
        ));
    }
}

/// `◄ lv/max ►` stepper row. The arrows stage/unstage a level; each dims when its
/// direction is unavailable.
#[allow(clippy::too_many_arguments)]
fn spawn_cell_stepper(
    commands: &mut Commands,
    cell: Entity,
    skill_id: u32,
    level: u32,
    max: u32,
    can_raise: bool,
    can_lower: bool,
    font: &Handle<Font>,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(cell),
        ))
        .id();
    spawn_stepper_arrow(commands, row, skill_id, false, can_lower, font);
    commands.spawn((
        theme::label(
            format_level(level, max),
            font.clone(),
            9.5,
            theme::TEXT_FAINT,
        ),
        ChildOf(row),
    ));
    spawn_stepper_arrow(commands, row, skill_id, true, can_raise, font);
}

/// A single `◄`/`►` arrow button: a `Stepper`-marked clickable, dimmed when disabled.
fn spawn_stepper_arrow(
    commands: &mut Commands,
    row: Entity,
    skill_id: u32,
    raise: bool,
    enabled: bool,
    font: &Handle<Font>,
) {
    let glyph = if raise { "\u{25BA}" } else { "\u{25C4}" };
    let color = if enabled {
        theme::EMERALD_BRI
    } else {
        theme::TEXT_FAINT
    };
    let button = commands
        .spawn((
            Stepper { skill_id, raise },
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::default(),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        theme::label(glyph, font.clone(), 9.0, color),
        ChildOf(button),
    ));
    commands.entity(button).observe(on_stepper);
}

/// `◄`/`►` observer: stages or unstages a level via `SkillStaging`. Reads the
/// player status so `can_raise`'s point/level gates are evaluated from the source.
fn on_stepper(
    click: On<Pointer<Click>>,
    steppers: Query<&Stepper>,
    tree: Res<SkillTreeState>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<SkillStaging>,
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

fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&SkillCell>,
    mut ui: ResMut<SkillUi>,
    time: Res<Time>,
    mut last: ResMut<LastSkillClick>,
    mut cast_writer: MessageWriter<SkillCastRequested>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    ui.selected = Some(cell.0);
    let now = time.elapsed();
    if is_cast_double_click(&last, cell.0, now) {
        cast_writer.write(SkillCastRequested { skill_id: cell.0 });
    }
    *last = LastSkillClick {
        skill_id: cell.0,
        at: now,
    };
}

/// Rebuilds the connector overlay: one orthogonal `Node` segment per in-tab
/// `requires` edge, colored met/unmet from the effective (server + staged) levels.
fn rebuild_connectors(
    commands: &mut Commands,
    layer: Entity,
    tab: u32,
    tree: &SkillTreeState,
    staging: &SkillStaging,
    placements: &std::collections::HashMap<u32, Placement>,
) {
    for (&skill_id, placement) in placements {
        if placement.tab != tab {
            continue;
        }
        let Some(node) = tree.skills.get(&skill_id) else {
            continue;
        };
        for &(prereq, min_level) in &node.requires {
            let Some(prereq_place) = placements.get(&prereq) else {
                continue;
            };
            if prereq_place.tab != tab {
                continue;
            }
            let met = staging.effective_level(prereq, tree) >= min_level;
            spawn_connector(commands, layer, prereq_place, placement, met);
        }
    }
}

/// Two orthogonal segments (vertical then horizontal) from a prereq cell's icon
/// centre to the dependent cell's icon centre.
fn spawn_connector(
    commands: &mut Commands,
    layer: Entity,
    from: &Placement,
    to: &Placement,
    met: bool,
) {
    let x1 = from.col as f32 * CELL_W + IC_X;
    let y1 = from.row as f32 * CELL_H + IC_Y;
    let x2 = to.col as f32 * CELL_W + IC_X;
    let y2 = to.row as f32 * CELL_H + IC_Y;
    let color = if met {
        theme::EMERALD.with_alpha(0.32)
    } else {
        theme::STROKE
    };

    let (top, height) = ordered_span(y1, y2);
    commands.spawn((
        segment(Val::Px(x1), Val::Px(top), Val::Px(1.0), Val::Px(height)),
        BackgroundColor(color),
        Pickable::IGNORE,
        ChildOf(layer),
    ));

    let (left, width) = ordered_span(x1, x2);
    commands.spawn((
        segment(
            Val::Px(left),
            Val::Px(y2),
            Val::Px(width.max(1.0)),
            Val::Px(1.0),
        ),
        BackgroundColor(color),
        Pickable::IGNORE,
        ChildOf(layer),
    ));
}

/// `(start, length)` of a 1D span between two coordinates, length clamped to 0.
fn ordered_span(a: f32, b: f32) -> (f32, f32) {
    (a.min(b), (a - b).abs())
}

fn segment(left: Val, top: Val, width: Val, height: Val) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left,
        top,
        width,
        height,
        ..default()
    }
}

/// Rebuilds the info panel for the selected skill on tree/selection/staging change.
#[allow(clippy::too_many_arguments)]
fn rebuild_info_panel(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillUi>,
    staging: Res<SkillStaging>,
    catalog: Option<Res<SkillCatalog>>,
    asset_server: Res<AssetServer>,
    panel: Query<(Entity, Option<&Children>), With<SkillInfoPanel>>,
    connectors: Query<(Entity, Option<&Children>), With<SkillConnectorLayer>>,
) {
    if !tree.is_changed() && !ui.is_changed() && !staging.is_changed() {
        return;
    }
    if let Ok((layer, children)) = connectors.single() {
        rebuild_connector_layer(&mut commands, layer, children, &ui, &tree, &staging);
    }

    let Ok((panel, children)) = panel.single() else {
        return;
    };
    despawn_children(&mut commands, children);

    let font = asset_server.load(theme::FONT_BODY);
    let Some(skill_id) = ui.selected else {
        commands.spawn((
            theme::label(
                "Select a skill\nto view details",
                font.clone(),
                10.5,
                theme::TEXT_FAINT,
            ),
            ChildOf(panel),
        ));
        return;
    };
    spawn_info_content(
        &mut commands,
        panel,
        skill_id,
        &tree,
        &staging,
        catalog.as_deref(),
        &font,
    );
}

/// Despawns and rebuilds the connector overlay for the active tab.
fn rebuild_connector_layer(
    commands: &mut Commands,
    layer: Entity,
    children: Option<&Children>,
    ui: &SkillUi,
    tree: &SkillTreeState,
    staging: &SkillStaging,
) {
    despawn_children(commands, children);
    let Some(tab) = ui.tab else {
        return;
    };
    let placements = layout(tree);
    rebuild_connectors(commands, layer, tab, tree, staging, &placements);
}

fn spawn_info_content(
    commands: &mut Commands,
    panel: Entity,
    skill_id: u32,
    tree: &SkillTreeState,
    staging: &SkillStaging,
    catalog: Option<&SkillCatalog>,
    font: &Handle<Font>,
) {
    let Some(node) = tree.skills.get(&skill_id) else {
        return;
    };
    let passive = form(node.inf_type) == Form::Passive;
    let effective = staging.effective_level(skill_id, tree);

    commands.spawn((
        theme::label(
            skill_name(skill_id, catalog),
            font.clone(),
            12.5,
            theme::EMERALD_BRI,
        ),
        ChildOf(panel),
    ));
    let level_label = if passive { "Max Lv" } else { "Lv" };
    commands.spawn((
        theme::label(
            format!("{level_label} {}", format_level(effective, node.max_level)),
            font.clone(),
            10.0,
            theme::TEXT_FAINT,
        ),
        ChildOf(panel),
    ));

    spawn_info_row(commands, panel, "Form", form_label(node.inf_type), font);
    if !passive && node.sp > 0 {
        spawn_info_row(commands, panel, "SP Cost", node.sp.to_string(), font);
    }
    spawn_info_row(commands, panel, "Target", target_label(node.inf_type), font);

    if !node.requires.is_empty() {
        spawn_requires(
            commands,
            panel,
            node.requires.clone(),
            tree,
            staging,
            catalog,
            font,
        );
    }

    if let Some(meta) = catalog.and_then(|c| c.get(skill_id)) {
        for line in &meta.description {
            spawn_colored_text(commands, panel, line, font.clone(), 10.0, theme::TEXT_DIM);
        }
    }
}

fn spawn_info_row(
    commands: &mut Commands,
    panel: Entity,
    key: &str,
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
        theme::label(key, font.clone(), 9.0, theme::TEXT_FAINT),
        ChildOf(row),
    ));
    commands.spawn((
        theme::label(value, font.clone(), 10.5, theme::TEXT),
        ChildOf(row),
    ));
}

/// "Requires" list: one row per prereq with a met/unmet dot, name, and `Lv n`.
/// Met-state includes staged levels so a staged prereq reads as satisfied.
#[allow(clippy::too_many_arguments)]
fn spawn_requires(
    commands: &mut Commands,
    panel: Entity,
    requires: Vec<(u32, u32)>,
    tree: &SkillTreeState,
    staging: &SkillStaging,
    catalog: Option<&SkillCatalog>,
    font: &Handle<Font>,
) {
    commands.spawn((
        theme::label("Requires", font.clone(), 9.0, theme::TEXT_FAINT),
        ChildOf(panel),
    ));
    for (prereq, min_level) in requires {
        let met = staging.effective_level(prereq, tree) >= min_level;
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(7.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(panel),
            ))
            .id();
        let dot_color = if met {
            theme::EMERALD
        } else {
            theme::TEXT_FAINT
        };
        commands.spawn((
            theme::label("\u{25CF}", font.clone(), 6.0, dot_color),
            ChildOf(row),
        ));
        commands.spawn((
            theme::label(
                skill_name(prereq, catalog),
                font.clone(),
                10.0,
                theme::TEXT_FAINT,
            ),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(row),
        ));
        commands.spawn((
            theme::label(format!("Lv {min_level}"), font.clone(), 9.5, dot_color),
            ChildOf(row),
        ));
    }
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

/// Reflects the remaining skill points (server points minus staged spend) into the
/// footer value and dims Reset/Apply when nothing is staged. Change-detected writes.
fn update_skill_footer(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    staging: Res<SkillStaging>,
    mut bank: Query<&mut Text, With<SkillPointBank>>,
    mut commit: Query<&mut BackgroundColor, With<CommitButton>>,
) {
    let Ok(status) = player.single() else {
        return;
    };
    if let Ok(mut text) = bank.single_mut() {
        set_text(
            &mut text,
            staging.points_left(status.skill_point).to_string(),
        );
    }

    let alpha = if staging.is_empty() { 0.3 } else { 1.0 };
    for mut bg in &mut commit {
        if bg.0.alpha() != alpha {
            bg.0.set_alpha(alpha);
        }
    }
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        *text = Text::new(value);
    }
}

fn despawn_children(commands: &mut Commands, children: Option<&Children>) {
    let Some(children) = children else {
        return;
    };
    for child in children.iter() {
        commands.entity(child).despawn();
    }
}

/// Alt+S toggles the skills window between hidden and visible.
fn toggle_skill_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<SkillWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Skills) {
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

fn reset_ui(mut ui: ResMut<SkillUi>, mut staging: ResMut<SkillStaging>) {
    *ui = SkillUi::default();
    staging.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let last = LastSkillClick {
            skill_id: 5,
            at: Duration::from_millis(100),
        };
        assert!(is_cast_double_click(&last, 5, Duration::from_millis(350)));
    }

    #[test]
    fn click_different_skill_is_false() {
        let last = LastSkillClick {
            skill_id: 5,
            at: Duration::from_millis(100),
        };
        assert!(!is_cast_double_click(&last, 6, Duration::from_millis(200)));
    }

    #[test]
    fn double_click_too_far_apart_is_false() {
        let last = LastSkillClick {
            skill_id: 2,
            at: Duration::from_millis(100),
        };
        assert!(!is_cast_double_click(&last, 2, Duration::from_millis(500)));
    }

    #[test]
    fn first_click_no_prior_state_is_false() {
        let last = LastSkillClick::default();
        assert!(!is_cast_double_click(&last, 3, Duration::from_millis(5000)));
    }

    #[test]
    fn can_raise_blocked_without_points() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let staging = SkillStaging::default();
        assert!(!staging.can_raise(1, &t, &status(100, 50), 0));
        assert!(staging.can_raise(1, &t, &status(100, 50), 1));
    }

    #[test]
    fn can_raise_blocked_at_max_level() {
        let t = tree(&[(1, node(5, 5, 7))]);
        let staging = SkillStaging::default();
        assert!(!staging.can_raise(1, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_blocked_when_prereq_unmet() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let staging = SkillStaging::default();
        assert!(!staging.can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_allowed_when_prereq_staged_in_same_batch() {
        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        let mut staging = SkillStaging::default();
        staging.raise(1, &t, &status(100, 50), 99);
        assert!(staging.can_raise(2, &t, &status(100, 50), 99));
    }

    #[test]
    fn can_raise_blocked_when_base_or_job_level_too_low() {
        let t = tree(&[(1, with_levels(node(0, 5, 7), 50, 20))]);
        let staging = SkillStaging::default();
        assert!(!staging.can_raise(1, &t, &status(49, 99), 99));
        assert!(!staging.can_raise(1, &t, &status(99, 19), 99));
        assert!(staging.can_raise(1, &t, &status(50, 20), 99));
    }

    #[test]
    fn lower_clamps_at_zero_and_removes_entry() {
        let t = tree(&[(1, node(0, 5, 7))]);
        let mut staging = SkillStaging::default();
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
        let mut staging = SkillStaging::default();
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
        let order = apply_order(&pending, &t);
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn format_level_pairs_current_and_max() {
        assert_eq!(format_level(3, 10), "3/10");
        assert_eq!(format_level(0, 5), "0/5");
    }

    #[test]
    fn tab_label_falls_back_to_ordinal_when_unresolved() {
        assert_eq!(tab_label(999, 0, None), "Tier 1");
        assert_eq!(tab_label(999, 2, None), "Tier 3");
    }

    #[test]
    fn tab_ids_are_sorted_and_deduped() {
        let mut tree = SkillTreeState::default();
        tree.skills.insert(1, node(0, 5, 9));
        tree.skills.insert(2, node(0, 5, 7));
        tree.skills.insert(3, node(0, 5, 7));
        assert_eq!(tab_ids(&tree), vec![7, 9]);
    }

    #[test]
    fn cell_icon_color_tracks_state() {
        assert_eq!(cell_icon_color(false, false), theme::TEXT_FAINT);
        assert_eq!(cell_icon_color(true, false), theme::EMERALD_BRI);
        assert_eq!(cell_icon_color(true, true), theme::GOLD);
    }

    fn text_of(app: &App, e: Entity) -> String {
        app.world().get::<Text>(e).unwrap().0.clone()
    }

    #[test]
    fn footer_reflects_points_left_after_staging() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<SkillStaging>();

        let bank = app.world_mut().spawn((Text::new(""), SkillPointBank)).id();
        let reset = app
            .world_mut()
            .spawn((BackgroundColor(theme::FIELD), CommitButton))
            .id();
        app.world_mut().spawn((
            CharacterStatus {
                skill_point: 15,
                ..default()
            },
            LocalPlayer,
        ));

        app.add_systems(Update, update_skill_footer);
        app.update();
        assert_eq!(text_of(&app, bank), "15");
        assert_eq!(
            app.world().get::<BackgroundColor>(reset).unwrap().0.alpha(),
            0.3
        );

        app.world_mut().resource_mut::<SkillStaging>().pending = HashMap::from([(1, 4)]);
        app.update();
        assert_eq!(text_of(&app, bank), "11");
        assert_eq!(
            app.world().get::<BackgroundColor>(reset).unwrap().0.alpha(),
            1.0
        );
    }

    fn click_event(target: Entity, window: Entity) -> Pointer<Click> {
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
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
            },
            target,
        )
    }

    #[test]
    fn on_apply_emits_ordered_events_and_clears() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<SkillLearnRequested>();
        app.init_resource::<SkillStaging>();

        let t = tree(&[
            (1, node(0, 5, 7)),
            (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
        ]);
        app.insert_resource(t);
        app.world_mut().resource_mut::<SkillStaging>().pending = HashMap::from([(1, 2), (2, 1)]);

        let button = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(button).observe(on_apply);

        let window = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(click_event(button, window));
        app.update();

        let messages = app.world().resource::<Messages<SkillLearnRequested>>();
        let mut reader = messages.get_cursor();
        let learned: Vec<u32> = reader.read(messages).map(|m| m.skill_id).collect();

        assert_eq!(learned, vec![1, 1, 2]);
        assert!(app.world().resource::<SkillStaging>().is_empty());
    }
}
