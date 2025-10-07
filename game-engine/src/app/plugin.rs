use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_add_system, auto_plugin, AutoPlugin};
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
