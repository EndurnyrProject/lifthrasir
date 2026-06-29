//! Status window: local staging model for spending status points.
//!
//! Pure UI-side logic. The client replicates only the stat-point cost curve as a
//! UX estimate (`stat_point_cost`); the server stays authoritative and reconciles
//! on Save. Combat-stat formulas are deliberately not replicated.

use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::{
    CharacterStatus, StatusParameter,
};
use game_engine::domain::entities::character::events::StatIncreaseRequested;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashMap;

use crate::theme;
use crate::widgets::draggable::make_draggable;

/// Marks the status-window root so the toggle/close systems can flip its visibility.
#[derive(Component)]
pub struct StatusWindowRoot;

pub struct StatusWindowPlugin;

impl Plugin for StatusWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StatStaging>();
        app.add_systems(
            Update,
            toggle_status_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            update_status_window
                .run_if(in_state(GameState::InGame).and_then(status_inputs_changed)),
        );
    }
}

/// Gates `update_status_window`: run only when the local player's stats or the
/// staging draft change, or when the window's value elements are freshly spawned
/// (so the first open populates them). Skips the per-frame formatting otherwise.
fn status_inputs_changed(
    player: Query<(), (With<LocalPlayer>, Changed<CharacterStatus>)>,
    staging: Res<StatStaging>,
    added: Query<(), Added<StatValue>>,
) -> bool {
    !player.is_empty() || staging.is_changed() || !added.is_empty()
}

pub const PRIMARY_STATS: [StatusParameter; 6] = [
    StatusParameter::Str,
    StatusParameter::Agi,
    StatusParameter::Vit,
    StatusParameter::Int,
    StatusParameter::Dex,
    StatusParameter::Luk,
];

const STAT_CAP: u32 = 99;

/// Points staged per primary stat this session (added on top of the server base).
#[derive(Resource, Default)]
pub struct StatStaging {
    staged: HashMap<StatusParameter, u32>,
}

/// Renewal stat-point cost to raise a stat from `value` to `value + 1`.
pub fn stat_point_cost(value: u32) -> u32 {
    if value < 100 {
        2 + (value - 1) / 10
    } else {
        16 + 4 * ((value - 100) / 5)
    }
}

impl StatStaging {
    pub fn staged_value(&self, stat: StatusParameter) -> u32 {
        self.staged.get(&stat).copied().unwrap_or(0)
    }

    /// Total points spent across all staged stats, given each stat's server base.
    pub fn spent(&self, base: &HashMap<StatusParameter, u32>) -> u32 {
        self.staged
            .iter()
            .flat_map(|(stat, &staged)| {
                let start = base.get(stat).copied().unwrap_or(0);
                (0..staged).map(move |i| stat_point_cost(start + i))
            })
            .sum()
    }

    pub fn points_left(&self, status_point: u32, base: &HashMap<StatusParameter, u32>) -> u32 {
        status_point.saturating_sub(self.spent(base))
    }

    pub fn raise(
        &mut self,
        stat: StatusParameter,
        status_point: u32,
        bases: &HashMap<StatusParameter, u32>,
    ) {
        let staged = self.staged_value(stat);
        let base = bases.get(&stat).copied().unwrap_or(0);
        if can_raise(base, staged, self.points_left(status_point, bases)) {
            self.staged.insert(stat, staged + 1);
        }
    }

    pub fn lower(&mut self, stat: StatusParameter) {
        let staged = self.staged_value(stat);
        if staged > 0 {
            self.staged.insert(stat, staged - 1);
        }
    }

    pub fn clear(&mut self) {
        self.staged.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.staged.values().all(|&v| v == 0)
    }
}

/// Whether one more point can be staged: below cap and affordable.
pub fn can_raise(base: u32, staged: u32, points_left: u32) -> bool {
    let current = base + staged;
    current < STAT_CAP && stat_point_cost(current) <= points_left
}

/// Maps the current staging to one `StatIncreaseRequested` per modified primary
/// stat (`status_id` = the `StatusParameter` repr, `amount` = staged count).
fn save_messages(staging: &StatStaging) -> Vec<StatIncreaseRequested> {
    PRIMARY_STATS
        .iter()
        .filter_map(|&stat| {
            let staged = staging.staged_value(stat);
            (staged > 0).then_some(StatIncreaseRequested {
                status_id: stat as u16,
                amount: staged as u8,
            })
        })
        .collect()
}

