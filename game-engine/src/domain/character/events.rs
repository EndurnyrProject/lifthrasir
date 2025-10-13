use super::forms::CharacterCreationForm;
use crate::domain::entities::character::components::CharacterInfo;
use bevy::prelude::*;

#[derive(Event, Debug)]
pub struct RequestCharacterListEvent;

#[derive(Event, Debug)]
pub struct CharacterListReceivedEvent {
    pub characters: Vec<Option<CharacterInfo>>,
    pub max_slots: u8,
    pub available_slots: u8,
}

#[derive(Event, Debug)]
pub struct SelectCharacterEvent {
    pub slot: u8,
}

#[derive(Event, Debug)]
pub struct CharacterSelectedEvent {
    pub character: CharacterInfo,
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
    pub account_id: u32,
    pub login_id1: u32,
    pub sex: u8,
}

#[derive(Event, Debug)]
pub struct CreateCharacterRequestEvent {
    pub form: CharacterCreationForm,
}

#[derive(Event, Debug)]
pub struct CharacterCreatedEvent {
    pub character: CharacterInfo,
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
pub struct CharacterHoverEvent {
    pub slot: Option<u8>,
}

#[derive(Event, Debug)]
pub struct RefreshCharacterListEvent;

// Zone Server Connection Events

#[derive(Event, Debug)]
pub struct ZoneServerConnected;

#[derive(Event, Debug)]
pub struct ZoneServerConnectionFailed {
    pub reason: String,
}

#[derive(Event, Debug)]
pub struct ZoneAuthenticationSuccess {
    pub spawn_x: u16,
    pub spawn_y: u16,
    pub spawn_dir: u8,
    pub server_tick: u32,
}

#[derive(Event, Debug)]
pub struct ZoneAuthenticationFailed {
    pub error_code: u8,
}

#[derive(Event, Debug)]
pub struct MapLoadingStarted {
    pub map_name: String,
}

#[derive(Event, Debug)]
pub struct MapLoadCompleted {
    pub map_name: String,
}

#[derive(Event, Debug)]
pub struct MapLoadingFailed {
    pub map_name: String,
    pub reason: String,
}

#[derive(Event, Debug)]
pub struct ActorInitSent;
