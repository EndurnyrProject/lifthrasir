pub mod apply;
pub mod events;
pub mod resources;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::{AutoPlugin, auto_add_system};

pub use events::ApplySettings;
pub use resources::{
    ActionBinds, Anisotropy, AntiAliasing, AudioConfig, DisplayMode, FpsCap, GraphicsSettings,
    KeyBind, Keybinds, Modifier, RESOLUTIONS, UiScaling, resolution_label, resolution_next,
    resolution_prev,
};

/// Synchronizes loaded or newly committed settings resources with live runtime
/// state that Bevy's settings framework does not configure itself.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct SettingsRuntimePlugin;

/// Applies the settings loaded by Bevy once on boot.
#[auto_add_system(plugin = SettingsRuntimePlugin, schedule = PostStartup)]
fn emit_initial_apply(mut messages: MessageWriter<ApplySettings>) {
    messages.write(ApplySettings);
}
