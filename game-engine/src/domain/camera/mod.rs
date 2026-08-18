pub mod components;
pub mod resources;
pub mod systems;

use bevy::prelude::*;

pub use resources::CameraRotationDelta;
pub use systems::CameraSpawned;

use crate::core::state::GameState;
use resources::ActiveCameraProfile;
use systems::{apply_camera_map_profile, spawn_camera_on_player_ready};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraSpawned>();
        app.init_resource::<CameraRotationDelta>();
        app.init_resource::<ActiveCameraProfile>();
        app.add_systems(
            PostUpdate,
            spawn_camera_on_player_ready.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            apply_camera_map_profile.run_if(in_state(GameState::InGame)),
        );
    }
}
