//! The Character window's stat allocation panel and its combat read-outs.
//!
//! The panel switches between the six attributes and the six traits
//! ([`StatMode`]). Points are staged in [`CharStatStaging`] and submitted together by
//! Apply. The client estimates point costs for drafts, then locks editing while
//! submitted stats await acknowledgment. Rejections are shown inline; stat and
//! point-balance updates remain server-authoritative through `ParamChanged`.
//! Combat formulas are not replicated: the read-outs show `CharacterStatus` as sent.
//!
//! NOTE: the design also shows each stat's equipment bonus and a preview of the
//! combat values a draft would give. The protocol carries neither a per-stat bonus nor
//! the formulas, so add them once the server sends that data.

use std::collections::{HashMap, HashSet};

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::status::{
    CharacterStatus, StatusParameter,
};
use lifthrasir_client::domain::entities::character::events::StatIncreaseRequested;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{ButtonArt, HoverColors, inset};
use net_contract::events::StatRaised;
use net_contract::state::GameMode;

use crate::screens::shared::{DISABLED_TINT, flush_button, overline, small_button, thousands};

pub const PRIMARY_STATS: [StatusParameter; 6] = [
    StatusParameter::Str,
    StatusParameter::Agi,
    StatusParameter::Vit,
    StatusParameter::Int,
    StatusParameter::Dex,
    StatusParameter::Luk,
];

pub const TRAIT_STATS: [StatusParameter; 6] = [
    StatusParameter::Pow,
    StatusParameter::Sta,
    StatusParameter::Wis,
    StatusParameter::Spl,
    StatusParameter::Con,
    StatusParameter::Crt,
];

/// Width of a stat row's `+` button, kept under the value's text height.
const STEPPER: f32 = 18.0;
const STAT_CAP: u32 = 99;
const TRAIT_CAP: u32 = 100;

/// Register attribute state and its live UI projection.
pub fn register(app: &mut App) {
    app.init_resource::<CharStatStaging>();
    app.add_systems(
        Update,
        (consume_stat_results, update_allocation_feedback)
            .chain()
            .before(update_console_attributes),
    );
    app.add_systems(
        Update,
        (
            reflect_stat_mode,
            update_console_attributes.run_if(console_attributes_changed),
        )
            .run_if(in_state(GameState::InGame)),
    );
    app.add_systems(OnExit(GameState::InGame), reset_allocations);
}

/// Drafts and outstanding replies belong to the current gameplay session only.
fn reset_allocations(mut staging: ResMut<CharStatStaging>) {
    *staging = default();
}

// ---------------------------------------------------------------------------
// Stat allocation state and draft cost estimates.
// ---------------------------------------------------------------------------

/// Which six stats the allocation panel lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StatMode {
    #[default]
    Attributes,
    Traits,
}

impl StatMode {
    const ALL: [Self; 2] = [Self::Attributes, Self::Traits];

    fn label(self) -> &'static str {
        match self {
            Self::Attributes => "Attributes",
            Self::Traits => "Traits",
        }
    }

    fn points_caption(self) -> &'static str {
        match self {
            Self::Attributes => "Status Points",
            Self::Traits => "Trait Points",
        }
    }

    fn stats(self) -> &'static [StatusParameter; 6] {
        match self {
            Self::Attributes => &PRIMARY_STATS,
            Self::Traits => &TRAIT_STATS,
        }
    }
}

/// The shown [`StatMode`], draft points and outstanding acknowledgments. Submitted
/// points are never added to server stats locally; editing stays locked until every
/// stat has replied.
#[derive(Resource, Default)]
pub struct CharStatStaging {
    pub mode: StatMode,
    staged: HashMap<StatusParameter, u32>,
    pending: HashSet<u32>,
    rejection: String,
}

/// Renewal stat-point cost to raise a stat from `value` to `value + 1`.
pub fn stat_point_cost(value: u32) -> u32 {
    if value < 100 {
        2 + (value - 1) / 10
    } else {
        16 + 4 * ((value - 100) / 5)
    }
}

fn is_trait(stat: StatusParameter) -> bool {
    TRAIT_STATS.contains(&stat)
}

impl CharStatStaging {
    pub fn staged_value(&self, stat: StatusParameter) -> u32 {
        self.staged.get(&stat).copied().unwrap_or(0)
    }

