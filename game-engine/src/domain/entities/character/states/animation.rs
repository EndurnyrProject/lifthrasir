use crate::domain::entities::character::components::visual::ActionType;
use bevy::prelude::*;
use moonshine_behavior::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Component, Reflect, Default)]
#[component(storage = "SparseSet")]
pub enum AnimationState {
    #[default]
    Idle,
    Walking,
    Attacking,
    Hit,
    Sitting,
    Dead,
}

impl Behavior for AnimationState {
    fn filter_next(&self, next: &Self) -> bool {
        use AnimationState::*;
        match (self, next) {
            // Dead is terminal - no transitions out
            (Dead, _) => false,
            // Idle can transition to any state
            (Idle, Walking | Attacking | Hit | Sitting | Dead) => true,
            // Walking can transition to any state
            (Walking, Idle | Attacking | Hit | Sitting | Dead) => true,
            // Attacking can go back to idle, or be interrupted
            (Attacking, Idle | Hit | Sitting | Dead) => true,
            // Hit can recover to idle, swing back (flinch is interruptible by an attack) or die
            (Hit, Idle | Attacking | Dead) => true,
            // Sitting can stand, be interrupted, or die
            (Sitting, Idle | Walking | Attacking | Hit | Dead) => true,
            // Same state is always valid (no-op)
            (a, b) if a == b => true,
            // All other transitions are invalid
            _ => false,
        }
    }
}

impl From<AnimationState> for ActionType {
    fn from(state: AnimationState) -> Self {
        match state {
            AnimationState::Idle => ActionType::Idle,
            AnimationState::Walking => ActionType::Walk,
            AnimationState::Attacking => ActionType::Attack,
            AnimationState::Hit => ActionType::Hit,
            AnimationState::Sitting => ActionType::Sit,
            AnimationState::Dead => ActionType::Dead,
        }
    }
}