/// Builds the `{ stat -> base value }` map for all six primary stats from the
/// server status. The staging helpers require every primary present (a missing
/// entry would read as base 0 and underflow `stat_point_cost(0)`).
fn primary_bases(status: &CharacterStatus) -> HashMap<StatusParameter, u32> {
    PRIMARY_STATS
        .iter()
        .map(|&stat| (stat, status.get_param(stat)))
        .collect()
}

/// The `SP_U*` cost parameter that pairs with a primary stat.
fn cost_param(stat: StatusParameter) -> StatusParameter {
    match stat {
        StatusParameter::Str => StatusParameter::UStr,
        StatusParameter::Agi => StatusParameter::UAgi,
        StatusParameter::Vit => StatusParameter::UVit,
        StatusParameter::Int => StatusParameter::UInt,
        StatusParameter::Dex => StatusParameter::UDex,
        _ => StatusParameter::ULuk,
    }
}

const TRAITS: [&str; 6] = ["Pow", "Sta", "Wis", "Spl", "Con", "Crt"];

/// Marks a primary stat row's value text (base + staged).
#[derive(Component, Clone, Copy)]
struct StatValue(StatusParameter);

/// Marks a primary stat row's next-point cost text.
#[derive(Component, Clone, Copy)]
struct CostLabel(StatusParameter);

/// Marks the "Status Point" bank value text.
#[derive(Component)]
struct PointBank;

/// Marks a stepper button so its observer and dim-state are keyed to a stat.
#[derive(Component, Clone, Copy)]
struct Stepper {
    stat: StatusParameter,
    raise: bool,
}

/// Marks a combat-readout cell value text.
#[derive(Component, Clone, Copy)]
enum CombatCell {
    Atk,
    Matk,
    Def,
    Mdef,
    Hit,
    Flee,
    Crit,
    Aspd,
}

/// Marks the collapsible advanced container, toggled by its header button.
#[derive(Component)]
struct AdvancedSection;

/// Marks the Save / Reset commit buttons so their dim-state tracks staging.
#[derive(Component)]
struct CommitButton;

/// Builds the status-window shell under `parent`: a 468px glass panel with a
/// titlebar (rune + "Status" + close) and an empty body, hidden by default and
/// draggable by its titlebar. Body content is filled in by a later task.
pub fn spawn_status_window(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let root = commands
        .spawn((
            StatusWindowRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(220.0),
                top: Val::Px(90.0),
                width: Val::Px(468.0),
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
        theme::icon(asset_server, "user", 16.0, theme::GOLD),
        ChildOf(titlebar),
    ));
    commands.spawn((
        theme::label("Status", font_title.clone(), 15.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(titlebar),
    ));

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
        |_: On<Pointer<Click>>, mut window: Query<&mut Visibility, With<StatusWindowRoot>>| {
            if let Ok(mut visibility) = window.single_mut() {
                *visibility = Visibility::Hidden;
            }
        },
    );

    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(12.0),
                min_height: Val::Px(60.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    spawn_body(commands, body, &font_title, &font_body);

    make_draggable(commands, titlebar, root);
}

/// Section header: a faint uppercase caption.
fn spawn_head(commands: &mut Commands, parent: Entity, text: &str, font: &Handle<Font>) {
    commands.spawn((
        theme::label(text, font.clone(), 11.0, theme::GOLD),
        Node {
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        },
        ChildOf(parent),
    ));
}

/// Fills the window body: two-pane ledger (attributes rail + combat readout) and
/// the collapsible read-only Advanced/Traits section.
fn spawn_body(
    commands: &mut Commands,
    body: Entity,
    font_title: &Handle<Font>,
    font_body: &Handle<Font>,
) {
    let ledger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();

    spawn_rail(commands, ledger, font_body);
    spawn_combat(commands, ledger, font_body);
    spawn_advanced(commands, body, font_title, font_body);
    spawn_commit_row(commands, body, font_body);
}

/// Save / Reset buttons. Save commits staged points to the server; Reset
/// discards them. Both dim when nothing is staged (see `update_status_window`).
fn spawn_commit_row(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();
    spawn_commit_button(
        commands,
        row,
        "Reset",
        theme::FIELD,
        theme::TEXT_DIM,
        font,
        on_reset,
    );
    spawn_commit_button(
        commands,
        row,
        "Save",
        theme::EMERALD,
        theme::EMERALD_INK,
        font,
        on_save,
    );
}

fn spawn_commit_button<M>(
    commands: &mut Commands,
    row: Entity,
    text: &str,
    bg: Color,
    fg: Color,
    font: &Handle<Font>,
    observer: impl IntoObserverSystem<Pointer<Click>, (), M>,
) {
    let button = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(bg),
            CommitButton,
            Pickable::default(),
            ChildOf(row),
        ))
        .id();
    commands.spawn((theme::label(text, font.clone(), 13.0, fg), ChildOf(button)));
    commands.entity(button).observe(observer);
}