    /// Status points spent across the staged attributes, given each one's server base.
    pub fn spent(&self, base: &HashMap<StatusParameter, u32>) -> u32 {
        self.staged
            .iter()
            .filter(|(stat, _)| !is_trait(**stat))
            .flat_map(|(stat, &staged)| {
                let start = base.get(stat).copied().unwrap_or(0);
                (0..staged).map(move |i| stat_point_cost(start + i))
            })
            .sum()
    }

    pub fn points_left(&self, status_point: u32, base: &HashMap<StatusParameter, u32>) -> u32 {
        status_point.saturating_sub(self.spent(base))
    }

    /// Trait points spent across the staged traits. A trait point is a flat 1 per level.
    pub fn trait_spent(&self) -> u32 {
        TRAIT_STATS
            .iter()
            .map(|&stat| self.staged_value(stat))
            .sum()
    }

    pub fn trait_points_left(&self, trait_point: u32) -> u32 {
        trait_point.saturating_sub(self.trait_spent())
    }

    pub fn raise(
        &mut self,
        stat: StatusParameter,
        status_point: u32,
        bases: &HashMap<StatusParameter, u32>,
    ) {
        if !self.pending.is_empty() {
            return;
        }
        let staged = self.staged_value(stat);
        let base = bases.get(&stat).copied().unwrap_or(0);
        if can_raise(base, staged, self.points_left(status_point, bases)) {
            self.staged.insert(stat, staged + 1);
        }
    }

    pub fn raise_trait(&mut self, stat: StatusParameter, base: u32, trait_point: u32) {
        if !self.pending.is_empty() {
            return;
        }
        let staged = self.staged_value(stat);
        if can_raise_trait(base, staged, self.trait_points_left(trait_point)) {
            self.staged.insert(stat, staged + 1);
        }
    }

    pub fn clear(&mut self) {
        self.staged.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.staged.values().all(|&v| v == 0)
    }

    /// Whether any stat of `mode` has staged points.
    fn has_staged(&self, mode: StatMode) -> bool {
        mode.stats().iter().any(|&stat| self.staged_value(stat) > 0)
    }
}

/// Whether one more attribute point can be staged: below cap and affordable.
pub fn can_raise(base: u32, staged: u32, points_left: u32) -> bool {
    let current = base + staged;
    current < STAT_CAP && stat_point_cost(current) <= points_left
}

/// Whether one more trait point can be staged: below cap with a point left.
pub fn can_raise_trait(base: u32, staged: u32, points_left: u32) -> bool {
    base + staged < TRAIT_CAP && points_left > 0
}

/// Maps the current staging to one `StatIncreaseRequested` per modified stat
/// (`status_id` = the `StatusParameter` repr, `amount` = staged count).
fn save_messages(staging: &CharStatStaging) -> Vec<StatIncreaseRequested> {
    PRIMARY_STATS
        .iter()
        .chain(TRAIT_STATS.iter())
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

/// What the next point of `stat` costs, as its row shows it.
fn cost_text(stat: StatusParameter, status: &CharacterStatus, staged: u32) -> String {
    let current = status.get_param(stat) + staged;
    if is_trait(stat) {
        let text = if current >= TRAIT_CAP { "Max" } else { "1 pt" };
        return text.to_string();
    }
    if current >= STAT_CAP {
        return "Max".to_string();
    }
    let cost = if staged == 0 {
        status.get_param(cost_param(stat))
    } else {
        stat_point_cost(current)
    };
    format!("{cost} pts")
}

// ---------------------------------------------------------------------------
// UI markers.
// ---------------------------------------------------------------------------

/// A read-out of the combat panels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombatStat {
    Atk,
    Matk,
    Hit,
    Flee,
    Def,
    Mdef,
    Crit,
    Aspd,
    PerfectDodge,
    MaxWeight,
    PAtk,
    SMatk,
    Res,
    MRes,
    HPlus,
    CRate,
}

const COMBAT: [(&str, CombatStat); 10] = [
    ("Atk", CombatStat::Atk),
    ("Matk", CombatStat::Matk),
    ("Hit", CombatStat::Hit),
    ("Flee", CombatStat::Flee),
    ("Def", CombatStat::Def),
    ("Mdef", CombatStat::Mdef),
    ("Critical", CombatStat::Crit),
    ("Aspd", CombatStat::Aspd),
    ("Perfect Dodge", CombatStat::PerfectDodge),
    ("Max Weight", CombatStat::MaxWeight),
];

