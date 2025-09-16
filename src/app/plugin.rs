use crate::core::state::{GameState, LoginState, MapState, NetworkState};
// use crate::domain::camera::controller::camera_movement_system;  // Disabled for UI development
// use crate::presentation::rendering::terrain::setup;            // Disabled for UI development
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_plugin, auto_add_system, AutoPlugin};
// Animation system available: use crate::systems::animate_sprites;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;

#[auto_add_system(
    plugin = LifthrasirPlugin,
    schedule = Startup
)]
fn initialize_app(_commands: Commands) {
    // The authentication plugin will handle the state transition after loading config
    info!("Application initialized");
}
