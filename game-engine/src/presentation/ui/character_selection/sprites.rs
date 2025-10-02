use crate::domain::character::events::CharacterListReceivedEvent;
use crate::domain::entities::character::sprite_hierarchy::SpawnCharacterSpriteEvent;
use crate::domain::entities::character::{components, convert_legacy_character_to_unified};
use crate::presentation::ui::character_selection::layout::slot_position;
use bevy::prelude::*;

/// Component to link UI containers to character sprite entities
#[derive(Component)]
pub struct CharacterSpriteContainer {
    pub slot: u8,
    pub character_entity: Option<Entity>,
}

impl CharacterSpriteContainer {
    pub fn new(slot: u8) -> Self {
        Self {
            slot,
            character_entity: None,
        }
    }
}

/// Marker component for character slot containers
#[derive(Component)]
pub struct CharacterSlotContainer {
    pub slot: u8,
}

/// System to spawn sprite containers for all character slots
pub fn setup_character_slot_containers(mut commands: Commands, windows: Query<&Window>) {
    let Ok(window) = windows.get_single() else {
        warn!("No window found for character slot container setup");
        return;
    };

    // Create containers for all 8 slots
    for slot in 0..8 {
        let position = slot_position(slot, window);

        info!("📍 Slot {} container at position: {:?}", slot, position);

        commands.spawn((
            Name::new(format!("CharacterSlotContainer_{}", slot)),
            CharacterSlotContainer { slot },
            CharacterSpriteContainer::new(slot),
            Transform::from_translation(position),
            GlobalTransform::default(),
            Visibility::default(),
            ViewVisibility::default(),
            InheritedVisibility::default(),
        ));
    }

    info!("📦 Created character slot containers for 8 slots");
}

/// System to spawn character sprites when character list is received
/// Uses the unified character entity system with proper sprite hierarchies
pub fn spawn_character_sprites_on_list_received(
    mut commands: Commands,
    mut char_list_events: EventReader<CharacterListReceivedEvent>,
    mut containers: Query<(Entity, &mut CharacterSpriteContainer, &Transform)>,
    existing_characters: Query<
        Entity,
        (
            With<components::CharacterData>,
            With<components::CharacterAppearance>,
        ),
    >,
) {
    // Only process the last event to avoid duplicates
    if let Some(event) = char_list_events.read().last() {
        info!(
            "🎮 CharacterListReceivedEvent received with {} slots",
            event.characters.len()
        );

        // Clear existing character entities
        for character_entity in existing_characters.iter() {
            commands.entity(character_entity).despawn();
        }

        info!(
            "✨ Spawning unified character entities for {} slots",
            event.characters.len()
        );

        // Spawn unified character entities for each slot
        for (slot, char_data_opt) in event.characters.iter().enumerate() {
            if let Some(character) = char_data_opt {
                // Find the container for this slot
                if let Some((container_entity, mut sprite_container, container_transform)) =
                    containers
                        .iter_mut()
                        .find(|(_, container, _)| container.slot == slot as u8)
                {
                    info!(
                        "👤 Creating unified character entity for: {} in slot {}",
                        character.name, slot
                    );

                    // Use container position as spawn position for character
                    let spawn_position = container_transform.translation;

                    // Create unified character entity using conversion helper
                    let character_entity = convert_legacy_character_to_unified(
                        &mut commands,
                        character,
                        slot as u8,
                        spawn_position,
                    );

                    // Store character entity reference in container
                    sprite_container.character_entity = Some(character_entity);

                    // Defer event emission until after commands are flushed
                    // This ensures the entity exists in queries when sprite hierarchy system runs
                    commands.queue(move |world: &mut World| {
                        world.send_event(SpawnCharacterSpriteEvent {
                            character_entity,
                            spawn_position,
                        });
                    });

                    info!(
                        "✅ Spawned unified character entity {:?} for: {} at position {:?}",
                        character_entity, character.name, spawn_position
                    );
                }
            }
        }
    }
}

/// System to update sprite positions when window resizes
pub fn update_sprite_positions_on_window_resize(
    mut containers: Query<(&CharacterSlotContainer, &mut Transform)>,
    windows: Query<&Window, Changed<Window>>,
) {
    if let Ok(window) = windows.get_single() {
        for (container, mut transform) in containers.iter_mut() {
            let new_position = slot_position(container.slot, window);
            transform.translation = new_position;
        }
    }
}

/// System to cleanup character selection entities when exiting CharacterSelection state
pub fn cleanup_character_selection(
    mut commands: Commands,
    character_entities: Query<
        Entity,
        (
            With<components::CharacterData>,
            With<components::CharacterAppearance>,
        ),
    >,
    slot_containers: Query<Entity, With<CharacterSlotContainer>>,
    camera_query: Query<Entity, With<super::CharacterSelectionCamera>>,
) {
    info!("🧹 Cleaning up character selection entities on state exit");

    // Despawn all character entities
    for entity in character_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
    info!(
        "🗑️ Despawned {} character entities",
        character_entities.iter().count()
    );

    // Despawn all slot containers
    for entity in slot_containers.iter() {
        commands.entity(entity).despawn_recursive();
    }
    info!(
        "🗑️ Despawned {} slot container entities",
        slot_containers.iter().count()
    );

    // Despawn camera
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
        info!("🗑️ Despawned character selection camera: {:?}", entity);
    }
}
