use crate::domain::effects::{
    PendingBodyStates, PendingEffectStates, PendingSpiritSpheres, PendingVirtues,
    body_state_visuals, efst_auras, on_ground_skill, on_skill_damage, on_skill_effect,
    on_special_effect, option_visuals, spirit_sphere_visuals, virtue_visuals,
};
use crate::domain::system_sets::EntityLifecycleSystems;
use bevy::prelude::*;

/// Client skill/status handlers; the binary owns shared playback composition.
/// Content catalog loading is owned by `ContentCatalogPlugin`.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingBodyStates>()
            .init_resource::<PendingEffectStates>()
            .init_resource::<PendingVirtues>()
            .init_resource::<PendingSpiritSpheres>()
            .add_systems(
                Update,
                (
                    on_skill_effect,
                    on_skill_damage,
                    on_ground_skill,
                    on_special_effect,
                ),
            )
            .add_systems(
                Update,
                (
                    body_state_visuals.after(EntityLifecycleSystems::Spawning),
                    virtue_visuals.after(EntityLifecycleSystems::Spawning),
                    option_visuals.after(EntityLifecycleSystems::Spawning),
                    spirit_sphere_visuals.after(EntityLifecycleSystems::Spawning),
                    efst_auras.after(EntityLifecycleSystems::Spawning),
                ),
            );
    }
}
