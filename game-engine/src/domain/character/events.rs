use super::forms::CharacterCreationForm;
use crate::domain::entities::character::components::CharacterInfo;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct RequestCharacterListEvent;

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterListReceivedEvent {
    pub characters: Vec<Option<CharacterInfo>>,
    pub max_slots: u8,
    pub available_slots: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct SelectCharacterEvent {
    pub slot: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterSelectedEvent {
    pub character: CharacterInfo,
    pub slot: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct EnterGameRequestEvent {
    pub character_id: u32,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ZoneServerInfoReceivedEvent {
    pub char_id: u32,
    pub map_name: String,
    pub server_ip: String,
    pub server_port: u16,
    pub account_id: u32,
    pub login_id1: u32,
    pub sex: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CreateCharacterRequestEvent {
    pub form: CharacterCreationForm,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterCreatedEvent {
    pub character: CharacterInfo,
    pub slot: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterCreationFailedEvent {
    pub slot: u8,
    pub error: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct DeleteCharacterRequestEvent {
    pub character_id: u32,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterDeletedEvent {
    pub character_id: u32,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterDeletionFailedEvent {
    pub character_id: u32,
    pub error: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterHoverEvent {
    pub slot: Option<u8>,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct RefreshCharacterListEvent;

// Zone Server Connection Events

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ZoneServerConnected;

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ZoneServerConnectionFailed {
    pub reason: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ZoneAuthenticationSuccess {
    pub spawn_x: u16,
    pub spawn_y: u16,
    pub spawn_dir: u8,
    pub server_tick: u32,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ZoneAuthenticationFailed {
    pub error_code: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct MapLoadingStarted {
    pub map_name: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct MapLoadCompleted {
    pub map_name: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct MapLoadingFailed {
    pub map_name: String,
    pub reason: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct ActorInitSent;
