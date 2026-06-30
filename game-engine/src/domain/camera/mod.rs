pub mod components;
pub mod resources;
pub mod systems;

use bevy::prelude::*;

pub use resources::CameraRotationDelta;
pub use systems::CameraSpawned;

use crate::core::state::GameState;
use resources::{ActiveCameraProfile, IndoorMapTable};
use systems::{apply_camera_map_profile, load_indoor_map_table, spawn_camera_on_player_ready};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraSpawned>();
        app.init_resource::<CameraRotationDelta>();
        app.init_resource::<IndoorMapTable>();
        app.init_resource::<ActiveCameraProfile>();
        app.add_systems(Startup, load_indoor_map_table);
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