/// Save: emit one `StatIncreaseRequested` per modified stat, then clear staging.
/// No-op when nothing is staged (`save_messages` returns empty).
fn on_save(
    _: On<Pointer<Click>>,
    mut staging: ResMut<StatStaging>,
    mut writer: MessageWriter<StatIncreaseRequested>,
) {
    for message in save_messages(&staging) {
        writer.write(message);
    }
    staging.clear();
}

/// Reset: discard all staged points without contacting the server.
fn on_reset(_: On<Pointer<Click>>, mut staging: ResMut<StatStaging>) {
    staging.clear();
}

/// Left rail: STR–LUK rows with steppers + cost, then the status-point bank.
fn spawn_rail(commands: &mut Commands, ledger: Entity, font: &Handle<Font>) {
    let rail = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(ledger),
        ))
        .id();

    spawn_head(commands, rail, "ATTRIBUTES", font);
    for stat in PRIMARY_STATS {
        spawn_stat_row(commands, rail, stat, font);
    }

    let bank = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Pickable::IGNORE,
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        theme::label("Status Point", font.clone(), 11.0, theme::TEXT_DIM),
        ChildOf(bank),
    ));
    commands.spawn((
        theme::label("0", font.clone(), 13.0, theme::GOLD),
        PointBank,
        ChildOf(bank),
    ));
}

/// A single STR–LUK row: key, value, `−`/`+` steppers, next-point cost.
fn spawn_stat_row(
    commands: &mut Commands,
    rail: Entity,
    stat: StatusParameter,
    font: &Handle<Font>,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        theme::label(stat.name(), font.clone(), 11.5, theme::TEXT_DIM),
        Node {
            width: Val::Px(34.0),
            ..default()
        },
        ChildOf(row),
    ));
    commands.spawn((
        theme::label("0", font.clone(), 13.0, theme::TEXT),
        StatValue(stat),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(row),
    ));
    spawn_stepper(commands, row, stat, false, font);
    spawn_stepper(commands, row, stat, true, font);
    commands.spawn((
        theme::label("", font.clone(), 10.5, theme::TEXT_FAINT),
        CostLabel(stat),
        Node {
            width: Val::Px(34.0),
            ..default()
        },
        ChildOf(row),
    ));
}

fn spawn_stepper(
    commands: &mut Commands,
    row: Entity,
    stat: StatusParameter,
    raise: bool,
    font: &Handle<Font>,
) {
    let glyph = if raise { "+" } else { "\u{2212}" };
    let button = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Stepper { stat, raise },
            Pickable::default(),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        theme::label(glyph, font.clone(), 13.0, theme::TEXT),
        ChildOf(button),
    ));
    commands.entity(button).observe(on_stepper);
}

/// Right pane: read-only combat readout, straight from `CharacterStatus`.
fn spawn_combat(commands: &mut Commands, ledger: Entity, font: &Handle<Font>) {
    let pane = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(ledger),
        ))
        .id();

    spawn_head(commands, pane, "COMBAT", font);
    let cells = [
        ("Atk", CombatCell::Atk),
        ("Matk", CombatCell::Matk),
        ("Def", CombatCell::Def),
        ("Mdef", CombatCell::Mdef),
        ("Hit", CombatCell::Hit),
        ("Flee", CombatCell::Flee),
        ("Crit", CombatCell::Crit),
        ("Aspd", CombatCell::Aspd),
    ];
    for (label, cell) in cells {
        spawn_combat_cell(commands, pane, label, cell, font);
    }
}

fn spawn_combat_cell(
    commands: &mut Commands,
    pane: Entity,
    label: &str,
    cell: CombatCell,
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
            ChildOf(pane),
        ))
        .id();
    commands.spawn((
        theme::label(label, font.clone(), 11.5, theme::TEXT_DIM),
        ChildOf(row),
    ));
    commands.spawn((
        theme::label("0", font.clone(), 12.0, theme::TEXT),
        cell,
        ChildOf(row),
    ));
}

