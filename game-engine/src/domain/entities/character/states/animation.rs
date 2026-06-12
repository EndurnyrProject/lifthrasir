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
    Dead,
}

impl Behavior for AnimationState {
    fn filter_next(&self, next: &Self) -> bool {
        use AnimationState::*;
        match (self, next) {
            // Dead is terminal - no transitions out
            (Dead, _) => false,
            // Idle can transition to any state
            (Idle, Walking | Attacking | Hit | Dead) => true,
            // Walking can transition to any state
            (Walking, Idle | Attacking | Hit | Dead) => true,
            // Attacking can go back to idle, or be interrupted
            (Attacking, Idle | Hit | Dead) => true,
            // Hit can recover to idle or die
            (Hit, Idle | Dead) => true,
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
            AnimationState::Dead => ActionType::Dead,
        }
    }
}