const TRAIT_COMBAT: [(&str, CombatStat); 6] = [
    ("P.Atk", CombatStat::PAtk),
    ("S.Matk", CombatStat::SMatk),
    ("Res", CombatStat::Res),
    ("Mres", CombatStat::MRes),
    ("H.Plus", CombatStat::HPlus),
    ("C.Rate", CombatStat::CRate),
];

/// The value of a combat read-out, straight from the server status. Two-part stats
/// show their left and right status-window values. MATK's left value is `matk2`: in
/// renewal the pair is `status + equipment`, in pre-renewal it is the `min ~ max` band.
fn combat_value(stat: CombatStat, status: &CharacterStatus, mode: GameMode) -> String {
    let pair = |a: u32, b: u32| format!("{a} + {b}");
    match stat {
        CombatStat::Atk => pair(status.atk1, status.atk2),
        CombatStat::Matk if mode == GameMode::PreRenewal => {
            format!("{} ~ {}", status.matk2, status.matk1)
        }
        CombatStat::Matk => pair(status.matk2, status.matk1),
        CombatStat::Def => pair(status.def1, status.def2),
        CombatStat::Mdef => pair(status.mdef1, status.mdef2),
        CombatStat::Hit => status.hit.to_string(),
        CombatStat::Flee => status.flee1.to_string(),
        CombatStat::PerfectDodge => status.flee2.to_string(),
        CombatStat::Crit => status.critical.to_string(),
        CombatStat::Aspd => status.aspd.to_string(),
        CombatStat::MaxWeight => thousands(status.max_weight.into()),
        CombatStat::PAtk => status.patk.to_string(),
        CombatStat::SMatk => status.smatk.to_string(),
        CombatStat::Res => status.res.to_string(),
        CombatStat::MRes => status.mres.to_string(),
        CombatStat::HPlus => status.hplus.to_string(),
        CombatStat::CRate => status.crit_damage_rate.to_string(),
    }
}

/// A text [`update_console_attributes`] keeps current.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharLabel {
    /// A stat's value, base plus staged.
    Value(StatusParameter),
    /// The points staged on a stat, empty when none.
    Staged(StatusParameter),
    /// What the stat's next point costs.
    Cost(StatusParameter),
    /// The unspent points of the shown mode.
    Points,
    /// The name of those points.
    PointsCaption,
    Combat(CombatStat),
}

/// A stat's `+` button.
#[derive(Component, Clone, Copy)]
pub struct CharStepper(pub StatusParameter);

/// Inline pending/rejection feedback below the stat allocation controls.
#[derive(Component, Default, Clone)]
struct CharAllocationFeedback;

/// Marks the Apply / Reset buttons so their disabled state tracks staging.
#[derive(Component, Default, Clone)]
pub struct CharCommitButton;

/// Mount of the Attributes / Traits switch, rebuilt by [`reflect_stat_mode`].
#[derive(Component, Default, Clone)]
struct StatModeMount;

#[derive(Component, Default, Clone, Copy)]
struct StatModeButton(StatMode);

/// The rows of one [`StatMode`]. Only the shown mode's page is in the layout.
#[derive(Component, Default, Clone, Copy)]
struct StatPage(StatMode);

// ---------------------------------------------------------------------------
// Scene: allocation panel, combat read-outs.
// ---------------------------------------------------------------------------

/// The right column: the allocation panel over the combat read-out.
pub fn stats_column() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        @ignore_picking()
        Children [ @allocation_panel() -- @combat_panel() ]
    }
}

fn allocation_panel() -> impl Scene {
    let points = CharLabel::Points;
    let caption = CharLabel::PointsCaption;
    bsn! {
        @inset()
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect::new(px(14), px(12), px(12), px(12))},
        }
        @ignore_picking()
        Children [
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            @ignore_picking()
            Children [
                StatModeMount
                Node { column_gap: px(4) }
                @ignore_picking()
                --
                @chrome_text("Cost to raise".into(), 11.0, ro_dark::MUTED)
            ]
            --
            @stat_page(StatMode::Attributes)
            --
            @stat_page(StatMode::Traits)
            --
            Node {
                margin: {UiRect::top(px(10))},
                align_items: AlignItems::Baseline,
                justify_content: JustifyContent::SpaceBetween,
            }
            @ignore_picking()
            Children [
                caption
                @chrome_text(String::new(), 12.0, ro_dark::MUTED)
                --
                points
                @chrome_text("0".into(), 16.0, ro_dark::BRONZE_LIGHT)
            ]
            --
            @commit_row()
            --
            CharAllocationFeedback
            @chrome_text(String::new(), 11.0, ro_dark::BRONZE_LIGHT)
        ]
    }
}

