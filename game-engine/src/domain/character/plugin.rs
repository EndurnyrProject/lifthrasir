use super::components::CharacterSelectionState;
use super::creation::{
    cleanup_character_creation_preview, handle_enter_character_creation,
    handle_update_character_preview, setup_character_creation_camera,
    update_preview_position_on_window_resize, CharacterCreationState,
};
use super::events::*;
use super::systems::*;
use crate::core::state::GameState;
use crate::domain::entities::animation::animate_sprites;
use crate::domain::entities::character::UnifiedCharacterEntityPlugin;
use crate::infrastructure::networking::CharServerEvent;
use crate::presentation::ui::character_selection::{
    setup_character_selection_camera,
    sprites::{
        cleanup_character_selection, setup_character_slot_containers,
        spawn_character_sprites_on_list_received, update_sprite_positions_on_window_resize,
    },
};
use bevy::prelude::*;

/// Minimal character domain plugin that registers events and systems
/// but no UI (UI is handled by Tauri)
pub struct CharacterDomainPlugin;

impl Plugin for CharacterDomainPlugin {
    fn build(&self, app: &mut App) {
        // Initialize character selection state resource
        app.insert_resource(CharacterSelectionState::default());

        // Initialize character creation state resource
        app.insert_resource(CharacterCreationState::default());

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
            .add_event::<OpenCharacterCreationEvent>()
            .add_event::<CloseCharacterCreationEvent>()
            .add_event::<CharacterHoverEvent>()
            .add_event::<RefreshCharacterListEvent>()
            .add_event::<super::creation::UpdateCharacterPreviewEvent>();

        // Setup character selection rendering (camera and containers)
        app.add_systems(
            OnEnter(GameState::CharacterSelection),
            (
                setup_character_selection_camera,
                setup_character_slot_containers,
            ),
        );

        // Cleanup character selection entities when exiting the state
        app.add_systems(
            OnExit(GameState::CharacterSelection),
            cleanup_character_selection,
        );

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
            )
                .chain(), // Chain them to ensure update runs before event handlers
        );

        // Register character sprite rendering systems (only active during CharacterSelection)
        app.add_systems(
            Update,
            (
                spawn_character_sprites_on_list_received,
                ApplyDeferred, // Flush commands so sprite hierarchy system can find newly created entities
                animate_sprites, // CRITICAL: This system updates sprite images from SPR/ACT data
                update_sprite_positions_on_window_resize,
            )
                .chain()
                .run_if(in_state(GameState::CharacterSelection)),
        );

        // Setup character creation preview when entering CharacterCreation state
        app.add_systems(
            OnEnter(GameState::CharacterCreation),
            (
                setup_character_creation_camera,
                handle_enter_character_creation,
            ),
        );

        // Cleanup character creation preview when exiting CharacterCreation state
        app.add_systems(
            OnExit(GameState::CharacterCreation),
            cleanup_character_creation_preview,
        );

        // Register character creation preview update systems
        app.add_systems(
            Update,
            (
                handle_update_character_preview,
                animate_sprites, // CRITICAL: Render the preview character sprites
                update_preview_position_on_window_resize,
            )
                .chain()
                .run_if(in_state(GameState::CharacterCreation)),
        );
    }
}
