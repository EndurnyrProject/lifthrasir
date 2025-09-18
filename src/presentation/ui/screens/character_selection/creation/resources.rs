use crate::domain::character::CharacterCreationForm;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CharacterCreationResource {
    pub is_active: bool,
    pub slot: Option<u8>,
    pub form: CharacterCreationForm,
    pub preview_entity: Option<Entity>,
    pub validation_errors: Vec<String>,
    pub available_hair_styles: Vec<u16>,
    pub available_hair_colors: Vec<u16>,
}

impl CharacterCreationResource {
    pub fn reset(&mut self) {
        self.is_active = false;
        self.slot = None;
        self.form = CharacterCreationForm::default();
        self.preview_entity = None;
        self.validation_errors.clear();
        self.available_hair_styles.clear();
        self.available_hair_colors.clear();
    }

    pub fn start_creation(&mut self, slot: u8) {
        self.is_active = true;
        self.slot = Some(slot);
        self.form = CharacterCreationForm {
            slot,
            ..Default::default()
        };
        self.validation_errors.clear();
    }
}
