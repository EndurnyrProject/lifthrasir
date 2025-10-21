use bevy::prelude::Component;

/// Marker for the entity controlled by the local player
///
/// Only ONE entity in the world should have this marker.
/// Used by camera system and input systems.
#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayer;

/// Marker for other players' entities
#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayer;

/// Marker for Non-Player Character entities
#[derive(Component, Debug, Clone, Copy)]
pub struct Npc;

/// Marker for Monster/mob entities
#[derive(Component, Debug, Clone, Copy)]
pub struct Mob;

/// Marker for Homunculus entities (player's summoned creature)
#[derive(Component, Debug, Clone, Copy)]
pub struct Homunculus;

/// Marker for Mercenary entities (hired fighter)
#[derive(Component, Debug, Clone, Copy)]
pub struct Mercenary;

/// Marker for Elemental entities (summoned element)
#[derive(Component, Debug, Clone, Copy)]
pub struct Elemental;
