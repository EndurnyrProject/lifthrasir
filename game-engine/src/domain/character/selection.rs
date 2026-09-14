use super::events::*;
use crate::core::state::GameState;
use crate::domain::entities::character::components::{
    CharacterInfo,
    visual::{CharacterDirection, CharacterSprite},
};
use crate::domain::system_sets::CharacterFlowSystems;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected,
};

/// Domain-owned snapshot of the character-select roster.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin)]
pub struct DomainCharacterRoster {
    pub characters: Vec<net_contract::dto::CharacterInfo>,
    pub slot_info: net_contract::dto::CharacterSlotInfo,
    pub display_pages: u32,
}

fn build_character_list_event(
    roster: &DomainCharacterRoster,
    job_registry: Option<&JobSpriteRegistry>,
) -> CharacterListReceivedEvent {
    let received_slots = roster
        .characters
        .iter()
        .map(|character| usize::from(character.char_num) + 1)
        .max()
        .unwrap_or_default();
    let mut characters = vec![None; usize::from(roster.slot_info.valid_slots).max(received_slots)];

    for character in &roster.characters {
        let slot = character.char_num as usize;
        if slot >= characters.len() {
            continue;
        }

        let job_name = job_registry
            .and_then(|registry| registry.get_display_name(character.class as u32))
            .unwrap_or("Unknown")
            .to_string();
        let body_sprite_path = job_registry
            .and_then(|registry| {
                registry.get_body_sprite_path(character.class as u32, character.sex)
            })
            .unwrap_or_else(|| "data\\sprite\\인간족\\몸통\\남\\초보자_남.spr".to_string());
        let hair_sprite_path = job_registry
            .map(|registry| registry.get_hair_sprite_path(character.hair, character.sex))
            .unwrap_or_else(|| "data\\sprite\\인간족\\머리통\\남\\1_남.spr".to_string());
        let hair_palette_path = job_registry.and_then(|registry| {
            registry.get_hair_palette_path(character.hair, character.sex, character.hair_color)
        });

        characters[slot] = Some(CharacterInfoWithJobName {
            base: character.clone(),
            job_name,
            body_sprite_path,
            hair_sprite_path,
            hair_palette_path,
        });
    }

    CharacterListReceivedEvent {
        characters,
        slot_info: roster.slot_info.clone(),
        display_pages: roster.display_pages.min(u8::MAX as u32) as u8,
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterList)
)]
pub fn handle_request_character_list(
    mut requests: MessageReader<RequestCharacterListEvent>,
    roster: Res<DomainCharacterRoster>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut lists: MessageWriter<CharacterListReceivedEvent>,
) {
    for _ in requests.read() {
        lists.write(build_character_list_event(&roster, job_registry.as_deref()));
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharServerConnection)
)]
pub fn handle_character_server_connected(
    mut events: MessageReader<CharacterServerConnected>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for _ in events.read() {
        if *state.get() != GameState::CharacterSelection {
            next_state.set(GameState::CharacterSelection);
        }
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterList)
)]
pub fn handle_character_roster_changed(
    mut events: MessageReader<CharacterServerConnected>,
    mut roster: ResMut<DomainCharacterRoster>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut lists: MessageWriter<CharacterListReceivedEvent>,
) {
    for event in events.read() {
        roster.characters.clone_from(&event.characters);
        roster.slot_info.clone_from(&event.slot_info);
        roster.display_pages = event.display_pages;
        lists.write(build_character_list_event(&roster, job_registry.as_deref()));
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_created_protocol(
    mut protocol_events: MessageReader<CharacterCreated>,
    mut domain_events: MessageWriter<CharacterCreatedEvent>,
) {
    for _ in protocol_events.read() {
        domain_events.write(CharacterCreatedEvent);
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterDeletion)
)]
pub fn handle_character_deleted_protocol(
    mut protocol_events: MessageReader<CharacterDeleted>,
    mut refresh_events: MessageWriter<RefreshCharacterListEvent>,
) {
    for _ in protocol_events.read() {
        refresh_events.write(RefreshCharacterListEvent);
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterDeletion)
)]
pub fn handle_character_deletion_failed_protocol(
    mut protocol_events: MessageReader<CharacterDeletionFailed>,
    mut domain_events: MessageWriter<CharacterDeletionFailedEvent>,
) {
    for event in protocol_events.read() {
        domain_events.write(CharacterDeletionFailedEvent {
            character_id: event.char_id,
            error: event.error.description().to_string(),
        });
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_creation_failed_protocol(
    mut protocol_events: MessageReader<CharacterCreationFailed>,
    mut domain_events: MessageWriter<CharacterCreationFailedEvent>,
) {
    use net_contract::dto::CharCreationError;

    for event in protocol_events.read() {
        let error = match event.error {
            CharCreationError::NameExists => "Character name already exists",
            CharCreationError::InvalidName => "Invalid character name",
            CharCreationError::Unknown(_) => "Unknown error",
        };
        domain_events.write(CharacterCreationFailedEvent {
            error: error.to_string(),
        });
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterSelection)
)]
pub fn handle_select_character(
    mut events: MessageReader<SelectCharacterEvent>,
    roster: Res<DomainCharacterRoster>,
    mut commands: Commands,
) {
    for event in events.read() {
        let Some(character) = roster
            .characters
            .iter()
            .find(|character| character.char_num == event.slot)
            .cloned()
        else {
            error!("Character not found in slot {}", event.slot);
            continue;
        };

        let character = CharacterInfo::from(character);
        let (data, appearance, meta) = character.clone().into_components();
        commands.spawn((
            data,
            appearance,
            meta,
            CharacterSprite::default(),
            CharacterDirection::default(),
            Name::new(format!("Character_{}", character.char_id)),
        ));

        debug!(
            "Spawned character entity for char_id: {} ({})",
            character.char_id, character.name
        );
    }
}

#[auto_add_system(
    plugin = crate::domain::character::plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::CharacterCreation)
)]
pub fn handle_character_created(
    mut events: MessageReader<CharacterCreatedEvent>,
    mut refresh_events: MessageWriter<RefreshCharacterListEvent>,
) {
    for _ in events.read() {
        refresh_events.write(RefreshCharacterListEvent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto_character(char_num: u8, name: &str) -> net_contract::dto::CharacterInfo {
        net_contract::dto::CharacterInfo {
            char_id: 150001,
            class: 7,
            base_level: 42,
            job_level: 10,
            char_num,
            name: name.into(),
            last_map: "prontera".into(),
            ..Default::default()
        }
    }

    #[test]
    fn character_list_preserves_slots_and_server_metadata() {
        let slot_info = net_contract::dto::CharacterSlotInfo {
            normal_slots: 9,
            premium_slots: 3,
            billing_slots: 1,
            producible_slots: 10,
            valid_slots: 20,
        };
        let roster = DomainCharacterRoster {
            characters: vec![dto_character(17, "Vidar")],
            slot_info: slot_info.clone(),
            display_pages: 7,
        };

        let event = build_character_list_event(&roster, None);

        assert_eq!(event.slot_info, slot_info);
        assert_eq!(event.display_pages, 7);
        assert_eq!(event.characters.len(), 20);
        assert!(event.characters[0].is_none());
        let placed = event.characters[17]
            .as_ref()
            .expect("character should land in slot 17");
        assert_eq!(placed.base.name, "Vidar");
        assert_eq!(placed.base.char_num, 17);
    }

    #[test]
    fn character_deletion_failure_reaches_domain() {
        use net_contract::{dto::CharDeletionError, events::CharacterDeletionFailed};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<CharacterDeletionFailed>();
        app.add_message::<CharacterDeletionFailedEvent>();
        app.add_systems(Update, handle_character_deletion_failed_protocol);
        app.world_mut().write_message(CharacterDeletionFailed {
            char_id: 150001,
            error: CharDeletionError::CannotDelete,
        });

        app.update();

        let messages = app
            .world()
            .resource::<Messages<CharacterDeletionFailedEvent>>();
        let mut cursor = messages.get_cursor();
        let failure = cursor.read(messages).next().expect("domain failure event");
        assert_eq!(failure.character_id, 150001);
        assert_eq!(failure.error, "Cannot delete character");
    }
}
