use bevy::prelude::*;

/// Shared presentation phases, ordered by RuntimeCorePlugin without client state gates.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpriteRenderingSystems {
    HierarchySpawn,
    AssetPopulation,
    AnimationEvents,
    AnimationSync,
    AnimationMarkers,
    AnimationAdvance,
    TransformUpdate,
    AnimationPlayback,
    OrphanCleanup,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaterRenderingSystems {
    WaterLoading,
    WaterFinalization,
    WaterAnimation,
}
