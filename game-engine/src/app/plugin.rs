use bevy::prelude::*;
use bevy_auto_plugin::prelude::{AutoPlugin, auto_add_system};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;

#[auto_add_system(
    plugin = LifthrasirPlugin,
    schedule = Startup
)]
fn initialize_app(_commands: Commands) {
    debug!("Application initialized");
}
