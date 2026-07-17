use super::catalog::{process_loaded_effect_data, start_loading_effect_data};
use crate::domain::effects::{
    advance_effect_timers, apply_body_state_tint, body_state_visuals, despawn_finished_effects,
    follow_effect_anchor, initialize_effect_layers, on_ground_skill, on_skill_damage,
    on_skill_effect, on_special_effect, order_effect_layers_by_depth, rebuild_effect_layers,
    EffectLayer, PendingBodyStates, PlayProceduralVfx,
};
use crate::domain::system_sets::EntityLifecycleSystems;
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
            .init_resource::<PendingBodyStates>()
            .add_systems(Startup, start_loading_effect_data)
            .add_systems(
                Update,
                (
                    process_loaded_effect_data,
                    // The skill-event and special-effect consumers spawn the effect instances.
                    (
                        on_skill_effect,
                        on_skill_damage,
                        on_ground_skill,
                        on_special_effect,
                    ),
                    follow_effect_anchor,
                    // timers advance current_frame/finished before rebuild and despawn read them;
                    // initialize creates the layer children rebuild queries over.
                    (
                        advance_effect_timers,
                        initialize_effect_layers,
                        // Gated so the Assets<Mesh>/Assets<EffectMaterial> ResMut
                        // access doesn't serialize the schedule while no effect
                        // is playing (the common case).
                        rebuild_effect_layers.run_if(any_with_component::<EffectLayer>),
                        order_effect_layers_by_depth,
                        despawn_finished_effects,
                    )
                        .chain(),
                ),
            )
            // Runs after entity spawning so a `UnitEntered` unit is registered
            // before we resolve it; `apply_body_state_tint` rides the per-frame
            // layer material write.
            .add_systems(
                Update,
                (
                    body_state_visuals.after(EntityLifecycleSystems::Spawning),
                    apply_body_state_tint,
                ),
            );
    }
}
