//! Session tools over MCP: listing, opening and creating maps (refused while
//! there are unsaved edits) and saving with errors returned to the agent, not
//! shown to the human.

mod common;
mod mcp_support;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        open::{ErrorModal, OpenMap, UnsavedGuard},
        save::map_glb_path,
    },
    mcp::call::{AgentError, Call, NewMapToolParams, OpenMapParams, PresetParam},
    shell::{
        browser::{AssetIndex, AssetRef},
        undo::UndoStack,
    },
};
use mcp_support::{call, mcp_app, send};

fn new_map(name: &str, app: &App) -> Call {
    let texture = app
        .world()
        .resource::<AssetIndex>()
        .ground_textures
        .first()
        .cloned()
        .expect("the fixture pak has a ground texture");
    Call::NewMap(NewMapToolParams {
        name: name.into(),
        width: 32,
        height: 32,
        texture,
        preset: PresetParam::Outdoor,
    })
}

/// One committed edit, so the map is dirty.
fn dirty(app: &mut App) {
    lifthrasir_editor::map::tools::place::spawn_placed(
        app.world_mut(),
        &AssetRef::Model("ro://models/prontera/tree01.glb".into()),
        Vec3::new(10.0, 0.0, -10.0),
    )
    .unwrap();
}

#[test]
fn list_maps_includes_the_fixture() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());

    let maps = call(&mut app, &tx, Call::ListMaps).unwrap().json;

    let names: Vec<&str> = maps["maps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|map| map.as_str())
        .collect();
    assert!(names.contains(&common::FIXTURE_NAME), "{maps}");
}

#[test]
fn open_and_new_are_refused_while_dirty() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    dirty(&mut app);
    let depth = app.world().resource::<UndoStack>().depth();

    let open = call(
        &mut app,
        &tx,
        Call::OpenMap(OpenMapParams {
            path: common::FIXTURE_MAP_PATH.into(),
        }),
    );
    let fresh = new_map("fresh", &app);
    let new = call(&mut app, &tx, fresh);

    assert!(matches!(open, Err(AgentError::Guarded(_))), "{open:?}");
    assert!(matches!(new, Err(AgentError::Guarded(_))), "{new:?}");
    app.update();
    assert_eq!(
        app.world().resource::<OpenMap>().path,
        common::FIXTURE_MAP_PATH
    );
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
    assert!(app.world().resource::<UnsavedGuard>().0.is_none());
    assert!(!map_glb_path(data_folder.path(), "fresh").exists());
}

#[test]
fn new_map_with_an_existing_name_is_invalid() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    app.update();

    let existing = new_map(common::FIXTURE_NAME, &app);
    let result = call(&mut app, &tx, existing);

    let Err(AgentError::Invalid(message)) = result else {
        panic!("expected Invalid, got {result:?}");
    };
    assert!(message.contains("already exists"), "{message}");
    assert!(!map_glb_path(data_folder.path(), common::FIXTURE_NAME).exists());
    app.update();
    assert!(app.world().get_resource::<OpenMap>().is_none());
}

#[test]
fn new_map_with_oversized_dimensions_is_invalid() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    app.update();
    let Call::NewMap(mut params) = new_map("huge", &app) else {
        unreachable!()
    };
    params.width = 131_070;

    let result = call(&mut app, &tx, Call::NewMap(params));

    assert!(matches!(result, Err(AgentError::Invalid(_))), "{result:?}");
    assert!(!map_glb_path(data_folder.path(), "huge").exists());
}

#[test]
fn a_queued_redo_never_raises_the_human_guard_over_an_agent_open() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    dirty(&mut app);
    call(&mut app, &tx, Call::Undo).unwrap();

    let mut open = send(
        &tx,
        Call::OpenMap(OpenMapParams {
            path: common::FIXTURE_MAP_PATH.into(),
        }),
    );
    let mut redo = send(&tx, Call::Redo);
    let (mut opened, mut redone) = (None, None);
    for _ in 0..20 {
        app.update();
        opened = opened.or_else(|| open.try_recv().ok());
        redone = redone.or_else(|| redo.try_recv().ok());
    }

    assert!(matches!(opened, Some(Ok(_))), "{opened:?}");
    assert!(app.world().resource::<UnsavedGuard>().0.is_none());
    assert!(
        matches!(redone, Some(Err(AgentError::Busy(_)))),
        "{redone:?}"
    );
}

#[test]
fn new_map_writes_and_starts_opening() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    app.update();

    let made = new_map("agent_made", &app);
    let started = call(&mut app, &tx, made).unwrap().json;

    assert_eq!(started["started"], true);
    assert!(map_glb_path(data_folder.path(), "agent_made").exists());
    common::wait_for_editing(&mut app);
    let status = call(&mut app, &tx, Call::Status).unwrap().json;
    assert_eq!(status["map"]["name"], "agent_made");
}

#[test]
fn save_map_writes_and_clears_dirty() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    dirty(&mut app);

    let saved = call(&mut app, &tx, Call::SaveMap).unwrap().json;

    assert_eq!(saved["saved"], true, "{saved}");
    assert!(map_glb_path(data_folder.path(), common::FIXTURE_NAME).exists());
    let status = call(&mut app, &tx, Call::Status).unwrap().json;
    assert_eq!(status["dirty"], false);
}

#[test]
fn save_map_reports_validation_errors_without_a_modal() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    lifthrasir_editor::map::tools::place::spawn_placed(
        app.world_mut(),
        &AssetRef::Model("ro://models/does_not_exist.glb".into()),
        Vec3::new(10.0, 0.0, -10.0),
    );

    let result = call(&mut app, &tx, Call::SaveMap);

    assert!(matches!(result, Err(AgentError::Invalid(_))), "{result:?}");
    assert!(app.world().resource::<ErrorModal>().messages.is_empty());
    assert!(!map_glb_path(data_folder.path(), common::FIXTURE_NAME).exists());
}
