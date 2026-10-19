//! Client bridge fixtures exercise the actual shared playback implementation.
use crate::domain::entities::model_rendering::*;
use bevy::animation::RepeatAnimation;
use lifthrasir_runtime::actors::models::test_support::{ActorPlayback, ModelClip, sync_animation};
use lifthrasir_runtime::actors::{ActionType, ActorAnimationInput};
use {
    crate::domain::{
        combat::components::AttackTimer,
        entities::{character::states::AnimationState, movement::components::MovementSpeed},
    },
    lifthrasir_runtime::effects::overlays::AnimationPaused,
};

mod tests;
