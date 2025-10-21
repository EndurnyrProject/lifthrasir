use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_system, auto_plugin, AutoPlugin};

use crate::domain::camera::components::{CameraFollowSettings, CameraFollowTarget};
use crate::domain::camera::systems::{
    camera_follow_system, spawn_camera_on_player_ready, update_camera_target_cache, CameraSpawned,
};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;

// Extend the auto-generated Plugin implementation with camera systems
impl LifthrasirPlugin {
    pub fn add_camera_systems(app: &mut App) {
        // Register camera components for reflection
        app.register_type::<CameraFollowTarget>()
            .register_type::<CameraFollowSettings>();

        // Initialize CameraSpawned resource
        app.init_resource::<CameraSpawned>();

        // Add camera spawn system in PostUpdate
        // Runs when player has Transform component
        app.add_systems(PostUpdate, spawn_camera_on_player_ready);

        // Add camera follow systems in Update with proper ordering
        // update_camera_target_cache MUST run before camera_follow_system
        app.add_systems(
            Update,
            (update_camera_target_cache, camera_follow_system).chain(),
        );
    }
}

#[auto_add_system(
    plugin = LifthrasirPlugin,
    schedule = Startup
)]
fn initialize_app(_commands: Commands) {
    // The authentication plugin will handle the state transition after loading config
    info!("Application initialized");
}
