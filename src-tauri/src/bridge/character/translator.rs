use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::domain::character::{
    CharacterCreationForm, CreateCharacterRequestEvent, DeleteCharacterRequestEvent,
    RequestCharacterListEvent, SelectCharacterEvent,
};
use game_engine::domain::entities::character::components::Gender;

use crate::bridge::events::{
    CreateCharacterRequestedEvent, DeleteCharacterRequestedEvent, GetCharacterListRequestedEvent,
    SelectCharacterRequestedEvent,
};
use crate::plugin::TauriSystems;

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers)
)]
pub fn handle_get_character_list_request(
    mut events: MessageReader<GetCharacterListRequestedEvent>,
    mut char_list_events: MessageWriter<RequestCharacterListEvent>,
) {
    for _event in events.read() {
        debug!("Processing GetCharacterListRequestedEvent");

        char_list_events.write(RequestCharacterListEvent);
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers)
)]
pub fn handle_select_character_request(
    mut events: MessageReader<SelectCharacterRequestedEvent>,
    mut select_char_events: MessageWriter<SelectCharacterEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing SelectCharacterRequestedEvent, slot: {}",
            event.slot
        );

        select_char_events.write(SelectCharacterEvent { slot: event.slot });
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers)
)]
pub fn handle_create_character_request(
    mut events: MessageReader<CreateCharacterRequestedEvent>,
    mut create_char_events: MessageWriter<CreateCharacterRequestEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing CreateCharacterRequestedEvent, name: {}, slot: {}",
            event.name, event.slot
        );

        let form = CharacterCreationForm {
            name: event.name.clone(),
            slot: event.slot,
            hair_style: event.hair_style,
            hair_color: event.hair_color,
            starting_job: 0, // JT_NOVICE
            sex: Gender::from(event.sex),
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
        };

        create_char_events.write(CreateCharacterRequestEvent { form });
    }
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers)
)]
pub fn handle_delete_character_request(
    mut events: MessageReader<DeleteCharacterRequestedEvent>,
    mut delete_char_events: MessageWriter<DeleteCharacterRequestEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing DeleteCharacterRequestedEvent, char_id: {}",
            event.char_id
        );

        delete_char_events.write(DeleteCharacterRequestEvent {
            character_id: event.char_id,
        });
    }
}
