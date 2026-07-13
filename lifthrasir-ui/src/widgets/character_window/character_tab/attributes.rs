//! The Character tab's attributes rail + combat readout.
//!
//! Rewritten in BSN from the old `status_window`, but self-contained: it owns its
//! own staging model ([`CharStatStaging`]) and UI markers ([`CharStatValue`],
//! [`CharStepper`], [`CharCombatCell`], ...) so the old window can be deleted whole in
//! the integration task with zero dangling references. Only the shared DOMAIN types +
//! messages (`CharacterStatus`, `StatusParameter`, `StatIncreaseRequested`,
//! `LocalPlayer`) and the chrome/theme helpers are reused.
//!
//! The staging model (`stat_point_cost`, `raise`/`lower`/`spent`/`points_left`/
//! `can_raise`, `save_messages`, `primary_bases`) is a verbatim carry-over of the old
//! window's pure logic, along with its unit tests. The client replicates only the
//! stat-point cost curve as a UX estimate; the server stays authoritative and
//! reconciles on Save. Combat-stat formulas are deliberately not replicated —
//! [`CharCombatCell`] is read straight from `CharacterStatus`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use game_engine::domain::entities::character::components::status::{
    CharacterStatus, StatusParameter,
};
use game_engine::domain::entities::character::events::StatIncreaseRequested;
use game_engine::domain::entities::markers::LocalPlayer;

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

pub const PRIMARY_STATS: [StatusParameter; 6] = [
    StatusParameter::Str,
    StatusParameter::Agi,
    StatusParameter::Vit,
    StatusParameter::Int,
    StatusParameter::Dex,
    StatusParameter::Luk,
];

const STAT_CAP: u32 = 99;

// ---------------------------------------------------------------------------
// Pure staging model (verbatim carry-over from the old status window).
// ---------------------------------------------------------------------------

/// Points staged per primary stat this session (added on top of the server base).
#[derive(Resource, Default)]
pub struct CharStatStaging {
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

impl CharStatStaging {
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
fn save_messages(staging: &CharStatStaging) -> Vec<StatIncreaseRequested> {
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

// ---------------------------------------------------------------------------
// UI markers.
// ---------------------------------------------------------------------------

/// Marks a primary stat row's value text (base + staged).
#[derive(Component, Clone, Copy)]
pub struct CharStatValue(pub StatusParameter);

impl Default for CharStatValue {
    fn default() -> Self {
        Self(StatusParameter::Str)
    }
}

/// Marks a primary stat row's next-point cost text.
#[derive(Component, Clone, Copy)]
pub struct CharCostLabel(pub StatusParameter);

impl Default for CharCostLabel {
    fn default() -> Self {
        Self(StatusParameter::Str)
    }
}

/// Marks the "Status Point" bank value text.
#[derive(Component, Default, Clone)]
pub struct CharPointBank;

/// Marks a stepper button so its observer and dim-state are keyed to a stat.
#[derive(Component, Clone, Copy)]
pub struct CharStepper {
    stat: StatusParameter,
    raise: bool,
}

impl Default for CharStepper {
    fn default() -> Self {
        Self {
            stat: StatusParameter::Str,
            raise: false,
        }
    }
}

/// Marks a combat-readout cell value text.
#[derive(Component, Clone, Copy, Default)]
pub enum CharCombatCell {
    #[default]
    Atk,
    Matk,
    Def,
    Mdef,
    Hit,
    Flee,
    Crit,
    Aspd,
}

/// Marks the Save / Reset commit buttons so their dim-state tracks staging.
#[derive(Component, Default, Clone)]
pub struct CharCommitButton;

// ---------------------------------------------------------------------------
// Scene: attributes rail + combat readout + commit row.
// ---------------------------------------------------------------------------

/// Section header: a faint uppercase caption.
fn head(text: &'static str) -> impl Scene {
    bsn! {
        Text({text.to_string()})
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        TextColor(theme::GOLD)
        Node { margin: {UiRect::bottom(px(4))} }
        ignore_picking()
    }
}

/// The whole attributes block: the two-pane ledger (rail + combat readout) over the
/// Save / Reset commit row.
pub fn attributes_panel() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12), flex_grow: 1.0, flex_basis: px(0) }
        ignore_picking()
        Children [ ledger(), commit_row() ]
    }
}

fn ledger() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(16) }
        ignore_picking()
        Children [ rail(), combat() ]
    }
}

