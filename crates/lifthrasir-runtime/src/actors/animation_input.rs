use super::ActionType;
use bevy::prelude::*;
use std::time::Duration;

/// A presentation request in Bevy's playback time domain, never server/wall time.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ActorAnimationInput {
    pub action: ActionType,
    /// Increment for each new request, even if its action and timestamp are unchanged.
    pub restart_id: u64,
    pub started_at: Duration,
    /// Optional full-cycle duration, used for attack stretching and model seeking.
    pub duration: Option<Duration>,
    /// Natural frame-delay multiplier: 1 is natural, greater than 1 is slower.
    pub speed_factor: f32,
    pub paused: bool,
}

impl Default for ActorAnimationInput {
    fn default() -> Self {
        Self {
            action: ActionType::Idle,
            restart_id: 0,
            started_at: Duration::ZERO,
            duration: None,
            speed_factor: 1.0,
            paused: false,
        }
    }
}

impl ActorAnimationInput {
    /// Use a natural rate for invalid presentation input instead of propagating NaNs.
    pub fn delay_factor(&self) -> f32 {
        if self.speed_factor.is_finite() && self.speed_factor > 0.0 {
            self.speed_factor
        } else {
            1.0
        }
    }
}

/// Presentation-owned sprite progress, independent of gameplay state and timers.
/// Stores natural and fixed-duration clocks separately so rate changes do not restart a pose.
#[derive(Debug, Clone, Default)]
pub struct ActorAnimationClock {
    request: Option<(u64, ActionType)>,
    updated_at: Duration,
    elapsed: Duration,
    natural_ms: f64,
    factor: f32,
    paused: bool,
}

impl ActorAnimationClock {
    /// Capture progress and apply a request. Returns true only when playback restarts.
    /// A paused existing pose defers action/restart changes until the caller resumes it.
    /// A deferred request still honors started_at; callers choose its intended playback age.
    pub fn update(&mut self, input: &ActorAnimationInput, now: Duration) -> bool {
        let restart = self.request != Some((input.restart_id, input.action));
        let factor = input.delay_factor();
        if restart && !(input.paused && self.request.is_some()) {
            self.request = Some((input.restart_id, input.action));
            self.elapsed = now.saturating_sub(input.started_at);
            self.natural_ms = self.elapsed.as_secs_f64() * 1000.0;
            self.updated_at = now.max(input.started_at);
            self.factor = factor;
            self.paused = input.paused;
            return true;
        }
        let delta = if self.paused {
            Duration::ZERO
        } else {
            now.saturating_sub(self.updated_at)
        };
        self.elapsed = self.elapsed.saturating_add(delta);
        self.natural_ms += delta.as_secs_f64() * 1000.0;
        if factor != self.factor {
            self.natural_ms *= f64::from(factor) / f64::from(self.factor);
        }
        self.updated_at = self.updated_at.max(now);
        self.factor = factor;
        self.paused = input.paused;
        false
    }

    pub fn restart_id(&self) -> Option<u64> {
        self.request.map(|(id, _)| id)
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
    pub fn natural_elapsed_ms(&self) -> f64 {
        self.natural_ms
    }
}
