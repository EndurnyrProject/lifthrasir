//! Plain views of the Homunculus window and its HUD plate: what each region shows,
//! read from the owner's snapshot. Nothing here touches the ECS.
//!
//! The views leave out what changes all the time (HP and SP on the plate, skill
//! cooldowns); those are written in place so a button is never respawned mid-click.

use std::time::Duration;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::ItemDb;
use lifthrasir_assets::catalogs::skill::SkillCatalog;
use lifthrasir_client::domain::homunculus::{
    HomunculusPresence, HomunculusSkillSpec, homunculus_skill_spec,
};
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{
    HomunculusActivity as Activity, HomunculusAiConfig as Config, HomunculusAiSkillMode as Mode,
    HomunculusAiStance as Stance, HomunculusIntimacyGrade as Grade,
    HomunculusLifecycle as Lifecycle, HomunculusSkillMetadata as Skill,
    HomunculusSnapshot as Snapshot,
};

use super::tactics::{self, Dial, Flag};
use super::{Confirmation, HomunculusTab, OwnerUi, selected_rank};
use crate::screens::shared::{clock, thousands};
use crate::widgets::info_modal::description;

/// The pink of everything about intimacy.
pub(super) const INTIMACY: Color = Color::srgb_u8(0xe8, 0xa7, 0xc0);
/// Hunger at or under this is hungry; the plate warns and intimacy is at risk.
const HUNGRY: u32 = 25;
/// Feeding above this hunger lowers intimacy, so it asks first.
pub(super) const FULL: u32 = 75;

/// What the views read besides the snapshot.
pub(super) struct Ctx<'a> {
    pub ui: &'a OwnerUi,
    pub skills: Option<&'a SkillCatalog>,
    pub items: Option<&'a ItemDb>,
    /// How many of the companion's food the owner carries.
    pub food: u32,
    /// Whether the companion is out in the world, where it takes orders.
    pub out: bool,
    /// What is left of the companion's active time.
    pub active_left: Duration,
    /// Why the server turned the taught tactics down.
    pub refusal: Option<&'static str>,
    /// Whether the HUD plate is minified to the portrait.
    pub plate_minimal: bool,
}

impl Ctx<'_> {
    fn skill_name(&self, id: u32) -> String {
        self.skills.and_then(|skills| skills.get(id)).map_or_else(
            || format!("Skill #{id}"),
            |skill| skill.display_name.clone(),
        )
    }

    fn food_name(&self, id: u32) -> String {
        self.items
            .and_then(|items| items.name(id, true))
            .map_or_else(|| format!("Item #{id}"), str::to_owned)
    }
}

/// What the window says while there is no companion to lay out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BodyView {
    Loading,
    Absent,
}

pub(super) fn body(presence: &HomunculusPresence) -> Option<BodyView> {
    match presence {
        HomunculusPresence::Unknown => Some(BodyView::Loading),
        HomunculusPresence::Absent => Some(BodyView::Absent),
        HomunculusPresence::Present(_) => None,
    }
}

/// An order the companion can be following.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Order {
    Follow,
    Attack,
    Stay,
    Move,
}

fn active(snapshot: &Snapshot) -> bool {
    snapshot.lifecycle == Lifecycle::Active
}

fn order(snapshot: &Snapshot) -> Option<Order> {
    if !active(snapshot) {
        return None;
    }
    match snapshot.activity {
        Activity::Following => Some(Order::Follow),
        Activity::Attacking => Some(Order::Attack),
        Activity::Standby => Some(Order::Stay),
        Activity::Moving => Some(Order::Move),
        _ => None,
    }
}

