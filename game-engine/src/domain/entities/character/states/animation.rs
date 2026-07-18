use crate::domain::entities::character::components::visual::ActionType;
use bevy::prelude::*;
use moonshine_behavior::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Component, Reflect, Default)]
#[component(storage = "SparseSet")]
pub enum AnimationState {
    #[default]
    Idle,
    CombatReady,
    Walking,
    Attacking,
    Hit,
    Sitting,
    Dead,
    PickingUp,
    Casting,
}

impl Behavior for AnimationState {
    fn filter_next(&self, next: &Self) -> bool {
        use AnimationState::*;
        match (self, next) {
            // Dead is terminal - no transitions out
            (Dead, _) => false,
            // Idle can transition to any state
            (
                Idle,
                CombatReady | Walking | Attacking | Hit | Sitting | Dead | PickingUp | Casting,
            ) => true,
            // CombatReady is the engaged idle stance: behaves like Idle
            (
                CombatReady,
                Idle | Walking | Attacking | Hit | Sitting | Dead | PickingUp | Casting,
            ) => true,
            // Walking can transition to any state
            (Walking, Idle | Attacking | Hit | Sitting | Dead | PickingUp | Casting) => true,
            // Attacking can go back to idle/combat-ready, or be interrupted
            (Attacking, Idle | CombatReady | Hit | Sitting | Dead | Casting) => true,
            // Hit can recover to idle, swing back (flinch is interruptible by an attack) or die
            (Hit, Idle | Attacking | Dead) => true,
            // Casting holds until the cast resolves, is interruptible, and the
            // executed skill may swing straight into an attack motion
            (Casting, Idle | CombatReady | Walking | Attacking | Hit | Dead) => true,
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
            AnimationState::CombatReady => ActionType::ReadyFight,
            AnimationState::Walking => ActionType::Walk,
            AnimationState::Attacking => ActionType::Attack,
            AnimationState::Hit => ActionType::Hit,
            AnimationState::Sitting => ActionType::Sit,
            AnimationState::Dead => ActionType::Dead,
            AnimationState::PickingUp => ActionType::Special,
            AnimationState::Casting => ActionType::Cast,
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

    #[test]
    fn casting_maps_to_cast_action() {
        assert_eq!(ActionType::from(AnimationState::Casting), ActionType::Cast);
    }

    #[test]
    fn idle_walking_and_combat_ready_can_start_casting() {
        assert!(AnimationState::Idle.filter_next(&AnimationState::Casting));
        assert!(AnimationState::Walking.filter_next(&AnimationState::Casting));
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Casting));
    }

    #[test]
    fn casting_resolves_to_idle_and_is_interruptible() {
        assert!(AnimationState::Casting.filter_next(&AnimationState::Idle));
        assert!(AnimationState::Casting.filter_next(&AnimationState::Attacking));
        assert!(AnimationState::Casting.filter_next(&AnimationState::Hit));
        assert!(AnimationState::Casting.filter_next(&AnimationState::Dead));
        assert!(!AnimationState::Casting.filter_next(&AnimationState::Sitting));
    }

    #[test]
    fn combat_ready_maps_to_ready_fight_action() {
        assert_eq!(
            ActionType::from(AnimationState::CombatReady),
            ActionType::ReadyFight
        );
    }

    #[test]
    fn idle_and_attacking_can_enter_combat_ready() {
        assert!(AnimationState::Idle.filter_next(&AnimationState::CombatReady));
        assert!(AnimationState::Attacking.filter_next(&AnimationState::CombatReady));
    }

    #[test]
    fn combat_ready_is_interruptible_like_idle() {
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Attacking));
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Hit));
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Walking));
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Dead));
        assert!(AnimationState::CombatReady.filter_next(&AnimationState::Idle));
    }
}