/// Collapsible Advanced section: a header toggle over a read-only Traits rail.
fn spawn_advanced(
    commands: &mut Commands,
    body: Entity,
    font_title: &Handle<Font>,
    font_body: &Handle<Font>,
) {
    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::default(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        theme::label("Advanced Status", font_title.clone(), 11.0, theme::GOLD),
        ChildOf(header),
    ));

    let advanced = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            AdvancedSection,
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();

    spawn_head(commands, advanced, "TRAITS", font_body);
    for name in TRAITS {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(advanced),
            ))
            .id();
        commands.spawn((
            theme::label(name, font_body.clone(), 11.5, theme::TEXT_DIM),
            ChildOf(row),
        ));
        commands.spawn((
            theme::label("0", font_body.clone(), 12.0, theme::TEXT),
            ChildOf(row),
        ));
    }

    commands.entity(header).observe(
        |_: On<Pointer<Click>>, mut section: Query<&mut Visibility, With<AdvancedSection>>| {
            if let Ok(mut visibility) = section.single_mut() {
                *visibility = match *visibility {
                    Visibility::Hidden => Visibility::Visible,
                    _ => Visibility::Hidden,
                };
            }
        },
    );
}

/// `+`/`−` observer: mutates `StatStaging` via the staging helpers. Reads every
/// primary base from `CharacterStatus` so no stat is ever missing from the bases.
fn on_stepper(
    click: On<Pointer<Click>>,
    steppers: Query<&Stepper>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<StatStaging>,
) {
    let Ok(stepper) = steppers.get(click.entity) else {
        return;
    };
    let Ok(status) = player.single() else {
        return;
    };
    if stepper.raise {
        let bases = primary_bases(status);
        staging.raise(stepper.stat, status.status_point, &bases);
    } else {
        staging.lower(stepper.stat);
    }
}

/// Point-bank text, kept disjoint from the stat-value and cost-label text queries.
type PointBankText<'w, 's> =
    Query<'w, 's, &'static mut Text, (With<PointBank>, Without<StatValue>, Without<CostLabel>)>;

/// Combat-cell text, kept disjoint from every other mutable `Text` query above.
type CombatCellText<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static CombatCell),
    (Without<StatValue>, Without<CostLabel>, Without<PointBank>),
>;

