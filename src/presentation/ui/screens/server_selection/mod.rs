// Server selection screen module
pub mod interactions;
pub mod resources;
pub mod systems;

pub use resources::*;
pub use systems::*;

use crate::{core::state::GameState, presentation::ui::events::*};
use bevy::prelude::*;

pub struct ServerSelectionPlugin;

impl Plugin for ServerSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            setup_server_selection_ui_once.run_if(in_state(GameState::ServerSelection)),
        )
        .add_systems(
            Update,
            (
                handle_server_card_click,
                handle_connect_button,
                handle_back_to_login,
                handle_keyboard_navigation,
            )
                .run_if(in_state(GameState::ServerSelection)),
        )
        .add_systems(
            Update,
            (handle_server_hover_effects).run_if(in_state(GameState::ServerSelection)),
        )
        .add_systems(
            OnExit(GameState::ServerSelection),
            cleanup_server_selection_ui,
        )
        .add_event::<ServerSelectedEvent>()
        .add_event::<BackToLoginEvent>()
        .insert_resource(ServerSelectionState::default());
    }
}
