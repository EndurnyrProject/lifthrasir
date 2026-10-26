//! Autosave recovery snapshots. While a map has unsaved edits the editor
//! periodically writes a copy into the per-user recovery folder; the author's
//! own map file is never touched. A snapshot survives only an interrupted
//! session: save and Discard remove it. On the next start the author may
//! recover it as a new map (never over an existing one) or delete it.

use super::{
    document::collect_document,
    layout::MapLayout,
    open::{EditorScreen, OpenMap, OpenMapRequest},
    save::{map_glb_path, write_atomically},
    session::{EditorSession, editor_config_dir},
};
use crate::{
    EditorPaths,
    shell::{ui::EditorUiSet, undo::UndoStack},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_tooling::converters::map::writer::write_map_glb;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Sidecar of a snapshot glb. Written after the glb, so a snapshot without
/// one is an interrupted write and is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The map's name; also the snapshot's file stem.
    pub name: String,
    /// The `ro://` path the map was opened from.
    pub open_path: String,
    pub saved_at_unix_secs: u64,
}

/// Autosave timer and the recovery folder.
#[derive(Resource, Debug)]
pub struct Autosave {
    dir: PathBuf,
    elapsed: Duration,
    /// The map and undo revision the last snapshot captured.
    last: Option<(String, u64)>,
}

impl Autosave {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            elapsed: Duration::ZERO,
            last: None,
        }
    }

    /// Drops the snapshot of `name`: its edits were saved or discarded.
    pub fn forget(&mut self, name: &str) {
        if self.last.as_ref().is_some_and(|(last, _)| last == name) {
            self.last = None;
        }
        remove_snapshot(&self.dir, name);
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Default for Autosave {
    fn default() -> Self {
        Self::new(editor_config_dir().join("recovery"))
    }
}

/// Snapshots found at startup that the author has not yet decided on.
#[derive(Resource, Default, Debug)]
pub struct RecoveryPrompt {
    pub snapshots: Vec<Snapshot>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Autosave>()
        .init_resource::<RecoveryPrompt>()
        .add_systems(Startup, find_snapshots)
        .add_systems(Update, autosave.run_if(in_state(EditorScreen::Editing)))
        .add_systems(
            EguiPrimaryContextPass,
            recovery_prompt.in_set(EditorUiSet::Windows),
        );
}

fn find_snapshots(
    autosave: Res<Autosave>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    mut prompt: ResMut<RecoveryPrompt>,
) {
    // Like reopening the last map, recovery belongs to the interactive editor.
    if windows.is_empty() {
        return;
    }
    prompt.snapshots = list_snapshots(&autosave.dir);
}

/// Writes a snapshot of the open map once per interval when its edits are
/// newer than the last one. A map whose snapshot is awaiting a recovery
/// decision is skipped, so the interrupted session's copy is not overwritten.
fn autosave(world: &mut World) {
    let delta = world.resource::<Time>().delta();
    let preferences = &world.resource::<EditorSession>().preferences;
    let (enabled, interval) = (
        preferences.autosave_enabled,
        Duration::from_secs(preferences.autosave_interval_secs.into()),
    );
    let mut state = world.resource_mut::<Autosave>();
    state.elapsed += delta;
    if !enabled || state.elapsed < interval {
        return;
    }
    state.elapsed = Duration::ZERO;

    let undo = world.resource::<UndoStack>();
    let (dirty, pristine, revision) = (undo.is_dirty(), undo.is_pristine(), undo.revision());
    let (Some(layout), Some(open)) = (
        world.get_resource::<MapLayout>().cloned(),
        world.get_resource::<OpenMap>().cloned(),
    ) else {
        return;
    };
    let current = Some((layout.name.clone(), revision));
    if !dirty || world.resource::<Autosave>().last == current {
        return;
    }
    if world
        .resource::<RecoveryPrompt>()
        .snapshots
        .iter()
        .any(|snapshot| snapshot.name == layout.name)
    {
        return;
    }
    let bytes = match collect_document(world, &layout, pristine)
        .map_err(|error| error.to_string())
        .and_then(|doc| write_map_glb(&doc).map_err(|error| format!("{error:#}")))
    {
        Ok(bytes) => bytes,
        Err(error) => return warn!("autosave of {} skipped: {error}", layout.name),
    };
    let snapshot = Snapshot {
        name: layout.name.clone(),
        open_path: open.path,
        saved_at_unix_secs: unix_now(),
    };
    let mut state = world.resource_mut::<Autosave>();
    match write_snapshot(&state.dir, &snapshot, &bytes) {
        Ok(()) => state.last = current,
        Err(error) => warn!("autosave of {} failed: {error}", layout.name),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn glb_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.glb"))
}

fn sidecar_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.toml"))
}

/// Writes the glb, then the sidecar that makes it visible.
pub fn write_snapshot(dir: &Path, snapshot: &Snapshot, glb: &[u8]) -> Result<(), String> {
    let sidecar = toml::to_string_pretty(snapshot).map_err(|error| error.to_string())?;
    write_atomically(&glb_path(dir, &snapshot.name), glb).map_err(|error| error.to_string())?;
    write_atomically(&sidecar_path(dir, &snapshot.name), sidecar.as_bytes())
        .map_err(|error| error.to_string())
}

/// Complete snapshots, newest first. Unreadable sidecars are skipped with a
/// warning and left on disk.
pub fn list_snapshots(dir: &Path) -> Vec<Snapshot> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut snapshots: Vec<Snapshot> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .filter_map(|path| {
            let parsed = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|text| toml::from_str::<Snapshot>(&text).map_err(|e| e.to_string()));
            parsed
                .map_err(|error| warn!("reading snapshot {}: {error}", path.display()))
                .ok()
        })
        .filter(|snapshot| glb_path(dir, &snapshot.name).exists())
        .collect();
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.saved_at_unix_secs));
    snapshots
}

