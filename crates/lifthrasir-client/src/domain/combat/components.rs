use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

static ATTACK_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Timer for attack animations
#[derive(Component, Debug, Clone)]
pub struct AttackTimer {
    pub timer: Timer,
    /// Distinguishes new requests even when their start time and duration match.
    pub request_id: u64,
}

impl AttackTimer {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
            request_id: ATTACK_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
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
    /// The target dies on this hit: the death animation plays when the swing
    /// connects instead of the instant the server vanish arrives.
    pub kills_target: bool,
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

/// Death arrived before its killing blow: the despawn (WORLD channel) and the
/// damage (GAMEPLAY channel) travel on independently ordered streams, so the
/// corpse is held briefly for the swing to land and bind. Expiring without a
/// blow plays the death immediately.
#[derive(Component, Debug, Clone)]
pub struct DeathGrace {
    pub timer: Timer,
}

impl DeathGrace {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }
}
