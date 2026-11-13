//! What the status bar shows, as plain values: the numbers a hero's status boils down
//! to and the text, ratios and tooltips made from them. No ECS in here.

use bevy::prelude::*;
use lifthrasir_ui::theme::ro_dark;

use crate::screens::shared::{Load, thousands};

/// Share of HP under which the bar pulses and the portrait ring turns red.
const LOW_HP: f32 = 25.0;

/// A resource drawn as a bar.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Gauge {
    #[default]
    Hp,
    Sp,
    Ap,
    BaseExp,
    JobExp,
    Weight,
}

impl Gauge {
    /// The three bars stacked in the middle of the plate.
    pub(super) const VITALS: [Self; 3] = [Self::Hp, Self::Sp, Self::Ap];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Hp => "HP",
            Self::Sp => "SP",
            Self::Ap => "AP",
            Self::BaseExp => "BASE EXP",
            Self::JobExp => "JOB EXP",
            Self::Weight => "WEIGHT",
        }
    }

    /// Colour of the gauge's label and of its tooltip title.
    pub(super) fn ink(self) -> Color {
        match self {
            Self::Hp => Color::srgb_u8(0xe8, 0xa4, 0x9d),
            Self::Sp => Color::srgb_u8(0x9c, 0xc0, 0xe6),
            Self::Ap => ro_dark::BRONZE_LIGHT,
            Self::BaseExp => Color::srgb_u8(0xd9, 0xad, 0x57),
            Self::JobExp => ro_dark::STATUS_ONLINE,
            Self::Weight => ro_dark::STATUS_ONLINE,
        }
    }
}

/// A piece of text the bar keeps up to date.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Field {
    #[default]
    Name,
    Job,
    BaseLevel,
    JobLevel,
    Zeny,
    /// The gauge's amount: a number, or a percentage for EXP.
    Value(Gauge),
    /// The gauge's limit, written after its amount.
    Max(Gauge),
    /// The gauge in the least room: a rounded percentage, or the AP count.
    Short(Gauge),
}

/// What hovering a part of the bar explains.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tip {
    #[default]
    Zeny,
    Gauge(Gauge),
}

/// A tooltip's title, the title's colour and the lines under it.
#[derive(Debug, PartialEq)]
pub(super) struct TipView {
    pub title: String,
    pub tone: Color,
    pub lines: Vec<String>,
}

/// The local hero's status, reduced to what the bar draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct StatusView {
    pub name: String,
    pub job: String,
    pub base_level: u32,
    pub job_level: u32,
    pub hp: (u32, u32),
    pub sp: (u32, u32),
    pub ap: (u32, u32),
    pub base_exp: (u32, u32),
    pub job_exp: (u32, u32),
    pub weight: (u32, u32),
    pub zeny: u32,
    /// Whether the job builds AP at all: fourth jobs with an AP pool.
    pub has_ap: bool,
}

impl StatusView {
    fn amounts(&self, gauge: Gauge) -> (u32, u32) {
        match gauge {
            Gauge::Hp => self.hp,
            Gauge::Sp => self.sp,
            Gauge::Ap => self.ap,
            Gauge::BaseExp => self.base_exp,
            Gauge::JobExp => self.job_exp,
            Gauge::Weight => self.weight,
        }
    }

    /// How full the gauge is, from 0 to 100.
    pub(super) fn percent(&self, gauge: Gauge) -> f32 {
        let (amount, limit) = self.amounts(gauge);
        percent(amount, limit)
    }

    pub(super) fn load(&self) -> Load {
        Load::of(self.weight.0, self.weight.1)
    }

    /// Colour of the weight read-out: green, amber past 50%, red past 90%.
    pub(super) fn load_tone(&self) -> Color {
        match self.load() {
            Load::Light => ro_dark::STATUS_ONLINE,
            Load::Heavy => ro_dark::STATUS_BUSY,
            Load::Overweight => ro_dark::STATUS_FULL,
        }
    }

    pub(super) fn text(&self, field: Field) -> String {
        match field {
            Field::Name => self.name.clone(),
            Field::Job => self.job.clone(),
            Field::BaseLevel => self.base_level.to_string(),
            Field::JobLevel => self.job_level.to_string(),
            Field::Zeny => format!("{} z", short(self.zeny)),
            Field::Value(gauge @ (Gauge::BaseExp | Gauge::JobExp)) => {
                format!("{:.1}%", self.percent(gauge))
            }
            Field::Value(Gauge::Weight) => self.pair(Gauge::Weight),
            Field::Value(gauge) => thousands(self.amounts(gauge).0.into()),
            Field::Max(gauge) => format!("/ {}", thousands(self.amounts(gauge).1.into())),
            Field::Short(Gauge::Ap) => self.ap.0.to_string(),
            Field::Short(Gauge::Weight) => format!("{}%", self.percent(Gauge::Weight).floor()),
            Field::Short(gauge) => format!("{}%", self.percent(gauge).round()),
        }
    }

