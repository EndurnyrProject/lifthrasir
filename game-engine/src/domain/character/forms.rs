use crate::domain::entities::character::components::Gender;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCreationForm {
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub starting_job: u16,
    pub sex: Gender,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
}

impl Default for CharacterCreationForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            slot: 0,
            hair_style: 1,
            hair_color: 0,
            starting_job: 0,
            sex: Gender::Male,
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
        }
    }
}

impl CharacterCreationForm {
    pub fn validate(&self) -> Result<(), CharacterCreationError> {
        if self.name.is_empty() {
            return Err(CharacterCreationError::NameEmpty);
        }
        if self.name.len() < 4 {
            return Err(CharacterCreationError::NameTooShort);
        }
        if self.name.len() > 23 {
            return Err(CharacterCreationError::NameTooLong);
        }
        if !self.name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(CharacterCreationError::NameInvalidCharacters);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CharacterCreationError {
    #[error("Character name cannot be empty")]
    NameEmpty,
    #[error("Character name must be at least 4 characters")]
    NameTooShort,
    #[error("Character name cannot exceed 23 characters")]
    NameTooLong,
    #[error("Character name can only contain letters, numbers, and underscores")]
    NameInvalidCharacters,
    #[error("Character name contains forbidden words")]
    NameForbidden,
    #[error("Invalid stat distribution")]
    InvalidStats,
    #[error("Server error: {0}")]
    ServerError(String),
}
