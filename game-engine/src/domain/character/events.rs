use super::forms::CharacterCreationForm;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::dto::CharacterInfo as ProtocolCharacterInfo;

#[derive(Debug, Clone)]
pub struct CharacterInfoWithJobName {
    pub base: ProtocolCharacterInfo,
    pub job_name: String,
    pub body_sprite_path: String,
    pub hair_sprite_path: String,
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
pub struct CreateCharacterRequestEvent {
    pub form: CharacterCreationForm,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterCreatedEvent;

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterCreationFailedEvent {
    pub error: String,
}

#[derive(Message, Debug)]
#[auto_add_message(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct DeleteCharacterRequestEvent {
    pub character_id: u32,
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
