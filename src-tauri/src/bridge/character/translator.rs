use bevy::prelude::*;
use game_engine::domain::character::{
    CharacterCreationForm, CreateCharacterRequestEvent, DeleteCharacterRequestEvent,
    RequestCharacterListEvent, SelectCharacterEvent,
};
use game_engine::domain::entities::character::components::{Gender, JobClass};

use crate::bridge::events::{
    CreateCharacterRequestedEvent, DeleteCharacterRequestedEvent, GetCharacterListRequestedEvent,
    SelectCharacterRequestedEvent,
};

/// System that handles GetCharacterListRequestedEvent
pub fn handle_get_character_list_request(
    mut events: EventReader<GetCharacterListRequestedEvent>,
    mut char_list_events: EventWriter<RequestCharacterListEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing GetCharacterListRequestedEvent, request_id: {}",
            event.request_id
        );

        char_list_events.write(RequestCharacterListEvent);
    }
}

/// System that handles SelectCharacterRequestedEvent
pub fn handle_select_character_request(
    mut events: EventReader<SelectCharacterRequestedEvent>,
    mut select_char_events: EventWriter<SelectCharacterEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing SelectCharacterRequestedEvent, slot: {}, request_id: {}",
            event.slot, event.request_id
        );

        select_char_events.write(SelectCharacterEvent { slot: event.slot });
    }
}

/// System that handles CreateCharacterRequestedEvent
pub fn handle_create_character_request(
    mut events: EventReader<CreateCharacterRequestedEvent>,
    mut create_char_events: EventWriter<CreateCharacterRequestEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing CreateCharacterRequestedEvent, name: {}, slot: {}, request_id: {}",
            event.name, event.slot, event.request_id
        );

        let form = CharacterCreationForm {
            name: event.name.clone(),
            slot: event.slot,
            hair_style: event.hair_style,
            hair_color: event.hair_color,
            starting_job: JobClass::Novice,
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

/// System that handles DeleteCharacterRequestedEvent
pub fn handle_delete_character_request(
    mut events: EventReader<DeleteCharacterRequestedEvent>,
    mut delete_char_events: EventWriter<DeleteCharacterRequestEvent>,
) {
    for event in events.read() {
        debug!(
            "Processing DeleteCharacterRequestedEvent, char_id: {}, request_id: {}",
            event.char_id, event.request_id
        );

        delete_char_events.write(DeleteCharacterRequestEvent {
            character_id: event.char_id,
        });
    }
}
