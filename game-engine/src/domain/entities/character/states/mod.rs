mod animation;
mod observers;
mod status_effects;

pub use animation::AnimationState;
pub use status_effects::StatusEffects;

use crate::domain::system_sets::{CombatSystems, SpriteRenderingSystems};
use bevy::prelude::*;
use moonshine_behavior::prelude::*;

pub fn setup_character_state_machines(app: &mut App) {
    app.add_plugins(BehaviorPlugin::<AnimationState>::default());

    // moonshine only applies queued transitions when this system runs; without it the
    // behavior stack and `filter_next` guards never execute. Run it after gameplay has
    // queued its transitions (Combat is chain-ordered with HandleDeath last, so death
    // wins the slot) and before sprite sync reads the resulting `AnimationState`.
    app.add_systems(
        Update,
        transition::<AnimationState>
            .after(CombatSystems::HandleDeath)
            .before(SpriteRenderingSystems::AnimationSync),
    );
}
