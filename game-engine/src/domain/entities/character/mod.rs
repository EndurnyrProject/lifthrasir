pub mod components;
pub mod events;
pub mod kinds;
pub mod states;
pub mod systems;
pub mod visual;

use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_plugin, AutoPlugin};

use crate::domain::entities::character::states::{AnimationState, ContextState, GameplayState};
use crate::domain::entities::movement;

pub use events::SpawnCharacterSpriteEvent;

// Type alias for querying unified character entities
type UnifiedCharacterFilter = (
    With<components::CharacterData>,
    With<components::CharacterAppearance>,
    With<components::EquipmentSet>,
);

/// Unified Character Entity Plugin
///
/// Handles unified character entity system including:
/// - Entity registry for multi-entity support
/// - Character state machines (Animation, Gameplay, Context states)
/// - Character sprite spawning and event forwarding
/// - Visual updates (direction changes)
///
/// Sub-plugins (StateMachinePlugin, GenericSpriteRenderingPlugin) are registered
/// by CharacterDomainPlugin before adding this plugin.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct UnifiedCharacterEntityPlugin;

// Helper function to create a unified character entity
pub fn spawn_unified_character(
    commands: &mut Commands,
    character_data: components::CharacterData,
    appearance: components::CharacterAppearance,
    equipment: components::EquipmentSet,
    position: Vec3,
) -> Entity {
    let character_entity = commands
        .spawn((
            // Core components
            character_data,
            appearance,
            equipment,
            // Visual components
            components::visual::CharacterSprite::default(),
            components::visual::CharacterDirection::default(),
            // Unified state machine component
            states::create_animation_state_machine(),
            // Initial state components (required by StateMachine)
            AnimationState::Idle,
            GameplayState::Normal,
            ContextState::CharacterSelection,
            // Transform components
            Transform::from_translation(position),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            // Name for debugging
            Name::new("UnifiedCharacter"),
        ))
        .id();

    character_entity
}

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
        // State machine for animation transitions
        states::create_animation_state_machine(),
        // Initial states (required by StateMachine)
        AnimationState::Idle,
        GameplayState::Normal,
        ContextState::InGame,
        // Terrain following
        components::core::Grounded,
    ));
}

// Helper to check if an entity has the unified character components
pub fn is_unified_character(
    entity: Entity,
    characters: &Query<(), UnifiedCharacterFilter>,
) -> bool {
    characters.get(entity).is_ok()
}
