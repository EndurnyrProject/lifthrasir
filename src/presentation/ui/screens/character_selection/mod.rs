use crate::core::state::{CharacterScreenState, GameState};
use crate::domain::character::components::CharacterSelectionState;
use crate::domain::character::rendering::{
    CharacterSelectionRenderState, animate_character_selection_sprites_system,
    finalize_character_sprites_system, handle_character_hover_system,
    request_character_assets_system,
};
use crate::domain::character::systems::{
    handle_char_server_events, handle_character_created, handle_character_deleted,
    handle_create_character, handle_delete_character, handle_select_character,
    handle_zone_server_info,
};
use crate::domain::character::*;
use crate::domain::entities::animation::animate_sprites;
use crate::domain::entities::sprite_factory::finalize_pending_sprite_loads;
use crate::infrastructure::networking::CharServerEvent;
use bevy::prelude::*;

// New modular structure
pub mod creation;
pub mod list;
pub mod shared;

// Keep existing modules for backward compatibility during transition
pub mod interactions;
pub mod sprite_display;

// Re-export from new modules
pub use creation::*;
pub use interactions::*;
pub use list::*;
pub use shared::*;
pub use sprite_display::*;

pub struct CharacterSelectionPlugin;

impl Plugin for CharacterSelectionPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .init_resource::<CharacterListResource>()
            .init_resource::<CharacterSelectionResource>()
            .init_resource::<CharacterCreationResource>()
            .init_resource::<CharacterSelectionState>()
            .init_resource::<CharacterSelectionRenderState>()
            // Events
            .add_event::<CharServerEvent>()
            .add_event::<RequestCharacterListEvent>()
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
            // Systems - OnEnter
            .add_systems(
                OnEnter(GameState::CharacterSelection),
                (
                    setup_character_selection_assets,
                    setup_character_selection_screen,
                    connect_to_character_server,
                )
                    .chain(),
            )
            // Systems - OnExit
            .add_systems(
                OnExit(GameState::CharacterSelection),
                cleanup_character_selection_screen,
            )
            // Systems - Update (when in CharacterSelection state)
            // Network and character management systems
            .add_systems(
                Update,
                (
                    crate::infrastructure::networking::char_client::char_client_update_system,
                    handle_char_server_events,
                    handle_select_character,
                    handle_create_character,
                    handle_delete_character,
                    handle_zone_server_info,
                    handle_character_created,
                    handle_character_deleted,
                )
                    .run_if(in_state(GameState::CharacterSelection)),
            )
            // UI and sprite systems
            .add_systems(
                Update,
                (
                    initialize_character_frame_textures,
                    update_character_list_ui,
                    update_character_details_panel,
                    spawn_character_selection_sprites,
                    request_character_assets_system,
                    finalize_character_sprites_system,
                    update_character_selection_states,
                    finalize_pending_sprite_loads, // Generic sprite loading finalizer
                    animate_sprites, // Generic sprite animation system
                    animate_character_selection_sprites_system, // Updates action indices
                    handle_character_hover_system,
                )
                    .run_if(in_state(GameState::CharacterSelection)),
            )
            // Interaction systems
            .add_systems(
                Update,
                (
                    handle_character_card_click,
                    handle_select_button_click,
                    handle_delete_button_click,
                    handle_back_button_click,
                    handle_character_hover,
                )
                    .run_if(in_state(GameState::CharacterSelection)),
            )
            // Event handler for character creation - runs in PostUpdate to ensure events are flushed
            .add_systems(
                PostUpdate,
                handle_open_character_creation.run_if(in_state(GameState::CharacterSelection)),
            )
            // Debug system to log state changes
            .add_systems(
                Update,
                (log_state_changes, log_game_state_changes)
                    .run_if(in_state(GameState::CharacterSelection)),
            )
            // Systems for character screen sub-states
            .add_systems(
                OnEnter(CharacterScreenState::CharacterList),
                setup_character_list_ui,
            )
            .add_systems(
                OnExit(CharacterScreenState::CharacterList),
                cleanup_character_list_ui,
            )
            .add_systems(
                OnEnter(CharacterScreenState::CharacterCreation),
                (setup_character_creation_ui, initialize_hair_options),
            )
            .add_systems(
                OnExit(CharacterScreenState::CharacterCreation),
                cleanup_character_creation_ui,
            )
            .add_systems(
                Update,
                (
                    spawn_all_hair_previews,
                    handle_character_creation_back_button,
                    auto_focus_character_name_input,
                    handle_character_name_input,
                    handle_input_field_focus,
                    validate_character_name_input,
                    update_character_name_display,
                    update_input_visual_feedback,
                    update_input_border_feedback,
                    initialize_gender_selection,
                    update_gender_button_highlight,
                    // Hair customization systems (Phase 3)
                    populate_hair_style_buttons,
                    populate_hair_color_buttons,
                    update_hair_style_button_highlight,
                    update_hair_color_button_highlight,
                )
                    .run_if(in_state(CharacterScreenState::CharacterCreation)),
            );
    }
}
