use super::animation_player::RoAnimationPlayer;
use super::markers::Animated;
use crate::core::state::GameState;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Automatically add Animated marker to entities with RoAnimationPlayer
/// This ensures animated entities are separated into their own archetype
/// for optimal query performance
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(
        after = crate::domain::entities::sprite_rendering::systems::update::update_generic_sprite_direction,
        run_if = in_state(GameState::InGame)
    )
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
    config(
        after = add_animated_marker,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn remove_animated_marker(
    mut commands: Commands,
    query: Query<Entity, (With<Animated>, Without<RoAnimationPlayer>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).remove::<Animated>();
    }
}
