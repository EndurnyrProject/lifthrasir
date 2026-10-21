//! The New map wizard: a form over `MapDocument::new_flat` that writes the
//! new glb and opens it like any other map.

use super::{
    open::{GuardedAction, OpenMapRequest, StartScreenActions, UnsavedGuard},
    save::{map_glb_path, write_atomically},
};
use crate::{
    EditorPaths,
    shell::{browser::AssetIndex, ui::EditorUiSet, undo::UndoStack},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_tooling::converters::map::{
    blank::{LightingPreset, NewMapParams},
    document::{MapDocument, TextureRef},
    writer::write_map_glb,
};
use std::path::{Path, PathBuf};

/// Largest side a new map may have, in GAT cells (retail tops out near 400).
const MAX_CELLS: u32 = 1024;

/// The wizard's form. Defaults: 64 x 64 GAT cells, outdoor, height range 50.
#[derive(Resource, Debug, Clone)]
pub struct NewMapWizard {
    pub open: bool,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// A ground texture URI from `AssetIndex.ground_textures`.
    pub texture_uri: Option<String>,
    /// Optional heightmap image path; empty builds a flat map.
    pub heightmap: String,
    pub height_range: f32,
    pub preset: LightingPreset,
    /// Set by the Create button; consumed by the create system.
    pub create_requested: bool,
    /// The last create failure, shown inline.
    pub error: Option<String>,
}

impl Default for NewMapWizard {
    fn default() -> Self {
        Self {
            open: false,
            name: String::new(),
            width: 64,
            height: 64,
            texture_uri: None,
            heightmap: String::new(),
            height_range: 50.0,
            preset: LightingPreset::Outdoor,
            create_requested: false,
            error: None,
        }
    }
}

impl NewMapWizard {
    /// Shows a fresh form whose texture is the index's first ground texture.
    pub fn open(&mut self, index: &AssetIndex) {
        *self = Self {
            open: true,
            texture_uri: index.ground_textures.first().cloned(),
            ..Self::default()
        };
    }

    /// The form as `new_flat` parameters.
    pub fn params(&self) -> Result<NewMapParams, String> {
        let uri = self.texture_uri.clone().ok_or("pick a ground texture")?;
        let name = Path::new(&uri)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("'{uri}' has no file name"))?
            .to_string();
        Ok(NewMapParams {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            texture: TextureRef { name, uri },
            heightmap: (!self.heightmap.is_empty()).then(|| PathBuf::from(&self.heightmap)),
            height_range: self.height_range,
            preset: self.preset,
        })
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NewMapWizard>()
        .add_systems(
            Update,
            (
                open_wizard.run_if(|actions: Res<StartScreenActions>| actions.new_map_requested),
                create_new_map.run_if(|wizard: Res<NewMapWizard>| wizard.create_requested),
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            wizard_modal.in_set(EditorUiSet::Windows),
        );
}

/// Opens the form, unless unsaved changes need confirming first.
fn open_wizard(
    mut actions: ResMut<StartScreenActions>,
    undo: Res<UndoStack>,
    mut guard: ResMut<UnsavedGuard>,
    mut wizard: ResMut<NewMapWizard>,
    index: Res<AssetIndex>,
) {
    actions.new_map_requested = false;
    if undo.is_dirty() {
        guard.0 = Some(GuardedAction::NewMap);
        return;
    }
    wizard.open(&index);
}

/// Builds, writes and opens the new map; a failure stays in the form.
fn create_new_map(
    mut wizard: ResMut<NewMapWizard>,
    source: Res<SharedCompositeAssetSource>,
    paths: Res<EditorPaths>,
    mut opens: MessageWriter<OpenMapRequest>,
) {
    wizard.create_requested = false;
    let source = source.0.read().expect("ro:// source lock poisoned");
    match create(&wizard, &*source, &paths.data_folder) {
        Ok(path) => {
            wizard.open = false;
            wizard.error = None;
            opens.write(OpenMapRequest { path });
        }
        Err(error) => wizard.error = Some(error),
    }
}

/// Writes the wizard's map into `data_folder`; returns its `ro://` path.
fn create(
    wizard: &NewMapWizard,
    source: &dyn AssetSource,
    data_folder: &Path,
) -> Result<String, String> {
    let params = wizard.params()?;
    let name = &params.name;
    if source.exists(&format!("maps/{name}/{name}.glb")) {
        return Err(format!("a map named '{name}' already exists"));
    }
    let doc = MapDocument::new_flat(&params).map_err(|error| format!("{error:#}"))?;
    let bytes = write_map_glb(&doc).map_err(|error| format!("{error:#}"))?;
    let path = map_glb_path(data_folder, name);
    write_atomically(&path, &bytes)
        .map_err(|error| format!("writing {}: {error}", path.display()))?;
    Ok(format!("ro://maps/{name}/{name}.glb"))
}

fn wizard_modal(
    mut contexts: EguiContexts,
    mut wizard: ResMut<NewMapWizard>,
    index: Res<AssetIndex>,
) -> Result {
    if !wizard.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let wizard = &mut *wizard;
    let modal = egui::Modal::new(egui::Id::new("new_map_wizard")).show(ctx, |ui| {
        ui.heading("New map");
        egui::Grid::new("new_map_form")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut wizard.name);
                ui.end_row();
                ui.label("Size (GAT cells)");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut wizard.width).range(2..=MAX_CELLS));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut wizard.height).range(2..=MAX_CELLS));
                });
                ui.end_row();
                ui.label("Ground texture");
                let selected = wizard.texture_uri.as_deref().unwrap_or("none").to_string();
                egui::ComboBox::from_id_salt("new_map_texture")
                    .selected_text(selected.trim_start_matches("ro://ground/tex/"))
                    .show_ui(ui, |ui| {
                        for uri in &index.ground_textures {
                            let label = uri.trim_start_matches("ro://ground/tex/");
                            ui.selectable_value(&mut wizard.texture_uri, Some(uri.clone()), label);
                        }
                    });
                ui.end_row();
                ui.label("Heightmap image");
                ui.text_edit_singleline(&mut wizard.heightmap);
                ui.end_row();
                ui.label("Height range");
                ui.add_enabled(
                    !wizard.heightmap.is_empty(),
                    egui::DragValue::new(&mut wizard.height_range).range(0.0..=1000.0),
                );
                ui.end_row();
                ui.label("Lighting");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut wizard.preset, LightingPreset::Outdoor, "Outdoor");
                    ui.radio_value(&mut wizard.preset, LightingPreset::Indoor, "Indoor");
                });
                ui.end_row();
            });
        if let Some(error) = &wizard.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            wizard.create_requested |= ui.button("Create").clicked();
            ui.button("Cancel").clicked()
        })
        .inner
    });
    if modal.inner || modal.should_close() {
        wizard.open = false;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_names_its_texture_after_the_file_stem() {
        let mut wizard = NewMapWizard::default();
        wizard.open(&AssetIndex {
            ground_textures: vec![
                "ro://ground/tex/grass01.ktx2".into(),
                "ro://ground/tex/sand.ktx2".into(),
            ],
            ..Default::default()
        });
        wizard.name = "field".into();

        let params = wizard.params().unwrap();

        assert_eq!(
            params.texture,
            TextureRef {
                name: "grass01".into(),
                uri: "ro://ground/tex/grass01.ktx2".into(),
            }
        );
        assert_eq!((params.width, params.height), (64, 64));
        assert_eq!(params.preset, LightingPreset::Outdoor);
        assert_eq!(params.heightmap, None);
    }

    #[test]
    fn a_form_without_a_ground_texture_is_refused() {
        let mut wizard = NewMapWizard::default();
        wizard.open(&AssetIndex::default());
        wizard.name = "field".into();

        assert!(wizard.params().is_err());
    }
}
