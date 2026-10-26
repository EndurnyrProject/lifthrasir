//! Autosave writes recovery snapshots of unsaved edits without touching the
//! map file; saving removes them.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        autosave::{list_snapshots, recover_snapshot},
        open::OpenMapRequest,
        save::{SaveRequest, map_glb_path},
        session::EditorSession,
    },
    shell::undo::{Batch, UndoStack, record},
};

fn tick(app: &mut App) {
    // A zero interval makes
    // every update eligible.
    app.world_mut()
        .resource_mut::<EditorSession>()
        .preferences
        .autosave_interval_secs = 0;
    app.update();
}

#[test]
fn unsaved_edits_are_snapshotted_and_saving_removes_the_snapshot() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let recovery = data_folder.path().join(".recovery");

    tick(&mut app);
    assert!(
        list_snapshots(&recovery).is_empty(),
        "clean maps are skipped"
    );

    record(app.world_mut(), Box::new(Batch(Vec::new())));
    assert!(app.world().resource::<UndoStack>().is_dirty());
    tick(&mut app);

    let snapshots = list_snapshots(&recovery);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].name, layout.name);
    assert!(
        !map_glb_path(data_folder.path(), &layout.name).exists(),
        "autosave must not write the map file"
    );

    app.world_mut().write_message(SaveRequest { save_as: None });
    app.update();
    assert!(list_snapshots(&recovery).is_empty());
}

#[test]
fn a_recovered_snapshot_opens_as_a_renamed_map() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let recovery = data_folder.path().join(".recovery");
    record(app.world_mut(), Box::new(Batch(Vec::new())));
    tick(&mut app);
    let snapshot = list_snapshots(&recovery).remove(0);

    let path = recover_snapshot(&recovery, data_folder.path(), &snapshot).unwrap();
    app.world_mut().resource_mut::<UndoStack>().clear();
    app.world_mut().write_message(OpenMapRequest { path });
    // Let the open request move the editor out of `Editing` first.
    app.update();
    app.update();
    let recovered = common::wait_for_editing(&mut app);

    assert_eq!(recovered.name, format!("{}_recovered", layout.name));
}
