pub mod apply;
pub mod events;
pub mod persistence;
pub mod resources;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, AutoPlugin};

pub use events::ApplySettings;
pub use persistence::settings_path;
pub use resources::{
    resolution_label, resolution_next, resolution_prev, ActionBinds, Anisotropy, AntiAliasing,
    AudioConfig, DisplayMode, FpsCap, GraphicsSettings, KeyBind, Keybinds, Modifier, Settings,
    UiScaling, RESOLUTIONS,
};

/// Owns the persisted `Settings` resource: loads `settings.ron` (or writes
/// defaults) on startup, then applies it to the live world.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct SettingsPlugin;

/// Applies the loaded settings once on boot. `PostStartup` runs after the
/// `Startup` insert command has been flushed, so the resource exists; the
/// message is then read by the apply systems on the first `Update`.
#[auto_add_system(plugin = SettingsPlugin, schedule = PostStartup)]
fn emit_initial_apply(mut messages: MessageWriter<ApplySettings>) {
    messages.write(ApplySettings);
}
