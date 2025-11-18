use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use crate::core::state::GameState;

// =============================================================================
// INPUT PROCESSING SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::InputPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
pub enum InputSystems {
    Raycast,
    Cursor,
    Click,
}

// =============================================================================
// ENTITY LIFECYCLE SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::EntitySpawningPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
pub enum EntityLifecycleSystems {
    Vanishing,
    Spawning,
    Despawning,
}

// =============================================================================
// MOVEMENT SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::MovementPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
pub enum MovementSystems {
    Confirm,
    Interpolate,
    Stop,
    TerrainAlignment,
}

// =============================================================================
// SPRITE RENDERING SYSTEMS (CRITICAL)
// =============================================================================

/// Sprite rendering and animation pipeline
/// This is the CRITICAL rendering pipeline - all sprite systems depend on this ordering
/// The `chain` attribute ensures variants execute in the exact order listed below
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::domain::entities::sprite_rendering::GenericSpriteRenderingPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
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

// =============================================================================
// ENTITY INTERACTION SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::EntityHoverPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
pub enum EntityInteractionSystems {
    Hover,
    Naming,
}

// =============================================================================
// CAMERA SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::LifthrasirPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::InGame))
)]
pub enum CameraSystems {
    TargetUpdate,
    Follow,
}

// =============================================================================
// CHARACTER FLOW SYSTEMS
// =============================================================================

/// Character flow systems (login → character selection → zone entry)
/// This is a long chain of 14 systems that handle the entire character flow
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::CharacterDomainPlugin,
    schedule = Update,
    chain
)]
pub enum CharacterFlowSystems {
    CharServerPing,
    CharServerConnection,
    CharacterList,
    CharacterSelection,
    CharacterCreation,
    CharacterDeletion,
    ZoneServerInfo,
    ZoneConnection,
    ZoneEntry,
    MapLoadStart,
    MapLoadTimeout,
    MapLoadDetect,
    MapLoadComplete,
    ActorInit,
}

// =============================================================================
// WORLD LOADING SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::WorldPlugin,
    schedule = Update,
    chain,
    config(run_if = in_state(GameState::Loading))
)]
pub enum WorldLoadingSystems {
    StateMonitoring,
    LoaderSetup,
    AssetExtraction,
    AssetFailureDetection,
    TerrainMeshGeneration,
    TerrainTextureApplication,
}

// =============================================================================
// AUTHENTICATION SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::AuthenticationPlugin,
    schedule = Update,
    chain
)]
pub enum AuthenticationSystems {
    ConfigLoading,
    LoginAttempt,
    LoginResponse,
    LoginClientUpdate,
    ServerSelection,
}

// =============================================================================
// RENDERING SYSTEMS
// =============================================================================

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::plugins::rendering_plugin::RenderingPlugin,
    schedule = Update,
    chain
)]
pub enum ModelRenderingSystems {
    ModelLoading,
    ModelMeshUpdate,
    ModelMaterialUpdate,
    ModelAnimation,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::plugins::rendering_plugin::RenderingPlugin,
    schedule = Update,
    chain
)]
pub enum WaterRenderingSystems {
    WaterLoading,
    WaterFinalization,
    WaterAnimation,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::plugins::rendering_plugin::RenderingPlugin,
    schedule = Update
)]
pub enum MiscRenderingSystems {
    LightingSetup,
    LightingCleanup,
    BillboardUpdate,
}
