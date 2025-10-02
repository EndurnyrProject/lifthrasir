pub mod components;
pub mod kinds;
pub mod sprite_hierarchy;
pub mod states;

use bevy::prelude::*;

use crate::domain::entities::character::states::{AnimationState, ContextState, GameplayState};

// Plugin that sets up the unified character entity system
pub struct UnifiedCharacterEntityPlugin;

impl Plugin for UnifiedCharacterEntityPlugin {
    fn build(&self, app: &mut App) {
        // Setup character state machines
        states::setup_character_state_machines(app);

        app
            // Add sprite hierarchy management
            .add_plugins(sprite_hierarchy::CharacterSpriteHierarchyPlugin)
            // Add trigger insertion systems
            .add_systems(
                Update,
                (
                    states::insert_animation_triggers_from_gameplay_changes,
                    states::cleanup_processed_triggers,
                )
                    .chain(),
            );
    }
}

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

// Conversion function from old character model to new unified system
pub fn convert_legacy_character_to_unified(
    commands: &mut Commands,
    legacy_character: &crate::domain::character::models::CharacterData,
    slot: u8,
    position: Vec3,
) -> Entity {
    let character_data = components::CharacterData::from(legacy_character.clone());
    let mut character_data = character_data;
    character_data.slot = slot;

    let appearance = components::CharacterAppearance::from(legacy_character.clone());
    let equipment = components::EquipmentSet::from(legacy_character);

    spawn_unified_character(commands, character_data, appearance, equipment, position)
}

// Helper to check if an entity has the unified character components
pub fn is_unified_character(
    entity: Entity,
    characters: &Query<
        (),
        (
            With<components::CharacterData>,
            With<components::CharacterAppearance>,
            With<components::EquipmentSet>,
        ),
    >,
) -> bool {
    characters.get(entity).is_ok()
}
