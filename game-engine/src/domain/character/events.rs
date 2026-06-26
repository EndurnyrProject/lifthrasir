use super::forms::CharacterCreationForm;
use crate::domain::entities::character::components::CharacterInfo as DomainCharacterInfo;
use crate::infrastructure::networking::char_types::CharacterInfo as ProtocolCharacterInfo;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfoWithJobName {
    #[serde(flatten)]
    pub base: ProtocolCharacterInfo,
    pub job_name: String,
    pub body_sprite_path: String,
    pub hair_sprite_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hair_palette_path: Option<String>,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct RequestCharacterListEvent;

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterListReceivedEvent {
    pub characters: Vec<Option<CharacterInfoWithJobName>>,
    pub max_slots: u8,
    pub available_slots: u8,
    /// Character-select display pages (3 slots per page), from HC_CHARLIST_NOTIFY.
    pub display_pages: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct SelectCharacterEvent {
    pub slot: u8,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterSelectedEvent {
    pub character: DomainCharacterInfo,
    pub slot: u8,
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
    /// Single-use token to echo back in zone `SessionAuth.zone_auth_token`.
    pub zone_auth_token: Vec<u8>,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CreateCharacterRequestEvent {
    pub form: CharacterCreationForm,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterCreatedEvent {
    pub character: DomainCharacterInfo,
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
pub struct RefreshCharacterListEvent;

// Zone Server Connection Events

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
