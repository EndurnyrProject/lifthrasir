//! Client bridge fixtures exercise the actual shared playback implementation.
use super::*;
use crate::domain::{
    combat::components::AttackTimer,
    effects::AnimationPaused,
    entities::{character::states::AnimationState, movement::components::MovementSpeed},
};
use bevy::animation::RepeatAnimation;
pub(super) use lifthrasir_runtime::actors::models::test_support::{
    ActorPlayback, ModelClip, sync_animation,
};
use lifthrasir_runtime::actors::{ActionType, ActorAnimationInput};

mod tests;
