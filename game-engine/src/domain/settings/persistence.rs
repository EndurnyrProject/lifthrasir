use std::path::PathBuf;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_persistent::prelude::*;

use super::resources::Settings;
use super::SettingsPlugin;

/// `<config dir>/lifthrasir/settings.ron`.
pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .expect("a platform config directory")
        .join("lifthrasir")
        .join("settings.ron")
}

/// Loads `settings.ron` (or writes defaults on first run) into a
/// `Persistent<Settings>` resource. The builder creates the parent directory.
#[auto_add_system(plugin = SettingsPlugin, schedule = Startup)]
pub fn insert_persistent_settings(mut commands: Commands) {
    let settings = Persistent::<Settings>::builder()
        .name("settings")
        .format(StorageFormat::Ron)
        .path(settings_path())
        .default(Settings::default())
        .build()
        .expect("failed to build the persistent settings resource");
    commands.insert_resource(settings);
}