/// Reflects `CharacterStatus` + `StatStaging` into the marked elements, writing
/// only on change. Combat is server-truth (no staged preview).
#[allow(clippy::too_many_arguments)]
fn update_status_window(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    staging: Res<StatStaging>,
    mut values: Query<(&mut Text, &StatValue)>,
    mut costs: Query<(&mut Text, &CostLabel), Without<StatValue>>,
    mut bank: PointBankText,
    mut combat: CombatCellText,
    mut steppers: Query<(&mut BackgroundColor, &Stepper)>,
    mut commit: Query<&mut BackgroundColor, (With<CommitButton>, Without<Stepper>)>,
) {
    let Ok(status) = player.single() else {
        return;
    };
    let bases = primary_bases(status);
    let points_left = staging.points_left(status.status_point, &bases);

    for (mut text, StatValue(stat)) in &mut values {
        let value = (status.get_param(*stat) + staging.staged_value(*stat)).to_string();
        set_text(&mut text, value);
    }

    for (mut text, CostLabel(stat)) in &mut costs {
        let base = status.get_param(*stat);
        let staged = staging.staged_value(*stat);
        let value = if base + staged >= STAT_CAP {
            "max".to_string()
        } else {
            let cost = if staged == 0 {
                status.get_param(cost_param(*stat))
            } else {
                stat_point_cost(base + staged)
            };
            format!("-{cost}")
        };
        set_text(&mut text, value);
    }

    if let Ok(mut text) = bank.single_mut() {
        set_text(&mut text, points_left.to_string());
    }

    for (mut text, cell) in &mut combat {
        let value = match cell {
            CombatCell::Atk => status.atk1 + status.atk2,
            CombatCell::Matk => status.matk1 + status.matk2,
            CombatCell::Def => status.def1 + status.def2,
            CombatCell::Mdef => status.mdef1 + status.mdef2,
            CombatCell::Hit => status.hit,
            CombatCell::Flee => status.flee1 + status.flee2,
            CombatCell::Crit => status.critical,
            CombatCell::Aspd => status.aspd,
        };
        set_text(&mut text, value.to_string());
    }

    for (mut bg, stepper) in &mut steppers {
        let base = status.get_param(stepper.stat);
        let staged = staging.staged_value(stepper.stat);
        let enabled = if stepper.raise {
            can_raise(base, staged, points_left)
        } else {
            staged > 0
        };
        let color = if enabled {
            theme::FIELD
        } else {
            theme::FIELD.with_alpha(0.3)
        };
        if bg.0 != color {
            bg.0 = color;
        }
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

/// Alt+A toggles the status window between hidden and visible.
fn toggle_status_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<StatusWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Status) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_curve_boundaries() {
        assert_eq!(stat_point_cost(1), 2);
        assert_eq!(stat_point_cost(9), 2);
        assert_eq!(stat_point_cost(10), 2);
        assert_eq!(stat_point_cost(11), 3);
        assert_eq!(stat_point_cost(99), 11);
        assert_eq!(stat_point_cost(100), 16);
        assert_eq!(stat_point_cost(104), 16);
        assert_eq!(stat_point_cost(105), 20);
    }

    #[test]
    fn points_left_after_staging_multiple_stats() {
        let bases = HashMap::from([(StatusParameter::Str, 9), (StatusParameter::Agi, 11)]);
        let mut staging = StatStaging::default();
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Agi, 100, &bases);

        assert_eq!(staging.staged_value(StatusParameter::Str), 1);
        assert_eq!(staging.staged_value(StatusParameter::Agi), 1);
        // cost(9) + cost(11) = 2 + 3 = 5 spent
        assert_eq!(staging.points_left(100, &bases), 95);
    }

    #[test]
    fn cannot_raise_at_cap() {
        assert!(!can_raise(99, 0, u32::MAX));
        assert!(!can_raise(98, 1, u32::MAX));
        assert!(can_raise(98, 0, u32::MAX));
    }

    #[test]
    fn cannot_raise_without_points() {
        // cost(50) = 2 + 49/10 = 6
        assert!(!can_raise(50, 0, 5));
        assert!(can_raise(50, 0, 6));
    }

    #[test]
    fn raise_blocked_when_points_left_below_next_cost() {
        let bases = HashMap::from([(StatusParameter::Str, 50)]);
        let mut staging = StatStaging::default();
        staging.raise(StatusParameter::Str, 5, &bases);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);
    }

    #[test]
    fn lower_clamped_at_base() {
        let mut staging = StatStaging::default();
        staging.lower(StatusParameter::Str);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);

        let bases = HashMap::from([(StatusParameter::Str, 1)]);
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Str, 100, &bases);
        assert_eq!(staging.staged_value(StatusParameter::Str), 2);
        staging.lower(StatusParameter::Str);
        staging.lower(StatusParameter::Str);
        staging.lower(StatusParameter::Str);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);
    }

    #[test]
    fn save_emits_one_message_per_modified_stat() {
        let bases = HashMap::from([(StatusParameter::Str, 10), (StatusParameter::Dex, 10)]);
        let mut staging = StatStaging::default();
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise(StatusParameter::Dex, 100, &bases);

        let mut messages = save_messages(&staging);
        messages.sort_by_key(|m| m.status_id);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].status_id, StatusParameter::Str as u16);
        assert_eq!(messages[0].amount, 2);
        assert_eq!(messages[1].status_id, StatusParameter::Dex as u16);
        assert_eq!(messages[1].amount, 1);
    }

    #[test]
    fn save_emits_nothing_when_empty() {
        assert!(save_messages(&StatStaging::default()).is_empty());
    }

    fn text_of(app: &App, e: Entity) -> String {
        app.world().get::<Text>(e).unwrap().0.clone()
    }

    #[test]
    fn update_reflects_base_plus_staged_without_touching_combat() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<StatStaging>();

        let str_value = app
            .world_mut()
            .spawn((Text::new(""), StatValue(StatusParameter::Str)))
            .id();
        let atk = app.world_mut().spawn((Text::new(""), CombatCell::Atk)).id();
        let bank = app.world_mut().spawn((Text::new(""), PointBank)).id();

        app.world_mut().spawn((
            CharacterStatus {
                str: 10,
                atk1: 30,
                atk2: 12,
                status_point: 100,
                ..default()
            },
            LocalPlayer,
        ));

        app.world_mut().resource_mut::<StatStaging>().raise(
            StatusParameter::Str,
            100,
            &HashMap::from([(StatusParameter::Str, 10)]),
        );

        app.add_systems(Update, update_status_window);
        app.update();

        assert_eq!(text_of(&app, str_value), "11");
        assert_eq!(text_of(&app, atk), "42");
        // cost(10) = 2, so one staged point spent from 100.
        assert_eq!(text_of(&app, bank), "98");
    }
}
