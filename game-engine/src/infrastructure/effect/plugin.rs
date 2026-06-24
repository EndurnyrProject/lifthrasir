use super::catalog::{process_loaded_skill_effect_data, start_loading_skill_effect_data};
use crate::domain::effects::{
    advance_effect_timers, despawn_finished_effects, follow_effect_anchor,
    initialize_effect_layers, rebuild_effect_layers,
};
use crate::presentation::rendering::effect_material::EffectMaterial;
use bevy::prelude::*;

/// Aggregate plugin for the skill-effect subsystem: registers the
/// `EffectMaterial` render pipeline, the `EffectCatalog` startup-load, and the
/// effect runtime systems.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<EffectMaterial>::default())
            .add_systems(Startup, start_loading_skill_effect_data)
            .add_systems(
                Update,
                (
                    process_loaded_skill_effect_data,
                    follow_effect_anchor,
                    // timers advance current_frame/finished before rebuild and despawn read them;
                    // initialize creates the layer children rebuild queries over.
                    (
                        advance_effect_timers,
                        initialize_effect_layers,
                        rebuild_effect_layers,
                        despawn_finished_effects,
                    )
                        .chain(),
                ),
            );
    }
}
