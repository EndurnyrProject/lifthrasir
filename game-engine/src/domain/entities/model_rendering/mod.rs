//! Client policy around shared converted-model presentation.

use crate::domain::guild::{GuildSystems, emblems::GuildEmblemImages};
use crate::domain::system_sets::SpriteRenderingSystems;
use bevy::prelude::*;
pub use lifthrasir_runtime::actors::{ActorEmblem, ActorVisualStatus, Gr2Actor};

mod client;

pub struct ModelRenderingPlugin;
impl Plugin for ModelRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildEmblemImages>()
            .add_systems(
                Update,
                client::sync_actor_emblems
                    .in_set(GuildSystems::UiSync)
                    .after(SpriteRenderingSystems::HierarchySpawn)
                    .before(SpriteRenderingSystems::TransformUpdate),
            )
            .add_systems(
                Update,
                client::enforce_model_load_failures.after(SpriteRenderingSystems::TransformUpdate),
            );
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(ActorEmblem)]
pub struct GuildFlag {
    pub guild_id: u32,
    pub emblem_id: u32,
}

#[cfg(test)]
mod animation;
#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod systems;
#[cfg(test)]
mod tests;
