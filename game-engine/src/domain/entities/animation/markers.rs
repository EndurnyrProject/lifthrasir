use bevy::prelude::*;

/// Marker component for entities with active animations
/// Separates animated entities into their own archetype
/// for more efficient iteration
#[derive(Component, Debug, Default)]
pub struct Animated;

/// Marker component for entities with static sprites
/// These are never processed by animation systems
#[derive(Component, Debug, Default)]
pub struct StaticSprite;
