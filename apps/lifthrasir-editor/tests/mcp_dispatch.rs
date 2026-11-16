//! The Bevy side of the MCP: calls wait out human gestures, abandoned calls
//! never run, edits are refused while the editor is busy, and status and
//! undo answer through the same path the server uses.

mod common;
mod mcp_support;

use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
};
use lifthrasir_editor::{
    map::open::EditorOperation,
    mcp::call::{AgentError, Call},
    shell::{
        node::EditorNodeId,
        selection::Selection,
        undo::{ComponentEdit, UndoStack, commit},
    },
};
use lifthrasir_runtime::world::{LifMapData, LifPropRef};
use mcp_support::{call, mcp_app, send};

/// A fixture prop's entity and id.
fn a_prop(app: &mut App) -> (Entity, EditorNodeId) {
    app.world_mut()
        .query_filtered::<(Entity, &EditorNodeId), With<LifPropRef>>()
        .iter(app.world())
        .map(|(entity, id)| (entity, *id))
        .next()
        .unwrap()
}

/// Moves the fixture prop to `x`, as one human edit.
fn move_prop(app: &mut App, x: f32) -> (Entity, Transform) {
    let (entity, id) = a_prop(app);
    let before = *app.world().get::<Transform>(entity).unwrap();
    let after = Transform::from_xyz(x, before.translation.y, before.translation.z);
    commit(
        app.world_mut(),
        Box::new(ComponentEdit::new(id, before, after)),
    );
    (entity, before)
}

#[test]
fn status_without_a_map_reports_null_map() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());

    let status = call(&mut app, &tx, Call::Status).unwrap().json;

    assert!(status["map"].is_null(), "{status}");
    assert_eq!(status["operation"], "idle");
}

#[test]
fn status_reports_the_open_map_and_selection() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let (width, height) = {
        let altitude = &app.world().get::<LifMapData>(layout.root).unwrap().altitude;
        (altitude.width, altitude.height)
    };
    let (_, id) = a_prop(&mut app);
    app.world_mut().resource_mut::<Selection>().0 = Some(id);

    let status = call(&mut app, &tx, Call::Status).unwrap().json;

    assert_eq!(status["map"]["name"], common::FIXTURE_NAME);
    assert_eq!(status["map"]["cells"], serde_json::json!([width, height]));
    assert_eq!(status["selected_node"]["id"], id.0);
    assert_eq!(status["selected_node"]["kind"], "prop");
}

#[test]
fn calls_wait_while_a_mouse_button_is_held() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);

    let mut result = send(&tx, Call::Status);
    app.update();
    assert!(result.try_recv().is_err(), "ran while the button was held");

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    app.update();
    assert!(result.try_recv().unwrap().is_ok());
}

#[test]
fn calls_wait_one_frame_after_a_release() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let mouse = |state| MouseButtonInput {
        button: MouseButton::Left,
        state,
        window: Entity::PLACEHOLDER,
    };
    app.world_mut().write_message(mouse(ButtonState::Pressed));
    app.update();

    let mut result = send(&tx, Call::Status);
    app.world_mut().write_message(mouse(ButtonState::Released));
    app.update();
    assert!(
        result.try_recv().is_err(),
        "ran in the release frame, before the gesture was recorded"
    );

    app.update();
    assert!(result.try_recv().unwrap().is_ok());
}

#[test]
fn agent_redo_keeps_the_original_label() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    move_prop(&mut app, 42.0);
    call(&mut app, &tx, Call::Undo).unwrap();

    let redone = call(&mut app, &tx, Call::Redo).unwrap().json;

    assert_eq!(redone["undo_label"], "Edit Transform");
    let stack = app.world().resource::<UndoStack>();
    assert_eq!(stack.undo_label(), Some("Edit Transform"));
}

#[test]
fn abandoned_calls_are_skipped() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let (entity, _) = move_prop(&mut app, 42.0);

    drop(send(&tx, Call::Undo));
    app.update();

    assert_eq!(
        app.world().get::<Transform>(entity).unwrap().translation.x,
        42.0
    );
    assert_eq!(app.world().resource::<UndoStack>().depth(), 1);
}

#[test]
fn edits_are_busy_while_loading() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    *app.world_mut().resource_mut::<EditorOperation>() = EditorOperation::Loading {
        name: "elsewhere".into(),
    };

    let mut undo = send(&tx, Call::Undo);
    let mut status = send(&tx, Call::Status);
    app.update();

    assert!(matches!(
        undo.try_recv().expect("undo got no reply"),
        Err(AgentError::Busy(_))
    ));
    let status = status.try_recv().expect("status got no reply");
    assert_eq!(status.unwrap().json["operation"], "loading");
}

#[test]
fn agent_undo_reverts_the_last_entry() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let (entity, before) = move_prop(&mut app, 42.0);

    let undone = call(&mut app, &tx, Call::Undo).unwrap().json;

    assert_eq!(*app.world().get::<Transform>(entity).unwrap(), before);
    assert_eq!(undone["redo_label"], "Edit Transform");
    assert!(matches!(
        call(&mut app, &tx, Call::Undo),
        Err(AgentError::Invalid(_))
    ));
}
