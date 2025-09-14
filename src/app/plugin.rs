use crate::core::state::{GameState, LoginState, MapState};
// use crate::domain::camera::controller::camera_movement_system;  // Disabled for UI development
// use crate::presentation::rendering::terrain::setup;            // Disabled for UI development
use bevy::prelude::*;
// Animation system available: use crate::systems::animate_sprites;

pub struct LifthrasirPlugin;

impl Plugin for LifthrasirPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<LoginState>()
            .init_state::<MapState>()
            .add_systems(Startup, initialize_app);
        // .add_systems(OnEnter(GameState::InGame), setup)                                    // Disabled for UI development
        // .add_systems(Update, camera_movement_system.run_if(in_state(GameState::InGame))); // Disabled for UI development
        // .add_systems(Update, animate_sprites); // Ready for map entities
    }
}

fn initialize_app(mut next_state: ResMut<NextState<GameState>>) {
    // Transition directly to Login screen instead of Loading
    next_state.set(GameState::Login);
}
