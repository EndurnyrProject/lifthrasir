//! Capture calls headless: refused without a map, and a capture the renderer
//! never delivers (no GPU here) times out and leaves nothing behind. The PNG
//! path itself needs a window and is checked by hand.

mod common;
mod mcp_support;

use bevy::render::view::screenshot::Screenshot;
use lifthrasir_editor::mcp::{
    call::{AgentError, Call, TopDownParams},
    handlers::capture::PendingCapture,
};
use mcp_support::{call, mcp_app, send};

#[test]
fn capture_without_a_map_is_refused() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());

    let result = call(&mut app, &tx, Call::Screenshot);

    assert_eq!(result.unwrap_err(), AgentError::NoMapOpen);
}

#[test]
fn a_top_down_rect_off_the_map_is_refused() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let result = call(
        &mut app,
        &tx,
        Call::RenderTopDown(TopDownParams {
            min: [0, 0],
            max: [99, 0],
        }),
    );

    assert!(
        matches!(result, Err(AgentError::OutOfBounds(_))),
        "{result:?}"
    );
}

#[test]
fn a_capture_that_never_renders_times_out_and_cleans_up() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let mut result = send(
        &tx,
        Call::RenderTopDown(TopDownParams {
            min: [0, 0],
            max: [3, 3],
        }),
    );
    app.update();
    let pending = |app: &mut bevy::prelude::App| {
        app.world_mut()
            .query::<&PendingCapture>()
            .iter(app.world())
            .count()
    };
    assert_eq!(pending(&mut app), 1);
    for _ in 0..400 {
        app.update();
    }

    assert!(matches!(result.try_recv(), Ok(Err(AgentError::Invalid(_)))));
    assert_eq!(pending(&mut app), 0);
    let screenshots = app
        .world_mut()
        .query::<&Screenshot>()
        .iter(app.world())
        .count();
    assert_eq!(screenshots, 0);
}
