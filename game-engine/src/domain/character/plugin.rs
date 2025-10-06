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

        // Initialize zone server client resource
        app.init_resource::<crate::infrastructure::networking::ZoneServerClient>();

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
            .add_event::<RefreshCharacterListEvent>()
            .add_event::<ZoneServerConnected>()
            .add_event::<ZoneServerConnectionFailed>()
            .add_event::<ZoneAuthenticationSuccess>()
            .add_event::<ZoneAuthenticationFailed>()
            .add_event::<MapLoadingStarted>()
            .add_event::<MapLoadCompleted>()
            .add_event::<MapLoadingFailed>()
            .add_event::<ActorInitSent>();

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
                crate::infrastructure::networking::zone_connection_system,
                crate::infrastructure::networking::zone_packet_handler_system,
                handle_zone_auth_success,
                start_map_loading_timer,
                detect_map_loading_timeout,
                detect_map_load_complete,
                handle_map_load_complete,
                handle_actor_init_sent,
            )
                .chain(), // Chain them to ensure update runs before event handlers
        );
    }
}