/// What the companion is doing, and the tone to say it in.
fn activity(snapshot: &Snapshot) -> (&'static str, Color) {
    match (snapshot.lifecycle, snapshot.activity) {
        (Lifecycle::Rested, _) => ("Resting in the Embryo", ro_dark::MUTED),
        (Lifecycle::Dead, _) => ("Fallen", ro_dark::TONE_ERROR),
        (Lifecycle::Active, Activity::Following) => ("Following", ro_dark::SELECT_RING),
        (Lifecycle::Active, Activity::Moving) => ("Moving", ro_dark::SELECT_RING),
        (Lifecycle::Active, Activity::Attacking) => ("Attacking", ro_dark::TONE_ERROR),
        (Lifecycle::Active, Activity::Casting) => ("Casting", ro_dark::TONE_OK),
        (Lifecycle::Active, Activity::Standby) => ("Standing by", ro_dark::SELECT_RING),
        (Lifecycle::Active, _) => ("Idle", ro_dark::SELECT_RING),
        _ => ("Unknown", ro_dark::MUTED),
    }
}

/// The companion's kind, from the classic job ids 6001 to 6016: four kinds, each
/// with a second look, then the same eight evolved.
fn species(snapshot: &Snapshot) -> String {
    const KINDS: [&str; 4] = ["Lif", "Amistr", "Filir", "Vanilmirth"];
    let kind = snapshot
        .appearance_id
        .checked_sub(6001)
        .filter(|index| *index < 16)
        .map_or("Homunculus", |index| KINDS[(index % 4) as usize]);
    if snapshot.evolved {
        format!("{kind} \u{b7} Evolved")
    } else {
        kind.into()
    }
}

fn grade(grade: Grade) -> &'static str {
    match grade {
        Grade::HateWithPassion => "Hates you",
        Grade::Hate => "Hate",
        Grade::Awkward => "Awkward",
        Grade::Shy => "Shy",
        Grade::Neutral => "Neutral",
        Grade::Cordial => "Cordial",
        Grade::Loyal => "Loyal",
        _ => "Unknown",
    }
}

fn hunger(hunger: u32) -> (&'static str, Color) {
    match hunger {
        0..=10 => ("Starving", ro_dark::STATUS_FULL),
        11..=HUNGRY => ("Hungry", ro_dark::STATUS_BUSY),
        26..=FULL => ("Fine", ro_dark::STATUS_ONLINE),
        76..=90 => ("Satisfied", ro_dark::STATUS_ONLINE),
        _ => ("Stuffed", ro_dark::SELECT_RING),
    }
}

/// The chips over the portrait.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PortraitView {
    pub activity: &'static str,
    pub tone: Color,
    pub level: u32,
    /// Dims the sprite of a companion that is not out.
    pub away: bool,
}

