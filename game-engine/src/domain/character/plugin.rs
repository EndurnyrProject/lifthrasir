use super::components::CharacterSelectionState;
use super::events::*;
use super::systems::*;
use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
use crate::infrastructure::networking::CharServerEvent;
use bevy::prelude::*;

/// Minimal character domain plugin that registers events and systems
/// but no UI (UI is handled by Tauri)
pub struct CharacterDomainPlugin;

impl Plugin for CharacterDomainPlugin {
    fn build(&self, app: &mut App) {
        // Initialize character selection state resource
        app.insert_resource(CharacterSelectionState::default());

        // Add unified character entity plugin (includes sprite hierarchy and state machines)
        app.add_plugins(UnifiedCharacterEntityPlugin);

        // Register networking events
        app.add_event::<CharServerEvent>();

        // Register all character-related events
        app.add_event::<RequestCharacterListEvent>()
            .add_event::<CharacterListReceivedEvent>()
            .add_event::<SelectCharacterEvent>()
            .add_event::<CharacterSelectedEvent>()
            .add_event::<EnterGameRequestEvent>()
            .add_event::<ZoneServerInfoReceivedEvent>()
            .add_event::<CreateCharacterRequestEvent>()
            .add_event::<CharacterCreatedEvent>()
            .add_event::<CharacterCreationFailedEvent>()
            .add_event::<DeleteCharacterRequestEvent>()
            .add_event::<CharacterDeletedEvent>()
            .add_event::<CharacterDeletionFailedEvent>()
            .add_event::<CharacterHoverEvent>()
            .add_event::<RefreshCharacterListEvent>();

        // Register character networking systems
        app.add_systems(
            Update,
            (
                crate::infrastructure::networking::char_client_update_system,
                handle_char_server_events,
                handle_request_character_list, // Handle explicit requests for cached character list
                handle_select_character,
                handle_create_character,
                handle_delete_character,
                handle_zone_server_info,
                handle_character_created,
                handle_character_deleted,
                handle_refresh_character_list, // Handle character list refresh requests
            )
                .chain(), // Chain them to ensure update runs before event handlers
        );
    }
}
