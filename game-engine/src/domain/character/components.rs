use crate::domain::entities::character::components::Gender;
use bevy::prelude::*;

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
