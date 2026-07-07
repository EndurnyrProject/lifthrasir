use bevy::prelude::*;

/// Marks an entity as capable of combat
#[derive(Component, Debug, Clone, Copy)]
pub struct Combatant {
    pub aspd: u16,
}

impl Combatant {
    pub fn new(aspd: u16) -> Self {
        Self { aspd }
    }
}

/// Current attack target for an entity
#[derive(Component, Debug, Clone, Copy)]
pub struct AttackTarget {
    pub target_entity: Entity,
    pub target_aid: u32,
}

impl AttackTarget {
    pub fn new(target_entity: Entity, target_aid: u32) -> Self {
        Self {
            target_entity,
            target_aid,
        }
    }
}

/// Timer for attack animations
#[derive(Component, Debug, Clone)]
pub struct AttackTimer {
    pub timer: Timer,
    pub action_type: u8,
}

impl AttackTimer {
    pub fn new(duration_secs: f32, action_type: u8) -> Self {
        Self {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
            action_type,
        }
    }
}

/// Scheduled target reaction for an incoming hit, spawned as its own entity.
/// Fires when the attacker's swing connects (src_speed/amotion delay), showing
/// the damage number and playing the flinch.
#[derive(Component, Debug, Clone)]
pub struct PendingHitReaction {
    pub target: Entity,
    pub damage: i32,
    pub is_critical: bool,
    pub flinches: bool,
    pub stun_secs: f32,
    pub timer: Timer,
}

/// Entity is in hit stun
#[derive(Component, Debug, Clone)]
pub struct HitStun {
    pub timer: Timer,
}

impl HitStun {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }
}

/// Marker for entities with Endure status (prevents flinch)
#[derive(Component, Debug, Clone, Copy)]
pub struct HasEndure;

/// Marker for dead entities
#[derive(Component, Debug, Clone, Copy)]
pub struct DeadEntity;