pub(super) fn portrait(snapshot: &Snapshot) -> PortraitView {
    let (activity, tone) = activity(snapshot);
    PortraitView {
        activity,
        tone,
        level: snapshot.level,
        away: !active(snapshot),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NameView {
    pub name: String,
    pub species: String,
    pub can_rename: bool,
    pub renaming: bool,
}

pub(super) fn name(snapshot: &Snapshot, ctx: &Ctx) -> NameView {
    NameView {
        name: snapshot.name.clone(),
        species: species(snapshot),
        can_rename: snapshot.rename_eligible,
        renaming: ctx.ui.renaming && snapshot.rename_eligible,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CareView {
    pub grade: &'static str,
    pub intimacy: String,
    /// Percent of the intimacy bar.
    pub intimacy_fill: f32,
    pub hunger: u32,
    pub hunger_word: &'static str,
    pub hunger_tone: Color,
    /// Where on the hunger bar the companion feeds itself.
    pub feed_line: Option<f32>,
    pub note: String,
    pub feed: String,
    pub can_feed: bool,
}

pub(super) fn care(snapshot: &Snapshot, ctx: &Ctx) -> CareView {
    let (hunger_word, hunger_tone) = hunger(snapshot.hunger);
    let auto_feed = snapshot
        .ai_config
        .as_ref()
        .filter(|config| config.auto_feed)
        .map(|config| config.auto_feed_threshold);
    let note = match auto_feed {
        _ if snapshot.hunger > FULL => "Not hungry \u{b7} feeding now lowers intimacy".into(),
        Some(at) => format!("Eats by itself at {at}"),
        None => format!("Feed before it drops under {HUNGRY}"),
    };
    CareView {
        grade: grade(snapshot.intimacy_grade),
        intimacy: format!("{:.2}", f64::from(snapshot.intimacy_hundredths) / 100.0),
        intimacy_fill: (snapshot.intimacy_hundredths as f32 / 1000.0).min(100.0),
        hunger: snapshot.hunger,
        hunger_word,
        hunger_tone,
        feed_line: auto_feed.map(|at| at as f32),
        note,
        feed: format!(
            "Feed \u{b7} {} \u{d7}{}",
            ctx.food_name(snapshot.food_item_id),
            ctx.food
        ),
        can_feed: ctx.out && ctx.food > 0,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OrdersView {
    pub name: String,
    pub order: Option<Order>,
    /// Whether the companion is out to take orders.
    pub out: bool,
}

pub(super) fn orders(snapshot: &Snapshot, ctx: &Ctx) -> OrdersView {
    OrdersView {
        name: snapshot.name.clone(),
        order: order(snapshot),
        out: ctx.out,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TabsView {
    pub tab: HomunculusTab,
    /// Skill points waiting for a skill that can take one.
    pub points: Option<u32>,
    /// Whether the tactics draft differs from what the companion knows.
    pub unsaved: bool,
}

pub(super) fn tabs(snapshot: &Snapshot, ctx: &Ctx) -> TabsView {
    let spendable =
        snapshot.skill_points > 0 && snapshot.skills.iter().any(|skill| skill.learnable);
    TabsView {
        tab: ctx.ui.tab,
        points: spendable.then_some(snapshot.skill_points),
        unsaved: ctx.ui.tactics.dirty(),
    }
}

/// The page under the tabs.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum PageView {
    Status(StatusView),
    Skills(SkillsView),
    Tactics(TacticsView),
}

pub(super) fn page(snapshot: &Snapshot, ctx: &Ctx) -> PageView {
    match ctx.ui.tab {
        HomunculusTab::Status => PageView::Status(status(snapshot, ctx)),
        HomunculusTab::Skills => PageView::Skills(skills(snapshot, ctx)),
        HomunculusTab::Tactics => PageView::Tactics(tactics(snapshot, ctx)),
    }
}

/// A labelled amount. `fill` is the percent of its bar, when it has one.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BarView {
    pub label: &'static str,
    pub value: String,
    pub fill: Option<f32>,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct StatusView {
    pub bars: Vec<BarView>,
    pub attributes: Vec<(&'static str, u32)>,
    pub combat: Vec<(&'static str, u32)>,
    pub tip: String,
}

fn percent(amount: u64, limit: u64) -> f32 {
    (amount as f32 / limit.max(1) as f32).min(1.0) * 100.0
}

fn status(snapshot: &Snapshot, ctx: &Ctx) -> StatusView {
    let pool = |label, amount: u64, limit: u64, color| BarView {
        label,
        value: format!("{} / {}", thousands(amount), thousands(limit)),
        fill: Some(percent(amount, limit)),
        color,
    };
    let exp = percent(snapshot.exp, snapshot.next_exp);
    let bars = vec![
        pool("HP", snapshot.hp, snapshot.max_hp, ro_dark::HP),
        pool("SP", snapshot.sp, snapshot.max_sp, ro_dark::SP),
        BarView {
            label: "EXP",
            value: format!("{exp:.1}%"),
            fill: Some(exp),
            color: ro_dark::BASE_EXP,
        },
        BarView {
            label: "ACTIVE TIME LEFT",
            value: clock(ctx.active_left),
            fill: None,
            color: ro_dark::JOB_EXP,
        },
    ];
    let stats = snapshot.stats.as_ref();
    let attributes = stats.map_or_else(Vec::new, |stats| {
        vec![
            ("STR", stats.str),
            ("AGI", stats.agi),
            ("VIT", stats.vit),
            ("INT", stats.int),
            ("DEX", stats.dex),
            ("LUK", stats.luk),
        ]
    });
    let combat = stats.map_or_else(Vec::new, |stats| {
        vec![
            ("Atk", stats.atk),
            ("Matk", stats.matk),
            ("Def", stats.def),
            ("Mdef", stats.mdef),
            ("Hit", stats.hit),
            ("Flee", stats.flee),
            ("Critical", stats.critical),
            ("Aspd", stats.aspd),
        ]
    });
    let name = &snapshot.name;
    let tip = match snapshot.intimacy_grade {
        Grade::Loyal => format!(
            "{name} is Loyal, the highest grade. Keep hunger above 10 and avoid overfeeding to stay there."
        ),
        other => format!(
            "{name} is {}. Feed it when hungry and keep it out with you to reach Loyal.",
            grade(other)
        ),
    };
    StatusView {
        bars,
        attributes,
        combat,
        tip,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SkillRow {
    pub id: u32,
    pub name: String,
    pub level: String,
    pub passive: bool,
    /// Whether the companion casts it by itself.
    pub auto: bool,
    pub about: String,
    /// Why an unlearned skill can or cannot be learned.
    pub lock: Option<(String, Color)>,
    pub cost: Option<String>,
    pub icon: Option<String>,
    /// The level it is cast at, shown when there is more than one to pick from.
    pub rank: Option<u32>,
    pub can_learn: bool,
    pub can_cast: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SkillsView {
    pub points: u32,
    pub rows: Vec<SkillRow>,
}

fn skills(snapshot: &Snapshot, ctx: &Ctx) -> SkillsView {
    SkillsView {
        points: snapshot.skill_points,
        rows: snapshot
            .skills
            .iter()
            .map(|skill| skill_row(skill, snapshot, ctx))
            .collect(),
    }
}

fn skill_row(skill: &Skill, snapshot: &Snapshot, ctx: &Ctx) -> SkillRow {
    let id = skill.skill_id;
    let spec = homunculus_skill_spec(id);
    let passive = spec == Some(HomunculusSkillSpec::Passive);
    let castable = skill.level > 0
        && matches!(
            spec,
            Some(HomunculusSkillSpec::SelfTarget | HomunculusSkillSpec::Enemy)
        );
    let rank = selected_rank(ctx.ui, id, skill.level);
    let meta = ctx.skills.and_then(|skills| skills.get(id));
    let about = meta
        .and_then(|meta| {
            description::parse(&meta.description, &meta.display_name)
                .into_iter()
                .next()
        })
        .map(|runs| runs.into_iter().map(|(_, text)| text).collect())
        .unwrap_or_default();
    let cost = meta
        .filter(|_| castable)
        .and_then(|meta| meta.sp_cost.get(rank.saturating_sub(1) as usize))
        .filter(|cost| **cost > 0)
        .map(|cost| format!("{cost} SP"));
    let short = skill.intimacy_required_hundredths > snapshot.intimacy_hundredths;
    let lock = (skill.level == 0).then(|| {
        if short {
            let needed = skill.intimacy_required_hundredths / 100;
            (format!("Needs {needed} intimacy"), ro_dark::TONE_ERROR)
        } else if skill.learnable {
            ("Ready to learn".into(), ro_dark::BRONZE_LIGHT)
        } else {
            ("Needs another skill first".into(), ro_dark::MUTED)
        }
    });
    let auto = snapshot.ai_config.as_ref().is_some_and(|config| {
        config
            .skills
            .iter()
            .any(|rule| rule.skill_id == id && rule.mode == Mode::Auto)
    });
    SkillRow {
        id,
        name: ctx.skill_name(id),
        level: format!("Lv {} / {}", skill.level, skill.max_level),
        passive,
        auto: auto && castable,
        about,
        lock,
        cost,
        icon: ctx.skills.and_then(|skills| skills.icon_path(id)),
        rank: (castable && skill.level > 1).then_some(rank),
        can_learn: skill.learnable && snapshot.skill_points > 0 && skill.level < skill.max_level,
        can_cast: castable && ctx.out,
    }
}

/// A slider of the Behaviour page.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DialView {
    pub dial: Dial,
    pub label: String,
    /// How far along the slider is, from 0 to 1.
    pub fraction: f32,
    pub readout: String,
}

fn dial(dial: Dial, label: String, config: &Config) -> DialView {
    DialView {
        dial,
        label,
        fraction: dial.fraction(config),
        readout: dial.readout(config),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Control {
    Dial(DialView),
    Flag(Flag, bool),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TacticRow {
    pub label: &'static str,
    pub about: String,
    pub control: Control,
}

/// One skill's rule. `place` counts from 1 in the order the server runs them.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RuleView {
    pub skill_id: u32,
    pub place: usize,
    pub name: String,
    pub auto: bool,
    /// The HP conditions of an automatic rule.
    pub dials: Vec<DialView>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TacticsBody {
    pub stance: Stance,
    pub rows: Vec<TacticRow>,
    pub rules: Vec<RuleView>,
    pub only: Vec<u32>,
    pub never: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TacticsView {
    pub name: String,
    /// The draft's controls. `None` while there is nothing the window can edit.
    pub body: Option<TacticsBody>,
    pub state: (String, Color),
    pub dirty: bool,
    pub can_teach: bool,
}

fn tactics(snapshot: &Snapshot, ctx: &Ctx) -> TacticsView {
    let name = snapshot.name.clone();
    let editor = &ctx.ui.tactics;
    let muted = |text: &str| (text.to_owned(), ro_dark::MUTED);
    let alert = |text: &str| (text.to_owned(), ro_dark::STATUS_BUSY);
    let closed = |state| TacticsView {
        name: name.clone(),
        body: None,
        state,
        dirty: false,
        can_teach: false,
    };
    let (Some(config), Some(draft)) = (snapshot.ai_config.as_ref(), editor.draft()) else {
        return closed(muted("This companion has no tactics to edit."));
    };
    if !tactics::known(config) {
        return closed(alert("These tactics hold rules this client cannot edit."));
    }
    let dirty = editor.dirty();
    let stale = dirty && editor.stale(config);
    let state = if stale {
        alert("The tactics changed on the server. Undo Changes to load them.")
    } else if let Some(refusal) = ctx.refusal.filter(|_| editor.sent()) {
        (refusal.to_owned(), ro_dark::TONE_ERROR)
    } else if dirty {
        ("Changes not taught yet".into(), ro_dark::FROST_GLOW)
    } else {
        muted(&format!("{name} knows these rules"))
    };
    TacticsView {
        body: Some(tactics_body(draft, snapshot, ctx)),
        state,
        dirty,
        can_teach: dirty && !stale,
        name,
    }
}

fn tactics_body(draft: &Config, snapshot: &Snapshot, ctx: &Ctx) -> TacticsBody {
    let name = &snapshot.name;
    let slide = |label, about: String, which: Dial| TacticRow {
        label,
        about,
        control: Control::Dial(dial(which, label.into(), draft)),
    };
    let toggle = |label, about: String, flag: Flag| TacticRow {
        label,
        about,
        control: Control::Flag(flag, flag.read(draft)),
    };
    let food = ctx.food_name(snapshot.food_item_id);
    let rows = vec![
        slide(
            "How far it roams",
            format!("{name} comes back if it gets this far from you"),
            Dial::Leash,
        ),
        toggle(
            "Join your target",
            "Attack whatever you are fighting".into(),
            Flag::Join,
        ),
        toggle(
            "Fight back",
            format!("Hit back when something attacks {name}"),
            Flag::Retaliate,
        ),
        toggle(
            "Stay away from bosses",
            "Never start a fight with an MVP or boss".into(),
            Flag::AvoidBoss,
        ),
        toggle(
            "Feed itself",
            format!("Eat {food} from your bag when hungry"),
            Flag::AutoFeed,
        ),
        slide(
            "Eat when hunger drops to",
            format!("Below {HUNGRY} {name} is hungry; below 10 it loses intimacy"),
            Dial::FeedAt,
        ),
        slide(
            "Save SP for you",
            format!("Auto skills leave this much of {name}\u{2019}s SP unspent"),
            Dial::SpReserve,
        ),
    ];
    let rules = tactics::rules(draft)
        .into_iter()
        .enumerate()
        .map(|(index, rule)| {
            let auto = rule.mode == Mode::Auto;
            let dials = tactics::dials(rule)
                .into_iter()
                .filter(|_| auto)
                .map(|which| {
                    let label = match which {
                        Dial::SelfHp(_) => format!("When {name}\u{2019}s HP is under"),
                        Dial::OwnerHp(_) => "When your HP is under".into(),
                        _ => "When the target\u{2019}s HP is under".into(),
                    };
                    dial(which, label, draft)
                })
                .collect();
            RuleView {
                skill_id: rule.skill_id,
                place: index + 1,
                name: ctx.skill_name(rule.skill_id),
                auto,
                dials,
            }
        })
        .collect();
    TacticsBody {
        stance: draft.stance,
        rows,
        rules,
        only: draft.allowed_mob_class_ids.clone(),
        never: draft.denied_mob_class_ids.clone(),
    }
}

/// The prompt before a step that can cost the companion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConfirmView {
    pub title: String,
    pub text: String,
    /// The "I understand" switch of a deletion, and whether it is on.
    pub sure: Option<bool>,
    pub keep: String,
    pub go: &'static str,
}

pub(super) fn confirm(pending: Confirmation, snapshot: &Snapshot, ctx: &Ctx) -> ConfirmView {
    let name = &snapshot.name;
    if !pending.delete {
        return ConfirmView {
            title: format!("Feed {name} anyway?"),
            text: format!(
                "{name} is not hungry. Overfeeding lowers intimacy and can lose you your companion for good."
            ),
            sure: None,
            keep: "Cancel".into(),
            go: "Feed Anyway",
        };
    }
    ConfirmView {
        title: format!("Delete {name}?"),
        text: format!(
            "{name} will be gone for good, with every level, skill and point of intimacy. You\u{2019}d have to make a new Embryo and start again from level 1."
        ),
        sure: Some(ctx.ui.sure),
        keep: format!("Keep {name}"),
        go: "Delete Forever",
    }
}

/// The HUD plate of a companion that is out.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct HudView {
    pub name: String,
    pub species: String,
    pub level: u32,
    pub grade: &'static str,
    pub activity: String,
    pub tone: Color,
    pub hunger: u32,
    pub hunger_word: &'static str,
    pub hunger_tone: Color,
    pub hungry: bool,
    pub order: Option<Order>,
    pub out: bool,
    pub can_feed: bool,
    /// The automatic rule that runs first.
    pub auto: Option<String>,
    /// The keys of Follow, Attack, Stay and Feed.
    pub keys: [String; 4],
    /// Shows the portrait alone; a click on it brings the plate back.
    pub minimal: bool,
}

/// The plate, or `None` unless the companion is out.
pub(super) fn hud(snapshot: &Snapshot, ctx: &Ctx, keys: [String; 4]) -> Option<HudView> {
    if !active(snapshot) {
        return None;
    }
    let (word, tone) = activity(snapshot);
    let (hunger_word, hunger_tone) = hunger(snapshot.hunger);
    let hungry = snapshot.hunger <= HUNGRY;
    let auto = snapshot
        .ai_config
        .as_ref()
        .filter(|config| tactics::known(config))
        .and_then(|config| {
            tactics::rules(config)
                .into_iter()
                .find(|rule| rule.mode == Mode::Auto)
        })
        .map(|rule| {
            let name = ctx.skill_name(rule.skill_id);
            match rule.owner_hp_threshold {
                Some(under) => format!("Auto {name} <{under}%"),
                None => format!("Auto {name}"),
            }
        });
    Some(HudView {
        name: snapshot.name.clone(),
        species: species(snapshot),
        level: snapshot.level,
        grade: grade(snapshot.intimacy_grade),
        activity: if hungry {
            format!("{word} \u{b7} Hungry")
        } else {
            word.into()
        },
        tone,
        hunger: snapshot.hunger,
        hunger_word,
        hunger_tone,
        hungry,
        order: order(snapshot),
        out: ctx.out,
        can_feed: ctx.out && ctx.food > 0,
        auto,
        keys,
        minimal: ctx.plate_minimal,
    })
}
