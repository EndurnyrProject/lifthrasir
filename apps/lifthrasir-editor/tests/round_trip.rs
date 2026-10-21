//! An untouched map collects into a document that writes the input glb byte
//! for byte; edits show up in the document in the native frame.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::map::document::collect_document;
use lifthrasir_runtime::world::LifPropRef;
use lifthrasir_tooling::converters::map::writer::write_map_glb;

#[test]
fn an_untouched_map_saves_byte_for_byte() {
    let mut app = common::runtime_app();
    let (_, layout) = common::load_fixture(&mut app);

    let doc = collect_document(app.world_mut(), &layout, true).unwrap();
    let bytes = write_map_glb(&doc).unwrap();

    let fixture = std::fs::read(common::fixtures_dir().join("mini_map.glb")).unwrap();
    assert!(bytes == fixture, "collected glb differs from the fixture");
}

#[test]
fn a_moved_prop_collects_in_the_native_frame_and_drops_the_source() {
    let mut app = common::runtime_app();
    let (_, layout) = common::load_fixture(&mut app);
    let before = collect_document(app.world_mut(), &layout, true).unwrap();
    let prop = app
        .world_mut()
        .query_filtered::<Entity, With<LifPropRef>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Transform>(prop)
        .unwrap()
        .translation
        .x += 2.0;

    let after = collect_document(app.world_mut(), &layout, false).unwrap();

    assert!(before.meta.source.is_some());
    assert_eq!(
        after.props[0].translation.x,
        before.props[0].translation.x + 2.0
    );
    assert_eq!(after.meta.source, None);
    assert_eq!(after.validate(), Ok(()));
}