/// Left rail: STR–LUK rows with steppers + cost, then the status-point bank.
fn rail() -> impl Scene {
    let rows: Vec<_> = PRIMARY_STATS.iter().map(|&stat| stat_row(stat)).collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
        }
        ignore_picking()
        Children [ head("ATTRIBUTES"), {rows}, bank() ]
    }
}

fn bank() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: {UiRect::top(px(4))},
            padding: {UiRect::axes(px(8), px(6))},
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            chrome_text("Status Point".to_string(), 11.0, theme::TEXT_DIM),
            (
                Text({"0".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                TextColor(theme::GOLD)
                CharPointBank
                ignore_picking()
            ),
        ]
    }
}

/// A single STR–LUK row: key, value, `−`/`+` steppers, next-point cost.
fn stat_row(stat: StatusParameter) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
        }
        ignore_picking()
        Children [
            (
                chrome_text(stat.name().to_string(), 11.5, theme::TEXT_DIM)
                Node { width: px(34) }
            ),
            (
                template_value(CharStatValue(stat))
                Text({"0".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                TextColor(theme::TEXT)
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            stepper(stat, false),
            stepper(stat, true),
            (
                template_value(CharCostLabel(stat))
                Text({"".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.5)},
                }
                TextColor(theme::TEXT_FAINT)
                Node { width: px(34) }
                ignore_picking()
            ),
        ]
    }
}

fn stepper(stat: StatusParameter, raise: bool) -> impl Scene {
    let glyph = if raise { "+" } else { "\u{2212}" };
    bsn! {
        template_value(CharStepper { stat, raise })
        Node {
            width: px(20),
            height: px(20),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(theme::FIELD)
        Pickable
        on(on_char_stepper)
        Children [ chrome_text(glyph.to_string(), 13.0, theme::TEXT) ]
    }
}

/// Right pane: read-only combat readout, straight from `CharacterStatus`.
fn combat() -> impl Scene {
    let cells: Vec<_> = [
        ("Atk", CharCombatCell::Atk),
        ("Matk", CharCombatCell::Matk),
        ("Def", CharCombatCell::Def),
        ("Mdef", CharCombatCell::Mdef),
        ("Hit", CharCombatCell::Hit),
        ("Flee", CharCombatCell::Flee),
        ("Crit", CharCombatCell::Crit),
        ("Aspd", CharCombatCell::Aspd),
    ]
    .into_iter()
    .map(|(label, cell)| combat_cell(label, cell))
    .collect();
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
        }
        ignore_picking()
        Children [ head("COMBAT"), {cells} ]
    }
}

fn combat_cell(label: &'static str, cell: CharCombatCell) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            chrome_text(label.to_string(), 11.5, theme::TEXT_DIM),
            (
                template_value(cell)
                Text({"0".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor(theme::TEXT)
                ignore_picking()
            ),
        ]
    }
}

fn commit_row() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
        ignore_picking()
        Children [
            (
                commit_button(theme::FIELD)
                on(on_char_reset)
                Children [ chrome_text("Reset".to_string(), 13.0, theme::TEXT_DIM) ]
            ),
            (
                commit_button(theme::EMERALD)
                on(on_char_save)
                Children [ chrome_text("Save".to_string(), 13.0, theme::EMERALD_INK) ]
            ),
        ]
    }
}

/// The shared commit-button chrome (a filled, centered pill). Reset/Save each attach
/// their own `on(...)` observer and caption at the call site.
fn commit_button(bg: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            height: px(32),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(bg)
        CharCommitButton
        Pickable
    }
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// `+`/`−` observer: mutates `CharStatStaging` via the staging helpers. Reads every
/// primary base from `CharacterStatus` so no stat is ever missing from the bases.
fn on_char_stepper(
    click: On<Pointer<Click>>,
    steppers: Query<&CharStepper>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<CharStatStaging>,
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

/// Save: emit one `StatIncreaseRequested` per modified stat, then clear staging.
/// No-op when nothing is staged (`save_messages` returns empty).
fn on_char_save(
    _: On<Pointer<Click>>,
    mut staging: ResMut<CharStatStaging>,
    mut writer: MessageWriter<StatIncreaseRequested>,
) {
    for message in save_messages(&staging) {
        writer.write(message);
    }
    staging.clear();
}

/// Reset: discard all staged points without contacting the server.
fn on_char_reset(_: On<Pointer<Click>>, mut staging: ResMut<CharStatStaging>) {
    staging.clear();
}

// ---------------------------------------------------------------------------
// Refresh system.
// ---------------------------------------------------------------------------

/// Gates [`update_console_attributes`]: run only when the local player's stats or the
/// staging draft change, or when the value elements are freshly spawned (so the first
/// build populates them). Skips the per-frame formatting otherwise.
pub fn console_attributes_changed(
    player: Query<(), (With<LocalPlayer>, Changed<CharacterStatus>)>,
    staging: Res<CharStatStaging>,
    added: Query<(), Added<CharStatValue>>,
) -> bool {
    !player.is_empty() || staging.is_changed() || !added.is_empty()
}

/// Point-bank text, kept disjoint from the stat-value and cost-label text queries.
type PointBankText<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CharPointBank>,
        Without<CharStatValue>,
        Without<CharCostLabel>,
    ),
