//! Save and save-as: collect the live map, validate it, check its asset
//! references and write the glb atomically into the loose `data_folder`.

use super::{
    document::collect_document,
    layout::MapLayout,
    open::{EditorOperation, EditorScreen, ErrorModal, OpenMap},
    session::{EditorSession, record_recent_map},
};
use crate::{
    EditorPaths,
    shell::{ui::EditorUiSet, undo::UndoStack},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_tooling::converters::map::{
    document::{MapDocument, is_valid_map_name},
    writer::write_map_glb,
};
use std::path::{Path, PathBuf};

/// Saves the open map; `save_as` renames it first.
#[derive(Message, Debug, Clone)]
pub struct SaveRequest {
    pub save_as: Option<String>,
}

/// The Save as dialog.
#[derive(Resource, Default)]
pub struct SaveAsDialog {
    open: bool,
    name: String,
    confirm_overwrite: bool,
}

impl SaveAsDialog {
    pub fn open(&mut self, current: &str) {
        *self = Self {
            open: true,
            name: current.to_string(),
            confirm_overwrite: false,
        };
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SaveRequest>()
        .init_resource::<SaveAsDialog>()
        .add_systems(
            Update,
            (save_shortcut, handle_save_requests).run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            save_as_dialog.in_set(EditorUiSet::Windows),
        );
}

/// Ctrl/Cmd+S.
fn save_shortcut(keys: Res<ButtonInput<KeyCode>>, mut saves: MessageWriter<SaveRequest>) {
    let modifier = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if modifier && keys.just_pressed(KeyCode::KeyS) {
        saves.write(SaveRequest { save_as: None });
    }
}

fn handle_save_requests(
    mut requests: MessageReader<SaveRequest>,
    mut commands: Commands,
    layout: Option<Res<MapLayout>>,
    mut operation: ResMut<EditorOperation>,
) {
    for request in requests.read().cloned() {
        let name = request
            .save_as
            .clone()
            .or_else(|| layout.as_ref().map(|layout| layout.name.clone()))
            .unwrap_or_else(|| "map".to_string());
        *operation = EditorOperation::Saving { name };
        commands.queue(move |world: &mut World| {
            if let Err(messages) = save(world, request.save_as.as_deref()) {
                world
                    .resource_mut::<ErrorModal>()
                    .show("Could not save the map", messages);
                *world.resource_mut::<EditorOperation>() = EditorOperation::Idle;
            }
        });
    }
}

/// Collects, validates, checks references, writes, then marks the history
/// saved. Save-as renames the open map without reloading it.
fn save(world: &mut World, save_as: Option<&str>) -> Result<(), Vec<String>> {
    let mut layout = world
        .get_resource::<MapLayout>()
        .cloned()
        .ok_or_else(|| vec!["no map is open".to_string()])?;
    if let Some(name) = save_as {
        layout.name = name.to_string();
    }
    let pristine = world.resource::<UndoStack>().is_pristine();
    let doc =
        collect_document(world, &layout, pristine).map_err(|error| vec![error.to_string()])?;

    let mut messages = doc.validate().err().unwrap_or_default();
    let source = world.resource::<SharedCompositeAssetSource>().0.clone();
    messages.extend(validate_references(
        &doc,
        &*source.read().expect("ro:// source lock poisoned"),
    ));
    if !messages.is_empty() {
        return Err(messages);
    }
    let bytes = write_map_glb(&doc).map_err(|error| vec![format!("{error:#}")])?;
    let path = map_glb_path(&world.resource::<EditorPaths>().data_folder, &layout.name);
    write_atomically(&path, &bytes)
        .map_err(|error| vec![format!("writing {}: {error}", path.display())])?;

    world.resource_mut::<UndoStack>().mark_saved();
    if save_as.is_some() {
        let name = layout.name.clone();
        world.insert_resource(layout);
        let mut open = world.resource_mut::<OpenMap>();
        open.path = format!("ro://maps/{name}/{name}.glb");
        open.name = name;
    }
    let open = world.resource::<OpenMap>();
    let name = open.name.clone();
    let open_path = save_as.is_some().then(|| open.path.clone());
    if let Some(open_path) = open_path {
        record_recent_map(&mut world.resource_mut::<EditorSession>(), &open_path);
    }
    *world.resource_mut::<EditorOperation>() = EditorOperation::Saved { name };
    info!("saved {}", path.display());
    Ok(())
}

/// Asks for a valid name and confirms before overwriting another map's loose
/// file.
fn save_as_dialog(
    mut contexts: EguiContexts,
    mut dialog: ResMut<SaveAsDialog>,
    layout: Option<Res<MapLayout>>,
    paths: Res<EditorPaths>,
    mut saves: MessageWriter<SaveRequest>,
) -> Result {
    if !dialog.open {
        return Ok(());
    }
    let Some(layout) = layout else {
        dialog.open = false;
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let dialog = &mut *dialog;
    let modal = egui::Modal::new(egui::Id::new("save_as")).show(ctx, |ui| {
        ui.heading("Save map as");
        if ui.text_edit_singleline(&mut dialog.name).changed() {
            dialog.confirm_overwrite = false;
        }
        let valid = is_valid_map_name(&dialog.name);
        if !valid {
            ui.colored_label(ui.visuals().error_fg_color, "Use only a-z, 0-9, _, - and @");
        }
        if dialog.confirm_overwrite {
            ui.label(format!(
                "maps/{0}/{0}.glb already exists in the data folder. Overwrite it?",
                dialog.name
            ));
        }
        ui.horizontal(|ui| {
            let label = if dialog.confirm_overwrite {
                "Overwrite"
            } else {
                "Save"
            };
            let submit = ui.add_enabled(valid, egui::Button::new(label)).clicked();
            (submit, ui.button("Cancel").clicked())
        })
        .inner
    });
    let (submit, cancel) = modal.inner;
    if cancel || modal.should_close() {
        dialog.open = false;
        return Ok(());
    }
    if !submit {
        return Ok(());
    }
    let overwrites =
        dialog.name != layout.name && map_glb_path(&paths.data_folder, &dialog.name).exists();
    if overwrites && !dialog.confirm_overwrite {
        dialog.confirm_overwrite = true;
        return Ok(());
    }
    saves.write(SaveRequest {
        save_as: Some(dialog.name.clone()),
    });
    dialog.open = false;
    Ok(())
}

/// Where map `name` lives in the loose data folder.
pub fn map_glb_path(data_folder: &Path, name: &str) -> PathBuf {
    data_folder
        .join("maps")
        .join(name)
        .join(format!("{name}.glb"))
}

/// Writes `bytes` to `<path>.tmp` beside `path`, then renames it over `path`.
/// Parent directories are created; a failed write leaves `path` untouched.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// One message per prop model, sound file or ground texture `doc` references
/// that `source` does not have.
pub fn validate_references(doc: &MapDocument, source: &dyn AssetSource) -> Vec<String> {
    let props = doc
        .props
        .iter()
        .map(|prop| ("prop", &prop.name, &prop.payload.model));
    let sounds = doc
        .sounds
        .iter()
        .map(|sound| ("sound", &sound.name, &sound.payload.file));
    let textures = doc
        .textures
        .iter()
        .map(|texture| ("ground texture", &texture.name, &texture.uri));
    props
        .chain(sounds)
        .chain(textures)
        .filter(|(_, _, uri)| !source.exists(uri.strip_prefix("ro://").unwrap_or(uri)))
        .map(|(kind, name, uri)| format!("{kind} '{name}' references missing '{uri}'"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_assets::source::AssetSourceError;
    use lifthrasir_data::lif::{LifAudio, LifProp};
    use lifthrasir_tooling::converters::map::{
        blank::{LightingPreset, NewMapParams},
        document::{MapNode, TextureRef},
    };
    use std::collections::HashSet;

    struct MemorySource(HashSet<&'static str>);

    impl AssetSource for MemorySource {
        fn name(&self) -> &str {
            "memory"
        }
        fn priority(&self) -> u32 {
            0
        }
        fn exists(&self, path: &str) -> bool {
            self.0.contains(path)
        }
        fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
            Err(AssetSourceError::NotFound(path.into()))
        }
        fn list_files(&self) -> Vec<String> {
            self.0.iter().map(|path| path.to_string()).collect()
        }
    }

    fn node<T>(name: &str, payload: T) -> MapNode<T> {
        MapNode {
            name: name.into(),
            translation: Default::default(),
            rotation: Default::default(),
            scale: Default::default(),
            payload,
        }
    }

    fn doc() -> MapDocument {
        let mut doc = MapDocument::new_flat(&NewMapParams {
            name: "refs".into(),
            width: 4,
            height: 4,
            texture: TextureRef {
                name: "grass".into(),
                uri: "ro://ground/tex/grass.ktx2".into(),
            },
            heightmap: None,
            height_range: 0.0,
            preset: LightingPreset::Outdoor,
        })
        .unwrap();
        doc.textures.push(TextureRef {
            name: "gone".into(),
            uri: "ro://ground/tex/gone.ktx2".into(),
        });
        for (name, model) in [("tree", "models/tree.glb"), ("rock", "models/rock.glb")] {
            doc.props.push(node(
                name,
                LifProp {
                    model: format!("ro://{model}"),
                    anim_type: 0,
                    anim_speed: 1.0,
                },
            ));
        }
        doc.sounds.push(node(
            "river",
            LifAudio {
                name: "river".into(),
                file: "ro://data/wav/river.wav".into(),
                volume: 1.0,
                range: 1.0,
                cycle: 1.0,
            },
        ));
        doc
    }

    #[test]
    fn a_map_lives_in_its_own_folder_under_maps() {
        assert_eq!(
            map_glb_path(Path::new("assets/data"), "1@pump"),
            Path::new("assets/data/maps/1@pump/1@pump.glb")
        );
    }

    #[test]
    fn atomic_writes_create_parents_replace_and_leave_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("maps/x/x.glb");

        write_atomically(&path, b"first").unwrap();
        write_atomically(&path, b"second").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, ["x.glb"]);
    }

    #[test]
    fn every_missing_reference_is_reported_once_by_name() {
        let source = MemorySource(HashSet::from(["ground/tex/grass.ktx2", "models/tree.glb"]));

        let messages = validate_references(&doc(), &source);

        assert_eq!(messages.len(), 3, "{messages:?}");
        for name in ["rock", "river", "gone"] {
            assert!(
                messages
                    .iter()
                    .any(|message| message.contains(&format!("'{name}'"))),
                "{name} missing from {messages:?}"
            );
        }
    }
}
