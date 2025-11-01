use bevy::prelude::*;

/// Animation state for fine-grained control
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// Animation is playing normally
    Playing,

    /// Animation is paused (system skips entirely)
    Paused,

    /// Animation completed (non-looping)
    Finished,

    /// Animation is waiting to start
    Waiting,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::Playing
    }
}
