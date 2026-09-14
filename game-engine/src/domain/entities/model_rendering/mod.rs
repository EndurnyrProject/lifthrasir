//! Rendering of offline-converted GR2 actors through standard glTF.

use bevy::prelude::*;

mod animation;
mod systems;

use crate::domain::{
    guild::{GuildSystems, emblems::GuildEmblemImages},
    system_sets::{CombatSystems, EntityLifecycleSystems, MovementSystems},
};

pub struct ModelRenderingPlugin;

impl Plugin for ModelRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildEmblemImages>()
            .add_systems(
                Update,
                (
                    systems::sync_model_scenes,
                    systems::detect_load_failure,
                    systems::wire_scenes,
                    systems::sync_facing,
                    animation::sync_animation,
                )
                    .chain()
                    .after(EntityLifecycleSystems::Spawning)
                    .after(MovementSystems::TerrainAlignment)
                    .after(CombatSystems::HandleDeath),
            )
            .add_systems(
                Update,
                systems::sync_emblems
                    .after(systems::wire_scenes)
                    .in_set(GuildSystems::UiSync),
            );
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Gr2Actor {
    pub model: String,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildFlag {
    pub guild_id: u32,
    pub emblem_id: u32,
}

#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod tests;
