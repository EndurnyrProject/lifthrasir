//! Lifthrasir map editor: edits map glbs through the runtime's own scene.

pub mod map;
pub mod shell;

use bevy::prelude::*;
use lifthrasir_assets::source::AssetConfig;
use std::path::PathBuf;

/// Marks entities that exist only in the editor (handles, overlays, previews).
/// They are never serialized into a map.
#[derive(Component, Default, Clone, Copy)]
pub struct EditorOnly;

/// Where the editor writes: the loose `data_folder` from `assets/loader.toml`,
/// never a pak.
#[derive(Resource, Clone)]
pub struct EditorPaths {
    pub data_folder: PathBuf,
}

impl EditorPaths {
    pub fn from_config(config: &AssetConfig) -> Self {
        Self {
            data_folder: config.data_folder_path(),
        }
    }
}
