mod animation;
mod observers;
mod status_effects;
pub mod status_icons;
mod status_network;

pub use crate::domain::entities::character::states::animation::AnimationState;
pub use crate::domain::entities::character::states::status_effects::StatusEffects;

use bevy::prelude::*;
use moonshine_behavior::prelude::*;
use {
    crate::domain::system_sets::CombatSystems,
    lifthrasir_runtime::rendering::SpriteRenderingSystems,
};

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