/// Removes both files of a snapshot; absent files are not an error.
pub fn remove_snapshot(dir: &Path, name: &str) {
    for path in [sidecar_path(dir, name), glb_path(dir, name)] {
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!("removing snapshot file {}: {error}", path.display());
        }
    }
}

/// Copies the snapshot into the data folder as a new map beside the original
/// (`<name>_recovered`, then `_recovered2`, ...) and removes the snapshot.
/// Returns the new map's `ro://` path. Existing maps are never overwritten.
pub fn recover_snapshot(
    dir: &Path,
    data_folder: &Path,
    snapshot: &Snapshot,
) -> Result<String, String> {
    let bytes = std::fs::read(glb_path(dir, &snapshot.name))
        .map_err(|error| format!("reading snapshot of {}: {error}", snapshot.name))?;
    let name = (1..)
        .map(|attempt| match attempt {
            1 => format!("{}_recovered", snapshot.name),
            n => format!("{}_recovered{n}", snapshot.name),
        })
        .find(|name| !map_glb_path(data_folder, name).exists())
        .expect("an unused recovered name exists");
    let bytes = rename_map_in_glb(&bytes, &snapshot.name, &name)?;
    let target = map_glb_path(data_folder, &name);
    write_atomically(&target, &bytes)
        .map_err(|error| format!("writing {}: {error}", target.display()))?;
    remove_snapshot(dir, &snapshot.name);
    Ok(format!("ro://maps/{name}/{name}.glb"))
}

/// Renames the map inside a glb: the editor finds the map node by the name of
/// the file it was opened from, and the writer names the scene and that node
/// after the map. Only the JSON chunk changes; the binary chunk is copied.
fn rename_map_in_glb(glb: &[u8], old: &str, new: &str) -> Result<Vec<u8>, String> {
    const HEADER: usize = 12;
    const CHUNK_HEADER: usize = 8;
    let word = |at: usize| {
        glb.get(at..at + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| "snapshot is not a valid glb".to_string())
    };
    let json_len = word(HEADER)? as usize;
    let json_end = HEADER + CHUNK_HEADER + json_len;
    let json = glb
        .get(HEADER + CHUNK_HEADER..json_end)
        .ok_or_else(|| "snapshot glb is truncated".to_string())?;
    let mut root: serde_json::Value =
        serde_json::from_slice(json).map_err(|error| format!("snapshot glb JSON: {error}"))?;

    let mut renamed = 0;
    let mut rename = |value: &mut serde_json::Value| {
        if value["name"] == old {
            value["name"] = new.into();
            renamed += 1;
        }
    };
    if let Some(scenes) = root["scenes"].as_array_mut() {
        scenes.iter_mut().for_each(&mut rename);
    }
    // The map node is the scene's root node, not any prop that shares the name.
    let map_node = root["scenes"][0]["nodes"][0].as_u64().map(|n| n as usize);
    if let Some(node) = map_node.and_then(|index| root["nodes"].get_mut(index)) {
        rename(node);
    }
    if renamed < 2 {
        return Err(format!("snapshot glb has no map named '{old}'"));
    }

    let mut json = serde_json::to_vec(&root).map_err(|error| error.to_string())?;
    json.resize(json.len().next_multiple_of(4), b' ');
    let mut out = Vec::with_capacity(glb.len() + new.len());
    out.extend_from_slice(&glb[..HEADER]);
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&glb[HEADER + 4..HEADER + CHUNK_HEADER]);
    out.extend_from_slice(&json);
    out.extend_from_slice(&glb[json_end..]);
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_le_bytes());
    Ok(out)
}

