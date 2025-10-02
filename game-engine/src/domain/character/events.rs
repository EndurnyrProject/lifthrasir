use super::models::{CharacterCreationError, CharacterCreationForm, CharacterData};
use bevy::prelude::*;

#[derive(Event, Debug)]
pub struct RequestCharacterListEvent;

#[derive(Event, Debug)]
pub struct CharacterListReceivedEvent {
    pub characters: Vec<Option<CharacterData>>,
    pub max_slots: u8,
    pub available_slots: u8,
}

#[derive(Event, Debug)]
pub struct SelectCharacterEvent {
    pub slot: u8,
}

#[derive(Event, Debug)]
pub struct CharacterSelectedEvent {
    pub character: CharacterData,
    pub slot: u8,
}

#[derive(Event, Debug)]
pub struct EnterGameRequestEvent {
    pub character_id: u32,
}

#[derive(Event, Debug)]
pub struct ZoneServerInfoReceivedEvent {
    pub char_id: u32,
    pub map_name: String,
    pub server_ip: String,
    pub server_port: u16,
}

#[derive(Event, Debug)]
pub struct CreateCharacterRequestEvent {
    pub form: CharacterCreationForm,
}

#[derive(Event, Debug)]
pub struct CharacterCreatedEvent {
    pub character: CharacterData,
    pub slot: u8,
}

#[derive(Event, Debug)]
pub struct CharacterCreationFailedEvent {
    pub slot: u8,
    pub error: String,
}

#[derive(Event, Debug)]
pub struct DeleteCharacterRequestEvent {
    pub character_id: u32,
}

#[derive(Event, Debug)]
pub struct CharacterDeletedEvent {
    pub character_id: u32,
}

#[derive(Event, Debug)]
pub struct CharacterDeletionFailedEvent {
    pub character_id: u32,
    pub error: String,
}

#[derive(Event, Debug)]
pub struct OpenCharacterCreationEvent {
    pub slot: u8,
}

#[derive(Event, Debug)]
pub struct CloseCharacterCreationEvent;

#[derive(Event, Debug)]
pub struct CharacterHoverEvent {
    pub slot: Option<u8>,
}

#[derive(Event, Debug)]
pub struct RefreshCharacterListEvent;
