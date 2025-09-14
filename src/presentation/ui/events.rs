use bevy::prelude::*;
use secrecy::SecretString;

#[derive(Event)]
pub struct LoginAttemptEvent {
    pub username: String,
    pub password: SecretString,
}

#[derive(Event)]
pub struct CharacterSelectEvent {
    pub character_id: u32,
}

#[derive(Event)]
pub struct CreateCharacterEvent;

#[derive(Event)]
pub struct DeleteCharacterEvent {
    pub character_id: u32,
}

#[derive(Event)]
pub struct BackToLoginEvent;
