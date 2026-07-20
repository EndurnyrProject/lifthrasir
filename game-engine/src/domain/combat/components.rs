use bevy::prelude::*;

/// Timer for attack animations
#[derive(Component, Debug, Clone)]
pub struct AttackTimer {
    pub timer: Timer,
}

impl AttackTimer {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
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
