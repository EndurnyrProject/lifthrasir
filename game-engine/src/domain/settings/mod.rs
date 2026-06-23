pub mod persistence;
pub mod resources;

use bevy_auto_plugin::prelude::AutoPlugin;

pub use persistence::settings_path;
pub use resources::{
    ActionBinds, AntiAliasing, AudioConfig, DisplayMode, FpsCap, GraphicsSettings, KeyBind,
    Keybinds, Modifier, Settings, RESOLUTIONS,
};

/// Owns the persisted `Settings` resource: loads `settings.ron` (or writes
/// defaults) on startup. Apply systems land in later tasks.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct SettingsPlugin;
