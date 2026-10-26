//! Per-user editor session preferences: recently opened maps and optional
//! reopening of the last successfully opened map.

use super::open::OpenMapRequest;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_RECENT_MAPS: usize = 10;

/// Preferences which belong to the editor user, not to a map or its content
/// folder. Paths are `ro://` paths so they remain valid when a pak overrides a
/// loose map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorPreferences {
    pub reopen_last_session: bool,
    pub recent_maps: Vec<String>,
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u32,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            reopen_last_session: true,
            recent_maps: Vec::new(),
            autosave_enabled: true,
            autosave_interval_secs: 60,
        }
    }
}

/// Loaded preferences and their on-disk location.
#[derive(Resource, Debug)]
pub struct EditorSession {
    pub preferences: EditorPreferences,
    path: PathBuf,
}

impl Default for EditorSession {
    fn default() -> Self {
        let path = preferences_path();
        Self {
            preferences: read_preferences(&path),
            path,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<EditorSession>()
        .add_systems(Startup, reopen_last_map);
}

/// Adds a successfully opened (or newly saved-as) map to the MRU list.
pub fn record_recent_map(session: &mut EditorSession, path: &str) {
    if session
        .preferences
        .recent_maps
        .first()
        .is_some_and(|recent| recent == path)
    {
        return;
    }
    session
        .preferences
        .recent_maps
        .retain(|recent| recent != path);
    session.preferences.recent_maps.insert(0, path.to_string());
    session.preferences.recent_maps.truncate(MAX_RECENT_MAPS);
    persist_preferences(session);
}

/// Saves preferences after a UI toggle. Failures deliberately do not affect an
/// editing session; they are reported to the log and retried by later changes.
pub fn persist_preferences(session: &EditorSession) {
    if let Err(error) = write_preferences(&session.path, &session.preferences) {
        warn!(
            "writing editor preferences {}: {error}",
            session.path.display()
        );
    }
}

fn reopen_last_map(
    session: Res<EditorSession>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    mut opens: MessageWriter<OpenMapRequest>,
) {
    // Headless hosts use MapEditorPlugin for document tests and tools; only the
    // interactive editor should restore a user's desktop session.
    if windows.is_empty() {
        return;
    }
    if !session.preferences.reopen_last_session {
        return;
    }
    let Some(path) = session.preferences.recent_maps.first() else {
        return;
    };
    opens.write(OpenMapRequest { path: path.clone() });
}

fn preferences_path() -> PathBuf {
    editor_config_dir().join("preferences.toml")
}

/// Per-user editor folder: preferences and autosave recovery snapshots.
pub(super) fn editor_config_dir() -> PathBuf {
    if let Some(directory) = std::env::var_os("LIFTHRASIR_EDITOR_CONFIG_DIR") {
        return PathBuf::from(directory);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lifthrasir")
        .join("editor")
}

fn read_preferences(path: &Path) -> EditorPreferences {
    let Ok(text) = std::fs::read_to_string(path) else {
        return EditorPreferences::default();
    };
    match toml::from_str(&text) {
        Ok(preferences) => preferences,
        Err(error) => {
            warn!("reading editor preferences {}: {error}", path.display());
            EditorPreferences::default()
        }
    }
}

fn write_preferences(path: &Path, preferences: &EditorPreferences) -> Result<(), String> {
    let text = toml::to_string_pretty(preferences).map_err(|error| error.to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "preferences path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_maps_are_unique_and_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = EditorSession {
            preferences: EditorPreferences::default(),
            path: directory.path().join("preferences.toml"),
        };
        for index in 0..12 {
            record_recent_map(&mut session, &format!("ro://maps/{index}/{index}.glb"));
        }
        record_recent_map(&mut session, "ro://maps/5/5.glb");
        assert_eq!(session.preferences.recent_maps.len(), MAX_RECENT_MAPS);
        assert_eq!(session.preferences.recent_maps[0], "ro://maps/5/5.glb");
        assert_eq!(
            session
                .preferences
                .recent_maps
                .iter()
                .filter(|path| path.ends_with("/5.glb"))
                .count(),
            1
        );
    }

    #[test]
    fn preferences_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.toml");
        let preferences = EditorPreferences {
            reopen_last_session: false,
            recent_maps: vec!["ro://maps/mini/mini.glb".into()],
            autosave_enabled: false,
            autosave_interval_secs: 15,
        };

        write_preferences(&path, &preferences).unwrap();

        assert_eq!(read_preferences(&path), preferences);
    }
}
