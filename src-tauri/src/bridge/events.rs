use bevy::prelude::*;
use secrecy::SecretString;

#[derive(Message)]
pub struct LoginRequestedEvent {
    pub username: String,
    pub password: SecretString,
}

#[derive(Message)]
pub struct ServerSelectionRequestedEvent {
    pub server_index: usize,
}

#[derive(Message)]
pub struct GetCharacterListRequestedEvent {}

#[derive(Message)]
pub struct SelectCharacterRequestedEvent {
    pub slot: u8,
}

#[derive(Message)]
pub struct CreateCharacterRequestedEvent {
    pub name: String,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub sex: u8,
}

#[derive(Message)]
pub struct DeleteCharacterRequestedEvent {
    pub char_id: u32,
}

#[derive(Message)]
pub struct GetHairstylesRequestedEvent {
    pub gender: u8,
}

#[derive(Message)]
pub struct KeyboardInputEvent {
    pub code: String,
    pub pressed: bool,
}

#[derive(Message)]
pub struct MousePositionEvent {
    pub x: f32,
    pub y: f32,
}

#[derive(Message)]
pub struct MouseClickEvent {
    pub x: f32,
    pub y: f32,
}
