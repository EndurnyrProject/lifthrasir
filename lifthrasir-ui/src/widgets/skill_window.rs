//! Skills window: a read-only render of the server's authoritative skill tree.
//!
//! Mirrors `status_window.rs` (glass panel, draggable titlebar, Alt-chord toggle)
//! but its body is data-driven: the tab strip, grid, connector overlay, and info
//! panel are rebuilt from `SkillTreeState` (+ `SkillCatalog` for icon/name/desc and
//! the job registry for tab labels) whenever the tree or selection changes. The
//! footer reflects `CharacterStatus.skill_point`. Staging (the `+`/`-` steppers and
//! Reset/Apply) is inert here — those buttons exist visually but do nothing until a
//! later task wires them.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::skill::{form, layout, target, Form, Placement, SkillTreeState};
use game_engine::infrastructure::job::registry::JobSpriteRegistry;
use game_engine::infrastructure::skill::SkillCatalog;
use leafwing_input_manager::prelude::ActionState;

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

/// Active tab (a `job_id`) and selected skill. Read-only this task; the grid and
/// info panel rebuild off changes here.
#[derive(Resource, Default)]
struct SkillUi {
    tab: Option<u32>,
    selected: Option<u32>,
}

pub struct SkillWindowPlugin;

impl Plugin for SkillWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillUi>();
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
            width: Val::Px(34.0),
            padding: UiRect::axes(Val::ZERO, Val::Px(8.0)),
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
    );
    spawn_footer_button(
        commands,
        actions,
        "Apply",
        theme::EMERALD,
        theme::EMERALD_INK,
        font,
    );
}

/// A footer action button. Inert this task — its click behaviour is added later.
fn spawn_footer_button(
    commands: &mut Commands,
    actions: Entity,
    text: &str,
    bg: Color,
    fg: Color,
    font: &Handle<Font>,
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
            Pickable::default(),
            ChildOf(actions),
        ))
        .id();
    commands.spawn((theme::label(text, font.clone(), 11.5, fg), ChildOf(button)));
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

/// Rebuilds the grid cells for the active tab on tree/selection change.
fn rebuild_grid(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillUi>,
    catalog: Option<Res<SkillCatalog>>,
    asset_server: Res<AssetServer>,
    grid: Query<(Entity, Option<&Children>), With<SkillGrid>>,
) {
    if !tree.is_changed() && !ui.is_changed() {
        return;
    }
    let Ok((grid, children)) = grid.single() else {
        return;
    };
    despawn_children(&mut commands, children);

    let Some(tab) = ui.tab else {
        return;
    };
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
    selected: bool,
    catalog: Option<&SkillCatalog>,
    asset_server: &AssetServer,
    font: &Handle<Font>,
) {
    let Some(node) = tree.skills.get(&skill_id) else {
        return;
    };
    let learned = node.level > 0;
    let maxed = node.level >= node.max_level && node.max_level > 0;
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
        node.level,
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

    spawn_cell_stepper(commands, cell, node.level, node.max_level, font);

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

/// `◄ lv/max ►` stepper row. The arrows are visual only this task.
fn spawn_cell_stepper(
    commands: &mut Commands,
    cell: Entity,
    level: u32,
    max: u32,
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
    commands.spawn((
        theme::label("\u{25C4}", font.clone(), 9.0, theme::TEXT_FAINT),
        ChildOf(row),
    ));
    commands.spawn((
        theme::label(
            format_level(level, max),
            font.clone(),
            9.5,
            theme::TEXT_FAINT,
        ),
        ChildOf(row),
    ));
    commands.spawn((
        theme::label("\u{25BA}", font.clone(), 9.0, theme::TEXT_FAINT),
        ChildOf(row),
    ));
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

fn on_cell_click(click: On<Pointer<Click>>, cells: Query<&SkillCell>, mut ui: ResMut<SkillUi>) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    ui.selected = Some(cell.0);
}

/// Rebuilds the connector overlay: one orthogonal `Node` segment per in-tab
/// `requires` edge, colored met/unmet from the server levels.
fn rebuild_connectors(
    commands: &mut Commands,
    layer: Entity,
    tab: u32,
    tree: &SkillTreeState,
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
            let met = tree
                .skills
                .get(&prereq)
                .is_some_and(|p| p.level >= min_level);
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

/// Rebuilds the info panel for the selected skill on tree/selection change.
fn rebuild_info_panel(
    mut commands: Commands,
    tree: Res<SkillTreeState>,
    ui: Res<SkillUi>,
    catalog: Option<Res<SkillCatalog>>,
    asset_server: Res<AssetServer>,
    panel: Query<(Entity, Option<&Children>), With<SkillInfoPanel>>,
    connectors: Query<(Entity, Option<&Children>), With<SkillConnectorLayer>>,
) {
    if !tree.is_changed() && !ui.is_changed() {
        return;
    }
    if let Ok((layer, children)) = connectors.single() {
        rebuild_connector_layer(&mut commands, layer, children, &ui, &tree);
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
) {
    despawn_children(commands, children);
    let Some(tab) = ui.tab else {
        return;
    };
    let placements = layout(tree);
    rebuild_connectors(commands, layer, tab, tree, &placements);
}

fn spawn_info_content(
    commands: &mut Commands,
    panel: Entity,
    skill_id: u32,
    tree: &SkillTreeState,
    catalog: Option<&SkillCatalog>,
    font: &Handle<Font>,
) {
    let Some(node) = tree.skills.get(&skill_id) else {
        return;
    };
    let passive = form(node.inf_type) == Form::Passive;

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
            format!("{level_label} {}", format_level(node.level, node.max_level)),
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
        spawn_requires(commands, panel, node.requires.clone(), tree, catalog, font);
    }

    if let Some(meta) = catalog.and_then(|c| c.get(skill_id)) {
        for line in &meta.description {
            commands.spawn((
                theme::label(line.clone(), font.clone(), 10.0, theme::TEXT_DIM),
                ChildOf(panel),
            ));
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
fn spawn_requires(
    commands: &mut Commands,
    panel: Entity,
    requires: Vec<(u32, u32)>,
    tree: &SkillTreeState,
    catalog: Option<&SkillCatalog>,
    font: &Handle<Font>,
) {
    commands.spawn((
        theme::label("Requires", font.clone(), 9.0, theme::TEXT_FAINT),
        ChildOf(panel),
    ));
    for (prereq, min_level) in requires {
        let met = tree
            .skills
            .get(&prereq)
            .is_some_and(|p| p.level >= min_level);
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

/// Reflects `CharacterStatus.skill_point` into the footer value, on change only.
fn update_skill_footer(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut bank: Query<&mut Text, With<SkillPointBank>>,
) {
    let Ok(status) = player.single() else {
        return;
    };
    let Ok(mut text) = bank.single_mut() else {
        return;
    };
    set_text(&mut text, status.skill_point.to_string());
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

fn reset_ui(mut ui: ResMut<SkillUi>) {
    *ui = SkillUi::default();
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
    fn footer_reflects_skill_point() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let bank = app.world_mut().spawn((Text::new(""), SkillPointBank)).id();
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
    }
}
