use super::animation_player::RoAnimationPlayer;
use super::markers::Animated;
use crate::domain::system_sets::SpriteRenderingSystems;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Automatically add Animated marker to entities with RoAnimationPlayer
/// This ensures animated entities are separated into their own archetype
/// for optimal query performance
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationMarkers)
)]
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
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationMarkers)
)]
pub fn remove_animated_marker(
    mut commands: Commands,
    query: Query<Entity, (With<Animated>, Without<RoAnimationPlayer>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).remove::<Animated>();
    }
}
