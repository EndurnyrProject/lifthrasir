use bevy::prelude::*;

// Component markers for different UI elements
#[derive(Component)]
pub struct LoginScreen;

#[derive(Component)]
pub struct CharacterSelectScreen;

#[derive(Component)]
pub struct UsernameInput;

#[derive(Component)]
pub struct PasswordInput;

#[derive(Component)]
pub struct RememberMeCheckbox;

#[derive(Component)]
pub struct LoginButton;

#[derive(Component)]
pub struct CharacterSlot {
    pub slot_id: u32,
    pub character_id: Option<u32>,
}

#[derive(Component)]
pub struct CreateCharacterButton;

#[derive(Component)]
pub struct DeleteCharacterButton;

#[derive(Component)]
pub struct EnterGameButton;

#[derive(Component)]
pub struct BackToLoginButton;

// Resources to hold form data
#[derive(Resource, Default)]
pub struct LoginFormData {
    pub username: String,
    pub password: String,
    pub remember_me: bool,
}

// Focus tracking component for input fields
#[derive(Component)]
pub struct FocusedInput;

// Character data structure
#[derive(Resource, Default)]
pub struct CharacterData {
    pub characters: Vec<Character>,
    pub selected_character: Option<u32>,
}

#[derive(Clone)]
pub struct Character {
    pub id: u32,
    pub name: String,
    pub level: u32,
    pub class: String,
    pub sprite_path: String,
}
