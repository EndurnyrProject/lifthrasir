use std::path::PathBuf;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_persistent::prelude::*;

use super::SettingsPlugin;
use super::resources::Settings;

/// `<config dir>/lifthrasir/settings.ron`.
pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .expect("a platform config directory")
        .join("lifthrasir")
        .join("settings.ron")
}

/// Loads `settings.ron` (or writes defaults on first run) into a
/// `Persistent<Settings>` resource. The builder creates the parent directory.
///
/// `#[serde(default)]` on the settings structs absorbs additive schema changes
/// (a missing field falls back to its default). A file that still fails to parse
/// — wrong field type, unknown enum, hand-edited corruption — is reset to
/// defaults with a warning rather than crashing the client.
#[auto_add_system(plugin = SettingsPlugin, schedule = Startup)]
pub fn insert_persistent_settings(mut commands: Commands) {
    let path = settings_path();
    let build = || {
        Persistent::<Settings>::builder()
            .name("settings")
            .format(StorageFormat::Ron)
            .path(path.clone())
            .default(Settings::default())
            .build()
    };
    let settings = build().unwrap_or_else(|error| {
        warn!("settings.ron failed to load ({error}); resetting to defaults");
        let _ = std::fs::remove_file(&path);
        build().expect("failed to build settings after reset")
    });
    commands.insert_resource(settings);
}
