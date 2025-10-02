use super::{Gender, JobClass};
use crate::domain::entities::character::{
    components::{CharacterAppearance, CharacterData, CharacterStats, EquipmentSet},
    spawn_unified_character,
    sprite_hierarchy::SpawnCharacterSpriteEvent,
};
use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct CharacterCreationPreview;

#[derive(Resource, Debug, Clone)]
pub struct CharacterCreationState {
    pub gender: Gender,
    pub hair_style: u16,
    pub hair_color: u16,
    pub preview_entity: Option<Entity>,
}

impl Default for CharacterCreationState {
    fn default() -> Self {
        Self {
            gender: Gender::Male,
            hair_style: 1, // First hairstyle
            hair_color: 0, // Default color
            preview_entity: None,
        }
    }
}

#[derive(Event, Debug)]
pub struct UpdateCharacterPreviewEvent {
    pub gender: Gender,
    pub hair_style: u16,
    pub hair_color: u16,
}

#[derive(Component)]
pub struct CharacterCreationCamera;

fn get_preview_position(window: &Window) -> Vec3 {
    let x = -window.width() * 0.25;
    let y = 0.0;
    Vec3::new(x, y, 0.0)
}

pub fn setup_character_creation_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("CharacterCreation2DCamera"),
        CharacterCreationCamera,
        Camera2d,
    ));
}

pub fn handle_enter_character_creation(
    mut commands: Commands,
    mut creation_state: ResMut<CharacterCreationState>,
    windows: Query<&Window>,
    existing_preview: Query<Entity, With<CharacterCreationPreview>>,
) {
    if let Some(existing_entity) = creation_state.preview_entity {
        if existing_preview.get(existing_entity).is_ok() {
            return;
        } else {
            creation_state.preview_entity = None;
        }
    }

    let Ok(window) = windows.single() else {
        warn!("No window found for character creation preview");
        return;
    };

    let spawn_position = get_preview_position(window);

    let character_data = CharacterData {
        name: "Preview".to_string(),
        job_id: JobClass::Novice as u16,
        level: 1,
        experience: 0,
        stats: CharacterStats::default(),
        slot: 0,
    };

    let appearance = CharacterAppearance {
        gender: creation_state.gender,
        hair_style: creation_state.hair_style,
        hair_color: creation_state.hair_color,
        clothes_color: 0,
    };

    let equipment = EquipmentSet::default();

    let preview_character_entity = spawn_unified_character(
        &mut commands,
        character_data,
        appearance,
        equipment,
        spawn_position,
    );

    commands
        .entity(preview_character_entity)
        .insert(CharacterCreationPreview);

    creation_state.preview_entity = Some(preview_character_entity);

    commands.queue(move |world: &mut World| {
        world.send_event(SpawnCharacterSpriteEvent {
            character_entity: preview_character_entity,
            spawn_position,
        });
    });
}

/// System to handle updating character preview appearance
pub fn handle_update_character_preview(
    mut events: EventReader<UpdateCharacterPreviewEvent>,
    mut creation_state: ResMut<CharacterCreationState>,
    mut preview_query: Query<&mut CharacterAppearance, With<CharacterCreationPreview>>,
) {
    for event in events.read() {
        creation_state.gender = event.gender;
        creation_state.hair_style = event.hair_style;
        creation_state.hair_color = event.hair_color;

        if let Some(preview_entity) = creation_state.preview_entity {
            if let Ok(mut appearance) = preview_query.get_mut(preview_entity) {
                appearance.gender = event.gender;
                appearance.hair_style = event.hair_style;
                appearance.hair_color = event.hair_color;
            }
        }
    }
}

pub fn update_preview_position_on_window_resize(
    mut preview_query: Query<&mut Transform, With<CharacterCreationPreview>>,
    windows: Query<&Window, Changed<Window>>,
) {
    if let Ok(window) = windows.single() {
        for mut transform in preview_query.iter_mut() {
            let new_position = get_preview_position(window);
            transform.translation = new_position;
        }
    }
}

pub fn cleanup_character_creation_preview(
    mut commands: Commands,
    preview_query: Query<Entity, With<CharacterCreationPreview>>,
    camera_query: Query<Entity, With<CharacterCreationCamera>>,
    mut creation_state: ResMut<CharacterCreationState>,
) {
    for entity in preview_query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }

    *creation_state = CharacterCreationState::default();
}
