use super::catalog::{process_loaded_effect_data, start_loading_effect_data};
use crate::domain::effects::{
    advance_effect_timers, despawn_finished_effects, follow_effect_anchor,
    initialize_effect_layers, on_ground_skill, on_skill_damage, on_skill_effect,
    order_effect_layers_by_depth, rebuild_effect_layers, PlayProceduralVfx,
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
            .add_message::<PlayProceduralVfx>()
            .add_systems(Startup, start_loading_effect_data)
            .add_systems(
                Update,
                (
                    process_loaded_effect_data,
                    // The three skill-event consumers spawn the effect instances.
                    (on_skill_effect, on_skill_damage, on_ground_skill),
                    follow_effect_anchor,
                    // timers advance current_frame/finished before rebuild and despawn read them;
                    // initialize creates the layer children rebuild queries over.
                    (
                        advance_effect_timers,
                        initialize_effect_layers,
                        rebuild_effect_layers,
                        order_effect_layers_by_depth,
                        despawn_finished_effects,
                    )
                        .chain(),
                ),
            );
    }
}
