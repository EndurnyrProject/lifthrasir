use bevy::prelude::*;

/// Combat action types from ZC_NOTIFY_ACT (rAthena e_damage_type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatActionType {
    /// Normal attack with damage; target flinches
    Attack = 0,
    PickupItem = 1,
    SitDown = 2,
    StandUp = 3,
    /// Damage while enduring; target does not flinch
    Endure = 4,
    Splash = 5,
    Skill = 6,
    Repeat = 7,
    /// Multi-hit damage (e.g. double attack); target flinches
    MultiHit = 8,
    /// Multi-hit damage while enduring; target does not flinch
    MultiHitEndure = 9,
    /// Critical hit; target flinches
    Critical = 10,
    LuckyDodge = 11,
    /// Multi-hit critical; target flinches
    MultiHitCritical = 13,
    Unknown = 255,
}

impl CombatActionType {
    /// Action deals damage and plays the attacker's attack animation
    pub fn is_damage(self) -> bool {
        matches!(
            self,
            Self::Attack
                | Self::Endure
                | Self::MultiHit
                | Self::MultiHitEndure
                | Self::Critical
                | Self::MultiHitCritical
        )
    }

    /// Target plays the hit animation (endure variants suppress the flinch)
    pub fn target_flinches(self) -> bool {
        matches!(
            self,
            Self::Attack | Self::MultiHit | Self::Critical | Self::MultiHitCritical
        )
    }

    pub fn is_critical(self) -> bool {
        matches!(self, Self::Critical | Self::MultiHitCritical)
    }
}

impl From<u8> for CombatActionType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Attack,
            1 => Self::PickupItem,
            2 => Self::SitDown,
            3 => Self::StandUp,
            4 => Self::Endure,
            5 => Self::Splash,
            6 => Self::Skill,
            7 => Self::Repeat,
            8 => Self::MultiHit,
            9 => Self::MultiHitEndure,
            10 => Self::Critical,
            11 => Self::LuckyDodge,
            13 => Self::MultiHitCritical,
            _ => Self::Unknown,
        }
    }
}

/// Event received from server for combat actions
#[derive(Message, Debug, Clone)]
pub struct CombatActionReceived {
    pub src_id: u32,
    pub target_id: u32,
    pub server_tick: u32,
    pub src_speed: i32,
    pub dmg_speed: i32,
    pub damage: i32,
    pub is_sp_damage: bool,
    pub div: u16,
    pub action_type: CombatActionType,
    pub damage2: i32,
}

/// Entity HP information received from server (ZC_HP_INFO)
/// Used for any entity type: players, monsters, NPCs, etc.
#[derive(Message, Debug, Clone)]
pub struct EntityHpReceived {
    pub entity_id: u32,
    pub hp: u32,
    pub max_hp: u32,
}

/// Display damage number on screen
#[derive(Message, Debug, Clone)]
pub struct DisplayDamageNumber {
    pub entity: Entity,
    pub amount: i32,
    pub damage_type: DamageDisplayType,
}

/// Type of damage to display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageDisplayType {
    Normal,
    Critical,
    Miss,
}
