pub mod components;
pub mod events;
pub mod kinds;
pub mod states;
pub mod systems;
pub mod visual;

use bevy_auto_plugin::prelude::{auto_plugin, AutoPlugin};

use crate::domain::combat::components::Combatant;
use crate::domain::entities::character::states::{AnimationState, StatusEffects};
use crate::domain::entities::movement;

pub use events::SpawnCharacterSpriteEvent;

/// Unified Character Entity Plugin
///
/// Handles unified character entity system including:
/// - Entity registry for multi-entity support
/// - Character state machines (Animation, Gameplay, Context states)
/// - Character sprite spawning and event forwarding
/// - Visual updates (direction changes)
///
/// Sub-plugins (BehaviorPlugin, GenericSpriteRenderingPlugin) are registered
/// by CharacterDomainPlugin before adding this plugin.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct UnifiedCharacterEntityPlugin;

/// Add gameplay components to an existing character entity
///
/// This helper adds all components required for in-game character functionality:
/// - Movement system components (speed, state)
/// - State machine for animation transitions
/// - Initial animation, gameplay, and context states
/// - Grounded marker for automatic terrain following
///
/// Use this when transitioning a character entity from character selection to in-game.
///
/// # Example
/// ```ignore
/// add_gameplay_components_to_entity(&mut commands.entity(character_entity));
/// ```
pub fn add_gameplay_components_to_entity(commands: &mut bevy::ecs::system::EntityCommands) {
    commands.insert((
        // Movement components
        movement::components::MovementSpeed::default_walk(),
        movement::components::MovementState::Idle,
        // Animation state (moonshine-behavior)
        AnimationState::Idle,
        StatusEffects::default(),
        // Combat component (required for attack animations)
        Combatant::new(150),
        // Terrain following
        components::core::Grounded,
    ));
}
