//! Native file picking and document/scene replacement.
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, futures_lite::future::poll_once},
};

use crate::{
    application::{EditorDocument, EditorNotice, control_pressed},
    infrastructure::content::MapSceneRequest,
    map_asset_path,
    presentation::MapAssetPath,
};

#[derive(Resource, Default)]
pub(super) struct MapFilePicker(Option<Task<Option<std::path::PathBuf>>>);

pub(super) fn start_map_file_picker(picker: &mut MapFilePicker) {
    if picker.0.is_some() {
        return;
    }
    picker.0 = Some(IoTaskPool::get().spawn(async {
        rfd::AsyncFileDialog::new()
            .set_directory("assets/data/maps")
            .add_filter("Lifthrasir maps", &["glb"])
            .pick_file()
            .await
            .map(|file| file.path().to_path_buf())
    }));
}

pub(super) fn open_file_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    mut picker: ResMut<MapFilePicker>,
) {
    if control_pressed(&keys) && keys.just_pressed(KeyCode::KeyO) {
        start_map_file_picker(&mut picker);
    }
}

pub(super) fn poll_map_file_picker(
    mut commands: Commands,
    mut picker: ResMut<MapFilePicker>,
    document: Option<ResMut<EditorDocument>>,
    mut map_path: ResMut<MapAssetPath>,
    mut requests: Query<(Entity, &mut MapSceneRequest)>,
    mut notice: ResMut<EditorNotice>,
) {
    let Some(task) = picker.0.as_mut() else {
        return;
    };
    let Some(path) = block_on(poll_once(task)) else {
        return;
    };
    picker.0 = None;
    let Some(path) = path else { return };
    let asset_path = match map_asset_path(&path) {
        Ok(path) => path,
        Err(error) => {
            notice.show(format!("Open failed: {error}"));
            return;
        }
    };
    let replacement = match lifthrasir_map_editor::MapDocument::open(&path) {
        Ok(document) => document,
        Err(error) => {
            notice.show(format!("Open failed: {error}"));
            return;
        }
    };
    if let Some(mut document) = document {
        *document = EditorDocument(replacement);
    } else {
        commands.insert_resource(EditorDocument(replacement));
    }
    map_path.0 = Some(asset_path.clone());
    if let Ok((root, mut request)) = requests.single_mut() {
        request.path = format!("ro://{asset_path}");
        commands
            .entity(root)
            .remove::<lifthrasir_runtime::world::MapSceneHandle>();
    } else {
        commands.spawn(MapSceneRequest {
            path: format!("ro://{asset_path}"),
        });
    }
    notice.show(format!("Opened {}", path.display()));
}
