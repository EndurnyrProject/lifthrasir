use crate::domain::entities::character::components::{CharacterInfo, Gender};
use bevy::prelude::*;

#[derive(Component, Clone)]
pub struct CharacterCard {
    pub slot: u8,
    pub character: Option<CharacterInfo>,
}

#[derive(Component)]
pub struct CharacterSpriteDisplay {
    pub slot: u8,
    pub sprite_layers: Vec<Entity>, // Entities for each sprite layer
}

#[derive(Component)]
pub struct CharacterNameLabel {
    pub slot: u8,
}

#[derive(Component)]
pub struct CharacterLevelLabel {
    pub slot: u8,
}

#[derive(Component)]
pub struct CharacterClassLabel {
    pub slot: u8,
}

#[derive(Component)]
pub struct CreateCharacterButton {
    pub slot: u8,
}

#[derive(Component)]
pub struct DeleteCharacterButton {
    pub character_id: u32,
}

#[derive(Component)]
pub struct SelectCharacterButton {
    pub slot: u8,
}

#[derive(Component)]
pub struct CharacterDetailsPanel;

#[derive(Component)]
pub struct CharacterStatsDisplay;

#[derive(Component)]
pub struct CharacterEquipmentDisplay;

#[derive(Component)]
pub struct CharacterSelectionScreen;

#[derive(Component)]
pub struct CharacterCreationScreen;

#[derive(Component)]
pub struct CharacterListUiRoot;

#[derive(Component)]
pub struct CharacterCreationUiRoot;

#[derive(Component, Default)]
pub struct CharacterCreationFormUI {
    pub name_input_entity: Option<Entity>,
    pub hair_style_selector: Option<Entity>,
    pub hair_color_selector: Option<Entity>,
    pub job_selector: Option<Entity>,
    pub stat_inputs: Option<Entity>,
    pub preview_entity: Option<Entity>,
}

#[derive(Component)]
pub struct CharacterPreview3D {
    pub slot: u8,
}

#[derive(Component)]
pub struct CharacterAnimationController {
    pub current_action: CharacterAction,
    pub animation_timer: Timer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterAction {
    Idle,
    Selected,
    Walk,
    Sit,
    Attack,
}

impl Default for CharacterAnimationController {
    fn default() -> Self {
        Self {
            current_action: CharacterAction::Idle,
            animation_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
        }
    }
}

// Marker components for UI elements
#[derive(Component)]
pub struct CharacterSlotGrid;

#[derive(Component)]
pub struct CharacterSlot {
    pub index: u8,
}

#[derive(Component)]
pub struct BackToServerSelectionButton;

#[derive(Component)]
pub struct EnterGameButton;

#[derive(Component)]
pub struct CreateCharacterSubmitButton;

#[derive(Component)]
pub struct CancelCharacterCreationButton;

#[derive(Component)]
pub struct CharacterCreationBackButton;

#[derive(Component)]
pub struct CharacterNameInput {
    pub current_text: String,
    pub cursor_position: usize,
    pub max_length: usize,
    pub is_focused: bool,
}

impl Default for CharacterNameInput {
    fn default() -> Self {
        Self {
            current_text: String::new(),
            cursor_position: 0,
            max_length: 23,
            is_focused: false,
        }
    }
}

#[derive(Component)]
pub struct CharacterPreviewContainer;

#[derive(Component)]
pub struct CharacterCreationFormSection;

#[derive(Component)]
pub struct CharacterCreationPreviewSection;

#[derive(Component)]
pub struct ValidationErrorDisplay;

#[derive(Component)]
pub struct GenderToggleButton {
    pub gender: Gender,
    pub is_selected: bool,
}

impl GenderToggleButton {
    pub fn new(gender: Gender) -> Self {
        Self {
            gender,
            is_selected: false,
        }
    }

    pub fn male() -> Self {
        Self::new(Gender::Male)
    }

    pub fn female() -> Self {
        Self::new(Gender::Female)
    }
}

#[derive(Component)]
pub struct GenderSelectionContainer;

#[derive(Component)]
pub struct HairStyleButton {
    pub style_id: u16,
    pub is_selected: bool,
}

impl HairStyleButton {
    pub fn new(style_id: u16) -> Self {
        Self {
            style_id,
            is_selected: false,
        }
    }
}

#[derive(Component)]
pub struct HairColorButton {
    pub color_id: u16,
    pub is_selected: bool,
}

impl HairColorButton {
    pub fn new(color_id: u16) -> Self {
        Self {
            color_id,
            is_selected: false,
        }
    }
}

#[derive(Component)]
pub struct HairStyleSelectionContainer;

#[derive(Component)]
pub struct HairColorSelectionContainer;

#[derive(Component)]
pub struct HairStyleScrollContainer;

#[derive(Component)]
pub struct HairColorScrollContainer;

#[derive(Component)]
pub struct HairStyleGrid;

#[derive(Component)]
pub struct HairColorGrid;

#[derive(Resource, Default)]
pub struct CharacterSelectionState {
    pub selected_slot: Option<u8>,
    pub hovering_slot: Option<u8>,
    pub is_creating_character: bool,
    pub creation_slot: Option<u8>,
}

#[derive(Resource)]
pub struct MapLoadingTimer {
    pub started: std::time::Instant,
    pub map_name: String,
}
