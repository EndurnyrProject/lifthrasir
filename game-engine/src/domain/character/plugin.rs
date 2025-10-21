use super::components::CharacterSelectionState;
use super::events::*;
use super::systems::*;
use crate::core::state::GameState;
use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
use crate::infrastructure::networking::protocol::character::{
    BlockedCharactersReceived, CharacterCreated, CharacterCreationFailed, CharacterDeleted,
    CharacterDeletionFailed, CharacterInfoPageReceived, CharacterServerConnected,
    CharacterSlotInfoReceived, PingReceived, SecondPasswordRequested, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::protocol::zone::{
    AccountIdReceived, ZoneEntryRefused, ZoneServerConnected as ZoneServerConnectedProtocol,
};
use bevy::prelude::*;

/// Minimal character domain plugin that registers events and systems
pub struct CharacterDomainPlugin;

impl Plugin for CharacterDomainPlugin {
    fn build(&self, app: &mut App) {
        // Initialize character selection state resource
        app.insert_resource(CharacterSelectionState::default());

        // Add unified character entity plugin (includes sprite hierarchy and state machines)
        app.add_plugins(UnifiedCharacterEntityPlugin);

        // Register protocol layer events (from new networking architecture)
        // Character protocol events
        app.add_message::<CharacterServerConnected>()
            .add_message::<CharacterCreated>()
            .add_message::<CharacterCreationFailed>()
            .add_message::<CharacterDeleted>()
            .add_message::<CharacterDeletionFailed>()
            .add_message::<ZoneServerInfoReceived>()
            .add_message::<PingReceived>()
            .add_message::<SecondPasswordRequested>()
            .add_message::<CharacterInfoPageReceived>()
            .add_message::<CharacterSlotInfoReceived>()
            .add_message::<BlockedCharactersReceived>();

        // Zone protocol events (new modular architecture)
        app.add_message::<ZoneServerConnectedProtocol>()
            .add_message::<AccountIdReceived>()
            .add_message::<ZoneEntryRefused>();

        // Register all character-related domain events
        app.add_message::<RequestCharacterListEvent>()
            .add_message::<CharacterListReceivedEvent>()
            .add_message::<SelectCharacterEvent>()
            .add_message::<CharacterSelectedEvent>()
            .add_message::<EnterGameRequestEvent>()
            .add_message::<ZoneServerInfoReceivedEvent>()
            .add_message::<CreateCharacterRequestEvent>()
            .add_message::<CharacterCreatedEvent>()
            .add_message::<CharacterCreationFailedEvent>()
            .add_message::<DeleteCharacterRequestEvent>()
            .add_message::<CharacterDeletedEvent>()
            .add_message::<CharacterDeletionFailedEvent>()
            .add_message::<CharacterHoverEvent>()
            .add_message::<RefreshCharacterListEvent>()
            .add_message::<ZoneServerConnected>()
            .add_message::<ZoneServerConnectionFailed>()
            .add_message::<ZoneAuthenticationSuccess>()
            .add_message::<ZoneAuthenticationFailed>()
            .add_message::<MapLoadingStarted>()
            .add_message::<MapLoadCompleted>()
            .add_message::<MapLoadingFailed>()
            .add_message::<ActorInitSent>();

        // Register character networking systems
        // Split into smaller groups to avoid Rust tuple size limits
        app.add_systems(
            Update,
            (
                // Character server systems
                crate::infrastructure::networking::client::char_server_update_system,
                update_char_client, // Keep-alive pings
                (
                    // Protocol event handlers - translate protocol events to domain events
                    handle_character_server_connected,
                    handle_character_created_protocol,
                    handle_character_creation_failed_protocol,
                    handle_character_deleted_protocol,
                    handle_character_deletion_failed_protocol,
                    handle_zone_server_info_protocol,
                )
                    .chain(),
                (
                    // Domain event handlers
                    handle_request_character_list,
                    handle_select_character,
                    spawn_unified_character_from_selection,
                    handle_create_character,
                    handle_delete_character,
                )
                    .chain(),
                (
                    handle_zone_server_info,
                    handle_character_created,
                    handle_character_deleted,
                    handle_refresh_character_list,
                )
                    .chain(),
                (
                    // Zone server systems (new modular architecture)
                    crate::infrastructure::networking::client::zone_server_update_system,
                    crate::infrastructure::networking::client::time_sync_system,
                    handle_zone_server_connected_protocol,
                    handle_zone_entry_refused_protocol,
                    handle_account_id_received_protocol,
                )
                    .chain(),
                (
                    // Map loading systems
                    start_map_loading_timer,
                    detect_map_loading_timeout,
                    detect_map_load_complete,
                    handle_map_load_complete,
                    handle_actor_init_sent,
                )
                    .chain(),
            )
                .chain(),
        );

        app.add_systems(
            OnEnter(GameState::InGame),
            spawn_character_sprite_on_game_start,
        );
    }
}