>;

/// Combat-cell text, kept disjoint from every other mutable `Text` query above.
type CombatCellText<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static CharCombatCell),
    (
        Without<CharStatValue>,
        Without<CharCostLabel>,
        Without<CharPointBank>,
    ),
>;

/// Reflects `CharacterStatus` + `CharStatStaging` into the marked elements, writing
/// only on change. Combat is server-truth (no staged preview).
#[allow(clippy::too_many_arguments)]
pub fn update_console_attributes(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    staging: Res<CharStatStaging>,
    mut values: Query<(&mut Text, &CharStatValue)>,
    mut costs: Query<(&mut Text, &CharCostLabel), Without<CharStatValue>>,
    mut bank: PointBankText,
    mut combat: CombatCellText,
    mut steppers: Query<(&mut BackgroundColor, &CharStepper)>,
    mut commit: Query<&mut BackgroundColor, (With<CharCommitButton>, Without<CharStepper>)>,
) {
    let Ok(status) = player.single() else {
        return;
    };
    let bases = primary_bases(status);
    let points_left = staging.points_left(status.status_point, &bases);

    for (mut text, CharStatValue(stat)) in &mut values {
        let value = (status.get_param(*stat) + staging.staged_value(*stat)).to_string();
        set_text(&mut text, value);
    }

    for (mut text, CharCostLabel(stat)) in &mut costs {
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
            CharCombatCell::Atk => status.atk1 + status.atk2,
            CharCombatCell::Matk => status.matk1 + status.matk2,
            CharCombatCell::Def => status.def1 + status.def2,
            CharCombatCell::Mdef => status.mdef1 + status.mdef2,
            CharCombatCell::Hit => status.hit,
            CharCombatCell::Flee => status.flee1 + status.flee2,
            CharCombatCell::Crit => status.critical,
            CharCombatCell::Aspd => status.aspd,
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
        let mut staging = CharStatStaging::default();
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
        let mut staging = CharStatStaging::default();
        staging.raise(StatusParameter::Str, 5, &bases);
        assert_eq!(staging.staged_value(StatusParameter::Str), 0);
    }

    #[test]
    fn lower_clamped_at_base() {
        let mut staging = CharStatStaging::default();
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
        let mut staging = CharStatStaging::default();
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
        assert!(save_messages(&CharStatStaging::default()).is_empty());
    }

    fn text_of(app: &App, e: Entity) -> String {
        app.world().get::<Text>(e).unwrap().0.clone()
    }

    #[test]
    fn update_reflects_base_plus_staged_without_touching_combat() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<CharStatStaging>();

        let str_value = app
            .world_mut()
            .spawn((Text::new(""), CharStatValue(StatusParameter::Str)))
            .id();
        let atk = app
            .world_mut()
            .spawn((Text::new(""), CharCombatCell::Atk))
            .id();
        let bank = app.world_mut().spawn((Text::new(""), CharPointBank)).id();

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

        app.world_mut().resource_mut::<CharStatStaging>().raise(
            StatusParameter::Str,
            100,
            &HashMap::from([(StatusParameter::Str, 10)]),
        );

        app.add_systems(Update, update_console_attributes);
        app.update();

        assert_eq!(text_of(&app, str_value), "11");
        assert_eq!(text_of(&app, atk), "42");
        // cost(10) = 2, so one staged point spent from 100.
        assert_eq!(text_of(&app, bank), "98");
    }
}
