use super::animation_player::RoAnimationPlayer;
use super::markers::Animated;
use bevy::prelude::*;

/// Automatically add Animated marker to entities with RoAnimationPlayer
/// This ensures animated entities are separated into their own archetype
/// for optimal query performance
pub fn add_animated_marker(
    mut commands: Commands,
    query: Query<Entity, (With<RoAnimationPlayer>, Without<Animated>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).insert(Animated);
    }
}

/// Remove Animated marker when animation player is removed
/// Keeps archetypes clean and prevents stale markers
pub fn remove_animated_marker(
    mut commands: Commands,
    query: Query<Entity, (With<Animated>, Without<RoAnimationPlayer>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).remove::<Animated>();
    }
}
