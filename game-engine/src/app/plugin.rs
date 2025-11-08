use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_system, auto_plugin, AutoPlugin};

use crate::domain::camera::resources::CameraRotationDelta;
use crate::domain::camera::systems::{
    camera_follow_system, spawn_camera_on_player_ready, update_camera_target_cache, CameraSpawned,
};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;

impl LifthrasirPlugin {
    pub fn add_camera_systems(app: &mut App) {
        app.init_resource::<CameraSpawned>();
        app.init_resource::<CameraRotationDelta>();
        app.add_systems(PostUpdate, spawn_camera_on_player_ready);
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
    info!("Application initialized");
}