    /// `1,470 / 3,100`.
    fn pair(&self, gauge: Gauge) -> String {
        let (amount, limit) = self.amounts(gauge);
        format!("{} / {}", thousands(amount.into()), thousands(limit.into()))
    }

    pub(super) fn tip(&self, tip: Tip) -> TipView {
        let Tip::Gauge(gauge) = tip else {
            return TipView {
                title: format!("{} z", thousands(self.zeny.into())),
                tone: ro_dark::BRONZE_LIGHT,
                lines: vec!["Carried zeny".into()],
            };
        };
        let (title, tone, lines) = match gauge {
            Gauge::Hp => {
                let stopped = match self.load() {
                    Load::Light => "",
                    _ => " (stopped: overweight)",
                };
                let line = format!(
                    "{} \u{b7} Regenerates while sitting{stopped}",
                    self.text(Field::Short(gauge))
                );
                (format!("HP {}", self.pair(gauge)), gauge.ink(), vec![line])
            }
            Gauge::Sp => (
                format!("SP {}", self.pair(gauge)),
                gauge.ink(),
                vec!["Spent by skills \u{b7} Regenerates over time".into()],
            ),
            Gauge::Ap => (
                format!("AP {}", self.pair(gauge)),
                gauge.ink(),
                vec![
                    "Built up by skills in combat".into(),
                    format!("Spent by {} skills", self.job),
                ],
            ),
            Gauge::BaseExp => (
                "Base EXP".into(),
                gauge.ink(),
                self.to_next("Level", self.base_level, gauge),
            ),
            Gauge::JobExp => (
                "Job EXP".into(),
                gauge.ink(),
                self.to_next("Job level", self.job_level, gauge),
            ),
            Gauge::Weight => (
                format!("Weight {}", self.pair(gauge)),
                self.load_tone(),
                self.weight_lines(),
            ),
        };
        TipView { title, tone, lines }
    }

    fn to_next(&self, what: &str, level: u32, gauge: Gauge) -> Vec<String> {
        vec![
            format!("{what} {level} \u{2192} {}", level + 1),
            format!("{:.1}% to go", 100.0 - self.percent(gauge)),
        ]
    }

    fn weight_lines(&self) -> Vec<String> {
        let (weight, limit) = self.weight;
        let (rule, next) = match self.load() {
            Load::Light => ("Under 50%: no penalties", Some(limit.div_ceil(2))),
            Load::Heavy => (
                "Over 50%: HP and SP stop regenerating",
                Some((u64::from(limit) * 9).div_ceil(10) as u32),
            ),
            Load::Overweight => ("Over 90%: you can\u{2019}t attack or use skills", None),
        };
        let room = next.map(|next| {
            format!(
                "{} until the next limit",
                thousands(next.saturating_sub(weight).into())
            )
        });
        std::iter::once(rule.to_string()).chain(room).collect()
    }
}

/// `amount` as a share of `limit`, from 0 to 100. An empty limit reads as empty.
pub(super) fn percent(amount: u32, limit: u32) -> f32 {
    if limit == 0 {
        return 0.0;
    }
    (amount as f32 / limit as f32 * 100.0).min(100.0)
}

/// Whether so little HP is left that the bar pulses and the portrait ring turns red.
pub(super) fn low_hp(hp: u32, max_hp: u32) -> bool {
    percent(hp, max_hp) < LOW_HP
}

/// Zeny in the room the plate has: `3,420`, `48k`, `1.25m`, `12.5m`.
fn short(zeny: u32) -> String {
    let millions = f64::from(zeny) / 1e6;
    let trimmed = |text: String| {
        let text = text.trim_end_matches('0').trim_end_matches('.');
        format!("{text}m")
    };
    match zeny {
        10_000_000.. => trimmed(format!("{millions:.1}")),
        1_000_000.. => trimmed(format!("{millions:.2}")),
        10_000.. => format!("{}k", (f64::from(zeny) / 1e3).round()),
        _ => thousands(zeny.into()),
    }
}

/// What a level-up banner announces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LevelUp {
    Base(u32),
    Job(u32),
}

impl LevelUp {
    /// The level-up between two `(base, job)` level pairs, if any. A level of zero is
    /// a status the server has not filled in yet, never a level to climb from.
    pub(super) fn between(before: (u32, u32), after: (u32, u32)) -> Option<Self> {
        if before.0 > 0 && after.0 > before.0 {
            return Some(Self::Base(after.0));
        }
        (before.1 > 0 && after.1 > before.1).then_some(Self::Job(after.1))
    }

    /// The small caption and the headline under it.
    pub(super) fn lines(self) -> (&'static str, String) {
        match self {
            Self::Base(level) => ("BASE LEVEL UP", format!("Level {level}")),
            Self::Job(level) => ("JOB LEVEL UP", format!("Job Level {level}")),
        }
    }
}
