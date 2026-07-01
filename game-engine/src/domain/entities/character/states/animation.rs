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
    PickingUp,
}

impl Behavior for AnimationState {
    fn filter_next(&self, next: &Self) -> bool {
        use AnimationState::*;
        match (self, next) {
            // Dead is terminal - no transitions out
            (Dead, _) => false,
            // Idle can transition to any state
            (Idle, Walking | Attacking | Hit | Sitting | Dead | PickingUp) => true,
            // Walking can transition to any state
            (Walking, Idle | Attacking | Hit | Sitting | Dead | PickingUp) => true,
            // Attacking can go back to idle, or be interrupted
            (Attacking, Idle | Hit | Sitting | Dead) => true,
            // Hit can recover to idle, swing back (flinch is interruptible by an attack) or die
            (Hit, Idle | Attacking | Dead) => true,
            // Sitting can stand, be interrupted, or die
            (Sitting, Idle | Walking | Attacking | Hit | Dead) => true,
            // PickingUp finishes back to idle, is interruptible by a hit/death, or by walking off
            (PickingUp, Idle | Hit | Dead | Walking) => true,
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
            AnimationState::PickingUp => ActionType::Special,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_up_maps_to_special_action() {
        assert_eq!(
            ActionType::from(AnimationState::PickingUp),
            ActionType::Special
        );
    }

    #[test]
    fn idle_and_walking_can_start_picking_up() {
        assert!(AnimationState::Idle.filter_next(&AnimationState::PickingUp));
        assert!(AnimationState::Walking.filter_next(&AnimationState::PickingUp));
    }

    #[test]
    fn picking_up_returns_to_idle_and_is_interruptible() {
        assert!(AnimationState::PickingUp.filter_next(&AnimationState::Idle));
        assert!(AnimationState::PickingUp.filter_next(&AnimationState::Hit));
        assert!(AnimationState::PickingUp.filter_next(&AnimationState::Dead));
        assert!(AnimationState::PickingUp.filter_next(&AnimationState::Walking));
        assert!(!AnimationState::PickingUp.filter_next(&AnimationState::Sitting));
    }

    #[test]
    fn dead_is_still_terminal() {
        assert!(!AnimationState::Dead.filter_next(&AnimationState::PickingUp));
    }
}