fn stat_page(mode: StatMode) -> impl Scene {
    let rows: Vec<_> = mode.stats().iter().map(|&stat| stat_row(stat)).collect();
    let display = if mode == StatMode::default() {
        Display::Flex
    } else {
        Display::None
    };
    bsn! {
        StatPage(mode)
        Node {
            display: {display},
            flex_direction: FlexDirection::Column,
            margin: {UiRect::top(px(6))},
        }
        @ignore_picking()
        Children [ {rows} ]
    }
}

/// One stat: its key, value, staged points, next-point cost and `+` button.
fn stat_row(stat: StatusParameter) -> impl Scene {
    let (value, staged, cost) = (
        CharLabel::Value(stat),
        CharLabel::Staged(stat),
        CharLabel::Cost(stat),
    );
    let stepper = CharStepper(stat);
    bsn! {
        Node {
            height: px(30),
            align_items: AlignItems::Center,
            column_gap: px(8),
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @chrome_text(stat.name().into(), 12.0, ro_dark::ACCENT)
            Node { width: px(30), flex_shrink: 0.0 }
            --
            value
            @chrome_text("0".into(), 18.0, ro_dark::STRONG)
            Node { width: px(34), flex_shrink: 0.0 }
            TextLayout { justify: Justify::Right }
            --
            staged
            @chrome_text(String::new(), 10.0, ro_dark::CHECK_MARK)
            Node { border_radius: BorderRadius::all(px(3)) }
            BackgroundColor(ro_dark::PRIMARY)
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            cost
            @chrome_text(String::new(), 11.0, ro_dark::MUTED)
            --
            stepper
            @small_button("plus", STEPPER)
            on(on_char_stepper)
        ]
    }
}

fn commit_row() -> impl Scene {
    bsn! {
        Node { column_gap: px(8), margin: {UiRect::top(px(10))} }
        @ignore_picking()
        Children [
            CharCommitButton
            @flush_button(ButtonArt::NORMAL, "Reset")
            on(on_char_reset)
            --
            CharCommitButton
            @flush_button(ButtonArt::PRIMARY, "Apply")
            on(on_char_save)
        ]
    }
}

/// One side of the Attributes / Traits switch. `dot` flags staged points on the side
/// that is not shown.
fn mode_button(mode: StatMode, active: bool, dot: bool) -> impl Scene {
    let (fill, stroke, label) = if active {
        (
            ro_dark::TAB_ACTIVE,
            ro_dark::TAB_STROKE,
            ro_dark::BUTTON_LABEL,
        )
    } else {
        (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE, ro_dark::MUTED)
    };
    let hover = if active {
        (fill, stroke)
    } else {
        (ro_dark::TAB_HOVER, ro_dark::MUTED_BRONZE)
    };
    let colors = HoverColors {
        rest: (fill, stroke),
        hover,
    };
    let dot = dot.then(|| {
        EntityScene(bsn! {
            Node { width: px(5), height: px(5), border_radius: BorderRadius::MAX }
            BackgroundColor(ro_dark::FROST_GLOW)
            @ignore_picking()
        })
    });
    bsn! {
        StatModeButton(mode)
        Hovered
        colors
        Node {
            height: px(24),
            padding: {UiRect::horizontal(px(10))},
            align_items: AlignItems::Center,
            column_gap: px(6),
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(pick_mode)
        Children [
            @chrome_text(mode.label().into(), 12.0, label)
            --
            {dot}
        ]
    }
}

/// The read-only combat read-out, two values per row.
fn combat_panel() -> impl Scene {
    let rows: Vec<_> = COMBAT
        .chunks(2)
        .map(|pair| {
            read_out_row(
                pair.iter()
                    .map(|&(label, stat)| read_out(label, stat))
                    .collect(),
            )
        })
        .collect();
    bsn! {
        @inset()
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: {UiRect::axes(px(14), px(12))},
        }
        @ignore_picking()
        Children [
            @overline("COMBAT".into(), ro_dark::ACCENT)
            Node { margin: {UiRect::bottom(px(2))} }
            --
            {rows}
        ]
    }
}

