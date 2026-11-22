//! Owner-private companion data and requested actions.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomunculusAction {
    Inspect,
    Move {
        x: i32,
        y: i32,
    },
    Follow,
    Attack {
        target_id: u32,
    },
    Standby,
    CastSkill {
        skill_id: u32,
        level: u32,
        target: HomunculusTarget,
    },
    Feed,
    Rename {
        name: String,
    },
    Rest,
    Delete {
        confirmed: bool,
    },
    ReplaceAi {
        config: HomunculusAiConfig,
    },
    LearnSkill {
        skill_id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomunculusTarget {
    SelfTarget,
    Unit(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomunculusError {
    None,
    NoCompanion,
    MalformedCommand,
    WrongChannel,
    InvalidLifecycle,
    InvalidPosition,
    InvalidTarget,
    OutOfRange,
    SkillNotLearned,
    InvalidSkillRank,
    OnCooldown,
    InsufficientSp,
    MissingItem,
    HpGate,
    RenameNotAllowed,
    InvalidName,
    ConfirmationRequired,
    InvalidAiConfig,
    InsufficientSkillPoints,
    PrerequisitesNotMet,
    Busy,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomunculusLifecycle {
    #[default]
    Unspecified,
    Active,
    Rested,
    Dead,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomunculusActivity {
    #[default]
    Unspecified,
    Idle,
    Following,
    Moving,
    Attacking,
    Casting,
    Standby,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomunculusIntimacyGrade {
    #[default]
    Unspecified,
    HateWithPassion,
    Hate,
    Awkward,
    Shy,
    Neutral,
    Cordial,
    Loyal,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomunculusAiStance {
    #[default]
    Unspecified,
    Passive,
    Assist,
    Aggressive,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HomunculusAiSkillMode {
    #[default]
    Unspecified,
    Manual,
    Auto,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomunculusHpRange {
    pub min_percent: u32,
    pub max_percent: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomunculusAiSkillConfig {
    pub skill_id: u32,
    pub mode: HomunculusAiSkillMode,
    pub priority: u32,
    pub self_hp_threshold: Option<u32>,
    pub owner_hp_threshold: Option<u32>,
    pub target_hp_range: Option<HomunculusHpRange>,
}

/// An entire tactics configuration, not a patch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomunculusAiConfig {
    pub stance: HomunculusAiStance,
    pub leash_distance: u32,
    pub join_owner_target: bool,
    pub retaliate: bool,
    pub avoid_bosses: bool,
    pub allowed_mob_class_ids: Vec<u32>,
    pub denied_mob_class_ids: Vec<u32>,
    pub auto_feed: bool,
    pub auto_feed_threshold: u32,
    pub auto_cast_sp_reserve_percent: u32,
    pub skills: Vec<HomunculusAiSkillConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomunculusDisplayedStats {
    pub str: u32,
    pub agi: u32,
    pub vit: u32,
    pub int: u32,
    pub dex: u32,
    pub luk: u32,
    pub atk: u32,
    pub matk: u32,
    pub def: u32,
    pub mdef: u32,
    pub hit: u32,
    pub flee: u32,
    pub critical: u32,
    pub aspd: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomunculusSkillMetadata {
    pub skill_id: u32,
    pub level: u32,
    pub max_level: u32,
    pub learnable: bool,
    pub intimacy_required_hundredths: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomunculusCooldown {
    pub skill_id: u32,
    pub remaining_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomunculusSnapshot {
    pub durable_id: u64,
    pub world_gid: Option<u32>,
    pub name: String,
    pub rename_eligible: bool,
    pub species_id: u32,
    pub evolved: bool,
    pub appearance_id: u32,
    pub lifecycle: HomunculusLifecycle,
    pub activity: HomunculusActivity,
    pub current_target_id: Option<u32>,
    pub level: u32,
    pub exp: u64,
    pub next_exp: u64,
    pub skill_points: u32,
    pub hp: u64,
    pub max_hp: u64,
    pub sp: u64,
    pub max_sp: u64,
    pub stats: Option<HomunculusDisplayedStats>,
    pub hunger: u32,
    pub intimacy_hundredths: u32,
    pub intimacy_grade: HomunculusIntimacyGrade,
    pub food_item_id: u32,
    pub active_remaining_ms: u64,
    pub skills: Vec<HomunculusSkillMetadata>,
    pub cooldowns: Vec<HomunculusCooldown>,
    pub ai_config: Option<HomunculusAiConfig>,
}
