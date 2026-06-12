mod animation;
mod observers;
mod status_effects;

pub use animation::AnimationState;
pub use status_effects::StatusEffects;

use bevy::prelude::*;
use moonshine_behavior::prelude::*;

pub fn setup_character_state_machines(app: &mut App) {
    app.add_plugins(BehaviorPlugin::<AnimationState>::default());
}