/// The trait read-out under the paperdoll, all six values in one row.
pub fn trait_combat_panel() -> impl Scene {
    let cells: Vec<_> = TRAIT_COMBAT
        .iter()
        .map(|&(label, stat)| read_out(label, stat))
        .collect();
    bsn! {
        @inset()
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            margin: {UiRect::top(px(12))},
            padding: {UiRect::new(px(14), px(14), px(10), px(12))},
        }
        @ignore_picking()
        Children [
            @overline("TRAIT COMBAT".into(), ro_dark::ACCENT)
            --
            @read_out_row(cells)
        ]
    }
}

fn read_out_row<S: Scene>(cells: Vec<S>) -> impl Scene {
    bsn! {
        Node { column_gap: px(14) }
        @ignore_picking()
        Children [ {cells} ]
    }
}

/// A caption over its live value.
fn read_out(label: &'static str, stat: CombatStat) -> impl Scene {
    let value = CharLabel::Combat(stat);
    bsn! {
        Node {
            height: px(30),
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
            --
            value
            @chrome_text("0".into(), 13.0, ro_dark::STRONG)
        ]
    }
}

// ---------------------------------------------------------------------------
// Observers.
// ---------------------------------------------------------------------------

/// `+` observer: stages one point on the button's stat. Reads every primary base from
/// `CharacterStatus` so no stat is ever missing from the bases.
fn on_char_stepper(
    activate: On<Activate>,
    steppers: Query<&CharStepper>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut staging: ResMut<CharStatStaging>,
) {
    let Ok(&CharStepper(stat)) = steppers.get(activate.entity) else {
        return;
    };
    let Ok(status) = player.single() else {
        return;
    };
    if is_trait(stat) {
        staging.raise_trait(stat, status.get_param(stat), status.trait_point);
        return;
    }
    staging.raise(stat, status.status_point, &primary_bases(status));
}

/// Submit the draft once, retaining each stat ID until its acknowledgment arrives.
fn on_char_save(
    _: On<Activate>,
    mut staging: ResMut<CharStatStaging>,
    mut writer: MessageWriter<StatIncreaseRequested>,
) {
    if !staging.pending.is_empty() || staging.is_empty() {
        return;
    }
    staging.rejection.clear();
    for message in save_messages(&staging) {
        staging.pending.insert(u32::from(message.status_id));
        writer.write(message);
    }
    staging.clear();
}

/// Reset: discard all staged points without contacting the server.
fn on_char_reset(_: On<Activate>, mut staging: ResMut<CharStatStaging>) {
    staging.clear();
}

fn pick_mode(
    click: On<PointerClick>,
    buttons: Query<&StatModeButton>,
    mut staging: ResMut<CharStatStaging>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    if staging.mode != button.0 {
        staging.mode = button.0;
    }
}

/// Consume acknowledgments even when the window is hidden. `value` is capped by
/// Aesir and may represent a partial allocation, so only `ParamChanged` updates
/// character stats and point balances.
fn consume_stat_results(
    mut results: MessageReader<StatRaised>,
    mut staging: ResMut<CharStatStaging>,
) {
    for result in results.read() {
        if !staging.pending.contains(&result.stat_id) {
            continue;
        }
        staging.pending.remove(&result.stat_id);
        if !result.ok
            && let Some(stat) = PRIMARY_STATS
                .iter()
                .chain(TRAIT_STATS.iter())
                .find(|&&stat| stat as u32 == result.stat_id)
        {
            staging
                .rejection
                .push_str(&format!("{} allocation rejected by server.\n", stat.name()));
        }
    }
}

/// Project pending/rejected results independently of whether the window is open.
fn update_allocation_feedback(
    staging: Res<CharStatStaging>,
    mut labels: Query<&mut Text, With<CharAllocationFeedback>>,
) {
    for mut text in &mut labels {
        if !staging.is_changed() && !text.is_added() {
            continue;
        }
        let waiting = if staging.pending.is_empty() {
            ""
        } else {
            "Waiting for stat allocation...\n"
        };
        set_text(
            &mut text,
            format!("{waiting}{}", staging.rejection)
                .trim_end()
                .to_string(),
        );
    }
}

