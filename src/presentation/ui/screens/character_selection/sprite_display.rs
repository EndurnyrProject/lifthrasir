use crate::domain::character::rendering::{CharacterSelectionSprite, CharacterSpriteContainer};
use crate::domain::character::{CharacterCard, CharacterSlot};
use bevy::prelude::*;

/// Simple system to spawn character selection sprites when character cards are updated
pub fn spawn_character_selection_sprites(
    mut commands: Commands,
    character_cards: Query<(Entity, &CharacterCard, &CharacterSlot), Changed<CharacterCard>>,
    sprite_containers: Query<
        (Entity, &CharacterSpriteContainer),
        Without<CharacterSelectionSprite>,
    >,
) {
    for (_card_entity, card, slot) in character_cards.iter() {
        if let Some(character) = &card.character {
            info!(
                "Creating character selection sprite for: {} in slot {}",
                character.name, slot.index
            );

            // Find the corresponding sprite container
            if let Some((container_entity, _)) =
                sprite_containers.iter().find(|(_, c)| c.slot == slot.index)
            {
                // Spawn a simple character selection sprite entity
                let character_sprite = CharacterSelectionSprite::new(character.clone());

                let character_entity = commands
                    .spawn((
                        Name::new(format!("CharacterSprite_{}", character.name)),
                        character_sprite,
                        Transform::default(),
                        GlobalTransform::default(),
                        Visibility::default(),
                    ))
                    .id();

                // Add the CharacterCard component separately
                commands.entity(character_entity).insert((*card).clone());

                // Parent the character sprite to the UI container
                commands
                    .entity(container_entity)
                    .add_child(character_entity);

                info!("Spawned character selection sprite for: {}", character.name);
            }
        }
    }
}

// Re-export legacy functions for backward compatibility

/// Update character selection states based on UI hover/selection
pub fn update_character_selection_states(
    selection: Res<super::list::CharacterSelectionResource>,
    mut character_sprites: Query<(&mut CharacterSelectionSprite, &CharacterCard)>,
) {
    for (mut sprite, card) in character_sprites.iter_mut() {
        if let Some(character) = &card.character {
            // Handle hover effects
            let is_hovered = selection.hovering_slot == Some(card.slot);
            let is_selected = selection
                .selected_character
                .as_ref()
                .map(|selected| selected.char_id == character.char_id)
                .unwrap_or(false);

            // Update animation based on state
            if is_hovered && !is_selected {
                sprite.start_hover_animation();
            } else if !is_hovered {
                sprite.stop_hover_animation();
            }
        }
    }
}
