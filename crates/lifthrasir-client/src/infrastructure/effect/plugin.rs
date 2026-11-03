use crate::core::GameState;
use crate::domain::effects::{
    PendingAilments, PendingBodyStates, PendingEffectStates, PendingSpiritSpheres, PendingVirtues,
    ailment_visuals, body_state_visuals, clear_pending_ailments, efst_auras, on_ground_skill,
    on_performance_skill, on_skill_damage, on_skill_effect, on_special_effect, option_visuals,
    spirit_sphere_visuals, sync_curse_direction, virtue_visuals,
};
use crate::domain::system_sets::EntityLifecycleSystems;
use bevy::prelude::*;

/// Client skill/status handlers; the binary owns shared playback composition.
/// Content catalog loading is owned by `ContentCatalogPlugin`.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingAilments>()
            .init_resource::<PendingBodyStates>()
            .init_resource::<PendingEffectStates>()
            .init_resource::<PendingVirtues>()
            .init_resource::<PendingSpiritSpheres>()
            .add_systems(OnExit(GameState::InGame), clear_pending_ailments)
            .add_systems(
                Update,
                (
                    on_skill_effect,
                    on_performance_skill
                        .before(lifthrasir_runtime::audio::controlled_sfx::control_skill_sfx),
                    on_skill_damage,
                    on_ground_skill,
                    on_special_effect,
                ),
            )
            .add_systems(
                Update,
                (
                    ailment_visuals
                        .after(EntityLifecycleSystems::Spawning)
                        .before(lifthrasir_runtime::effects::sprites::spawn_effect_sprites)
                        .before(lifthrasir_runtime::effects::overlays::apply_body_state_tint),
                    sync_curse_direction
                        .after(ailment_visuals)
                        .after(lifthrasir_runtime::rendering::SpriteRenderingSystems::AnimationSync)
                        .before(lifthrasir_runtime::effects::sprites::spawn_effect_sprites),
                    body_state_visuals.after(EntityLifecycleSystems::Spawning),
                    virtue_visuals.after(EntityLifecycleSystems::Spawning),
                    option_visuals.after(EntityLifecycleSystems::Spawning),
                    spirit_sphere_visuals.after(EntityLifecycleSystems::Spawning),
                    efst_auras.after(EntityLifecycleSystems::Spawning),
                ),
            );
    }
}