// ---------------------------------------------------------------------------
// Refresh systems.
// ---------------------------------------------------------------------------

/// Respawns the Attributes / Traits switch and lays out only the shown mode's rows,
/// whenever the staging changes or the mount first appears.
fn reflect_stat_mode(
    mut commands: Commands,
    staging: Res<CharStatStaging>,
    mounts: Query<(Entity, Option<&Children>, Ref<StatModeMount>)>,
    mut pages: Query<(&mut Node, &StatPage)>,
) {
    let Ok((mount, children, mount_ref)) = mounts.single() else {
        return;
    };
    if !staging.is_changed() && !mount_ref.is_added() {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    for mode in StatMode::ALL {
        let active = mode == staging.mode;
        commands
            .spawn_scene(mode_button(
                mode,
                active,
                !active && staging.has_staged(mode),
            ))
            .insert(ChildOf(mount));
    }
    for (mut node, page) in &mut pages {
        let want = if page.0 == staging.mode {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Gates [`update_console_attributes`]: run only when the local player's stats or the
/// staging draft change, or when the labels are freshly spawned (so the first build
/// populates them). Skips the per-frame formatting otherwise.
pub fn console_attributes_changed(
    player: Query<(), (With<LocalPlayer>, Changed<CharacterStatus>)>,
    staging: Res<CharStatStaging>,
    added: Query<(), Added<CharLabel>>,
) -> bool {
    !player.is_empty() || staging.is_changed() || !added.is_empty()
}

/// The Apply / Reset art buttons, kept disjoint from the `+` buttons.
type CommitButtons<'w, 's> = Query<
    'w,
    's,
    (&'static mut ImageNode, &'static mut Pickable),
    (With<CharCommitButton>, Without<CharStepper>),
>;

/// Reflects `CharacterStatus` + `CharStatStaging` into the marked texts and buttons,
/// writing only on change. Combat is server-truth (no staged preview).
pub fn update_console_attributes(
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    staging: Res<CharStatStaging>,
    game_mode: Option<Res<GameMode>>,
    mut labels: Query<(&mut Text, &mut TextColor, &CharLabel)>,
    mut steppers: Query<(&mut ImageNode, &mut Pickable, &CharStepper)>,
    mut commit: CommitButtons,
) {
    let Ok(status) = player.single() else {
        return;
    };
    let bases = primary_bases(status);
    let points_left = staging.points_left(status.status_point, &bases);
    let trait_points_left = staging.trait_points_left(status.trait_point);
    let game_mode = game_mode.map(|mode| *mode).unwrap_or_default();

    for (mut text, mut color, label) in &mut labels {
        let value = match *label {
            CharLabel::Value(stat) => {
                let staged = staging.staged_value(stat);
                let tint = if staged > 0 {
                    ro_dark::FROST_GLOW
                } else {
                    ro_dark::STRONG
                };
                if color.0 != tint {
                    color.0 = tint;
                }
                (status.get_param(stat) + staged).to_string()
            }
            CharLabel::Staged(stat) => match staging.staged_value(stat) {
                0 => String::new(),
                staged => format!("+{staged}"),
            },
            CharLabel::Cost(stat) => cost_text(stat, status, staging.staged_value(stat)),
            CharLabel::Points => match staging.mode {
                StatMode::Attributes => points_left.to_string(),
                StatMode::Traits => trait_points_left.to_string(),
            },
            CharLabel::PointsCaption => staging.mode.points_caption().to_string(),
            CharLabel::Combat(stat) => combat_value(stat, status, game_mode),
        };
        set_text(&mut text, value);
    }

    for (mut image, mut pickable, &CharStepper(stat)) in &mut steppers {
        let base = status.get_param(stat);
        let staged = staging.staged_value(stat);
        let affordable = if is_trait(stat) {
            can_raise_trait(base, staged, trait_points_left)
        } else {
            can_raise(base, staged, points_left)
        };
        set_usable(
            &mut image,
            &mut pickable,
            staging.pending.is_empty() && affordable,
        );
    }

    let staged = !staging.is_empty();
    for (mut image, mut pickable) in &mut commit {
        set_usable(&mut image, &mut pickable, staged);
    }
}

/// Greys out an art button that cannot be used and stops it hovering or activating.
fn set_usable(image: &mut ImageNode, pickable: &mut Pickable, enabled: bool) {
    let tint = if enabled { Color::WHITE } else { DISABLED_TINT };
    if image.color != tint {
        image.color = tint;
    }
    let want = Pickable {
        should_block_lower: enabled,
        is_hoverable: enabled,
    };
    if *pickable != want {
        *pickable = want;
    }
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        *text = Text::new(value);
    }
}

#[cfg(test)]
mod allocation_tests;

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
    fn traits_spend_their_own_points_one_per_level() {
        let bases = HashMap::from([(StatusParameter::Str, 10)]);
        let mut staging = CharStatStaging::default();
        staging.raise(StatusParameter::Str, 100, &bases);
        staging.raise_trait(StatusParameter::Pow, 0, 2);
        staging.raise_trait(StatusParameter::Pow, 0, 2);
        staging.raise_trait(StatusParameter::Con, 0, 2);

        assert_eq!(staging.staged_value(StatusParameter::Pow), 2);
        assert_eq!(staging.staged_value(StatusParameter::Con), 0);
        assert_eq!(staging.trait_points_left(2), 0);
        // Only the attribute draft spends status points: cost(10) = 2.
        assert_eq!(staging.points_left(100, &bases), 98);
    }

    #[test]
    fn traits_stop_at_their_cap() {
        assert!(can_raise_trait(99, 0, 1));
        assert!(!can_raise_trait(99, 1, 1));
        assert!(!can_raise_trait(10, 0, 0));
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
    fn save_includes_staged_traits() {
        let mut staging = CharStatStaging::default();
        staging.raise_trait(StatusParameter::Sta, 5, 3);

        let messages = save_messages(&staging);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status_id, StatusParameter::Sta as u16);
        assert_eq!(messages[0].amount, 1);
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

        let mut label = |label: CharLabel| app.world_mut().spawn((Text::new(""), label)).id();
        let str_value = label(CharLabel::Value(StatusParameter::Str));
        let str_staged = label(CharLabel::Staged(StatusParameter::Str));
        let str_cost = label(CharLabel::Cost(StatusParameter::Str));
        let atk = label(CharLabel::Combat(CombatStat::Atk));
        let bank = label(CharLabel::Points);

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
        assert_eq!(text_of(&app, str_staged), "+1");
        // cost(11) = 3 for the point after the staged one.
        assert_eq!(text_of(&app, str_cost), "3 pts");
        assert_eq!(text_of(&app, atk), "30 + 12");
        // cost(10) = 2, so one staged point spent from 100.
        assert_eq!(text_of(&app, bank), "98");
    }

    #[test]
    fn matk_reads_status_plus_equipment_in_renewal_and_a_band_in_pre_renewal() {
        let status = CharacterStatus {
            matk1: 155,
            matk2: 90,
            ..default()
        };

        assert_eq!(
            combat_value(CombatStat::Matk, &status, GameMode::Renewal),
            "90 + 155"
        );
        assert_eq!(
            combat_value(CombatStat::Matk, &status, GameMode::PreRenewal),
            "90 ~ 155"
        );
    }

    #[test]
    fn the_point_bank_follows_the_shown_mode() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(CharStatStaging {
            mode: StatMode::Traits,
            ..default()
        });
        let bank = app
            .world_mut()
            .spawn((Text::new(""), CharLabel::Points))
            .id();
        let caption = app
            .world_mut()
            .spawn((Text::new(""), CharLabel::PointsCaption))
            .id();
        app.world_mut().spawn((
            CharacterStatus {
                status_point: 100,
                trait_point: 7,
                ..default()
            },
            LocalPlayer,
        ));

        app.add_systems(Update, update_console_attributes);
        app.update();

        assert_eq!(text_of(&app, bank), "7");
        assert_eq!(text_of(&app, caption), "Trait Points");
    }

    #[test]
    fn only_the_shown_mode_is_laid_out() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.insert_resource(CharStatStaging {
            mode: StatMode::Traits,
            ..default()
        });
        app.world_mut().spawn(StatModeMount);
        let pages = StatMode::ALL.map(|mode| {
            app.world_mut()
                .spawn((Node::default(), StatPage(mode)))
                .id()
        });

        app.add_systems(Update, reflect_stat_mode);
        app.update();

        let shown: Vec<Display> = pages
            .iter()
            .map(|&page| app.world().get::<Node>(page).unwrap().display)
            .collect();
        assert_eq!(shown, vec![Display::None, Display::Flex]);
    }
}
