// Login screen module
pub mod interactions;
pub mod resources;
pub mod systems;

use crate::{
    core::state::GameState,
    infrastructure::assets::loading_states::AssetLoadingState,
    presentation::ui::{shared::LoginFormData, events::LoginAttemptEvent},
};
use bevy::prelude::*;
use bevy_lunex::prelude::*;

pub use interactions::*;
pub use resources::*;
pub use systems::*;

pub struct LoginPlugin;

impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiLunexPlugins)
            .add_systems(
                Update,
                setup_login_ui_once
                    .run_if(in_state(GameState::Login).and(in_state(AssetLoadingState::Ready))),
            )
            .add_systems(
                Update,
                (
                    handle_text_input,
                    update_input_display,
                    update_status_text,
                    handle_tab_navigation,
                    handle_enter_key_login,
                    process_login_attempts,
                    handle_login_started,
                    handle_login_failure_ui,
                    handle_login_success_ui,
                )
                    .run_if(in_state(GameState::Login)),
            )
            .add_systems(OnExit(GameState::Login), cleanup_login_ui)
            .add_event::<LoginAttemptEvent>()
            .insert_resource(LoginFormData::default())
            .insert_resource(LoginUiState::default());
    }
}