//! Rendering and playback of offline-converted actors through standard glTF.

use super::{ActorAnimationInput, ActorVisualRoot, CharacterDirection};
use bevy::prelude::*;

mod animation;
mod systems;
#[cfg(test)]
mod tests;

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[require(Transform, Visibility, ActorAnimationInput, ActorVisualStatus)]
pub struct Gr2Actor {
    pub model: String,
}

/// Caller-resolved emblem image. An empty image keeps the model's default texture.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub struct ActorEmblem(pub Option<Handle<Image>>);

/// Loading outcome for an actor instance; callers choose their own failure policy.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub enum ActorVisualStatus {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

pub(crate) fn register(app: &mut App) {
    use crate::rendering::SpriteRenderingSystems;
    if !app.world().contains_resource::<Assets<bevy::gltf::Gltf>>() {
        app.init_asset::<bevy::gltf::Gltf>();
    }
    if !app.world().contains_resource::<Assets<AnimationClip>>() {
        app.init_asset::<AnimationClip>();
    }
    if !app.world().contains_resource::<Assets<AnimationGraph>>() {
        app.init_asset::<AnimationGraph>();
    }
    if !app
        .world()
        .contains_resource::<Assets<bevy::world_serialization::WorldAsset>>()
    {
        app.init_asset::<bevy::world_serialization::WorldAsset>();
    }
    app.add_systems(
        Update,
        (
            systems::sync_model_scenes,
            systems::detect_load_failure,
            systems::wire_scenes,
        )
            .chain()
            .in_set(SpriteRenderingSystems::HierarchySpawn),
    )
    .add_systems(
        Update,
        (systems::sync_facing, animation::sync_animation)
            .chain()
            .in_set(SpriteRenderingSystems::AnimationSync),
    )
    .add_systems(
        Update,
        systems::sync_emblems.in_set(SpriteRenderingSystems::TransformUpdate),
    );
}

/// Low-level CPU fixture hooks, enabled only by downstream test dependencies.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    pub use super::animation::{ActorPlayback, ModelClip, sync_animation};
    pub use super::systems::{
        ModelScene, ModelSceneLink, SceneReady, facing_rotation, sync_emblems, sync_model_scenes,
        wire_scenes,
    };
}