fn describe_age(saved_at: u64) -> String {
    let minutes = unix_now().saturating_sub(saved_at) / 60;
    match minutes {
        0 => "less than a minute ago".into(),
        1..=59 => format!("{minutes} min ago"),
        60..=1439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

/// Lists snapshots of an interrupted session; each can be recovered as a new
/// map or deleted. "Decide later" keeps them for the next start.
fn recovery_prompt(
    mut contexts: EguiContexts,
    mut prompt: ResMut<RecoveryPrompt>,
    autosave: Res<Autosave>,
    paths: Res<EditorPaths>,
    mut opens: MessageWriter<OpenMapRequest>,
    mut errors: ResMut<super::open::ErrorModal>,
) -> Result {
    if prompt.snapshots.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let modal = egui::Modal::new(egui::Id::new("recovery_prompt")).show(ctx, |ui| {
        ui.heading("Recover unsaved work?");
        ui.label("The editor closed with unsaved changes. Recovering opens a copy as a new map; your saved maps are not modified.");
        let mut chosen = None;
        let mut later = false;
        for snapshot in &prompt.snapshots {
            ui.add_space(ui.spacing().item_spacing.y * 2.0);
            ui.label(format!(
                "{} ({})",
                snapshot.name,
                describe_age(snapshot.saved_at_unix_secs)
            ));
            ui.horizontal(|ui| {
                if ui.button("Recover").clicked() {
                    chosen = Some((snapshot.clone(), true));
                }
                if ui.button("Delete").clicked() {
                    chosen = Some((snapshot.clone(), false));
                }
                later |= ui.button("Decide later").clicked();
            });
        }
        (chosen, later)
    });
    let should_close = modal.should_close();
    let (chosen, later) = modal.inner;
    if later || should_close {
        prompt.snapshots.clear();
        return Ok(());
    }
    let Some((snapshot, recover)) = chosen else {
        return Ok(());
    };
    if !recover {
        remove_snapshot(autosave.dir(), &snapshot.name);
    } else {
        match recover_snapshot(autosave.dir(), &paths.data_folder, &snapshot) {
            Ok(path) => {
                opens.write(OpenMapRequest { path });
            }
            Err(error) => {
                errors.show("Could not recover the snapshot", vec![error]);
                return Ok(());
            }
        }
    }
    prompt.snapshots.retain(|other| other.name != snapshot.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(name: &str, at: u64) -> Snapshot {
        Snapshot {
            name: name.into(),
            open_path: format!("ro://maps/{name}/{name}.glb"),
            saved_at_unix_secs: at,
        }
    }

    #[test]
    fn snapshots_list_newest_first_and_skip_interrupted_writes() {
        let dir = tempfile::tempdir().unwrap();
        write_snapshot(dir.path(), &snapshot("old", 10), b"a").unwrap();
        write_snapshot(dir.path(), &snapshot("new", 20), b"b").unwrap();
        // A sidecar whose glb is gone, and a glb with no sidecar.
        write_snapshot(dir.path(), &snapshot("lost", 30), b"c").unwrap();
        std::fs::remove_file(glb_path(dir.path(), "lost")).unwrap();
        std::fs::write(glb_path(dir.path(), "partial"), b"d").unwrap();

        let names: Vec<_> = list_snapshots(dir.path())
            .into_iter()
            .map(|snapshot| snapshot.name)
            .collect();

        assert_eq!(names, ["new", "old"]);
    }

    fn fixture_glb() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/lifthrasir-runtime/tests/fixtures/mini_map.glb");
        std::fs::read(path).unwrap()
    }

    #[test]
    fn recovery_never_overwrites_an_existing_map() {
        let (recovery, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let snapshot = snapshot("mini_map", 1);
        write_atomically(&map_glb_path(data.path(), "mini_map_recovered"), b"keep me").unwrap();
        write_snapshot(recovery.path(), &snapshot, &fixture_glb()).unwrap();

        let path = recover_snapshot(recovery.path(), data.path(), &snapshot).unwrap();

        assert_eq!(
            path,
            "ro://maps/mini_map_recovered2/mini_map_recovered2.glb"
        );
        assert_eq!(
            std::fs::read(map_glb_path(data.path(), "mini_map_recovered")).unwrap(),
            b"keep me"
        );
        assert!(map_glb_path(data.path(), "mini_map_recovered2").exists());
        assert!(list_snapshots(recovery.path()).is_empty());
    }

    #[test]
    fn renaming_a_map_changes_only_its_names() {
        let glb = fixture_glb();

        let renamed = rename_map_in_glb(&glb, "mini_map", "other").unwrap();

        let json = |bytes: &[u8]| {
            let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            serde_json::from_slice::<serde_json::Value>(&bytes[20..20 + len]).unwrap()
        };
        let (before, after) = (json(&glb), json(&renamed));
        assert_eq!(after["scenes"][0]["name"], "other");
        assert_eq!(after["asset"], before["asset"]);
        assert_eq!(
            after["nodes"].as_array().unwrap().len(),
            before["nodes"].as_array().unwrap().len()
        );
        assert_eq!(
            u32::from_le_bytes(renamed[8..12].try_into().unwrap()) as usize,
            renamed.len()
        );
        assert!(rename_map_in_glb(&glb, "nope", "other").is_err());
    }

    #[test]
    fn removing_a_missing_snapshot_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        remove_snapshot(dir.path(), "never_written");
    }
}
