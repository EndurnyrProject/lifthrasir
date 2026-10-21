//! The New map wizard writes a loadable, fully walkable map and opens it.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        open::{EditorScreen, OpenMap},
        save::map_glb_path,
        wizard::NewMapWizard,
    },
    shell::browser::AssetIndex,
};
use lifthrasir_runtime::world::LifMapData;
use std::time::{Duration, Instant};

#[test]
fn a_new_map_is_written_loose_and_opens_walkable() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    app.update();
    let index = app.world().resource::<AssetIndex>().clone();
    {
        let mut wizard = app.world_mut().resource_mut::<NewMapWizard>();
        wizard.open(&index);
        wizard.name = "wizard_test".into();
        wizard.width = 32;
        wizard.height = 32;
        wizard.create_requested = true;
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    while *app.world().resource::<State<EditorScreen>>().get() != EditorScreen::Editing {
        assert!(Instant::now() < deadline, "the new map never opened");
        assert_eq!(app.world().resource::<NewMapWizard>().error, None);
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }

    assert!(map_glb_path(data_folder.path(), "wizard_test").exists());
    let world = app.world();
    let open = world.resource::<OpenMap>();
    assert_eq!(open.path, "ro://maps/wizard_test/wizard_test.glb");
    let altitude = &world.get::<LifMapData>(open.root).unwrap().altitude;
    assert_eq!((altitude.width, altitude.height), (32, 32));
    assert!(
        altitude
            .cells
            .iter()
            .all(|cell| cell.cell_type.is_walkable())
    );
}

#[test]
fn an_existing_map_name_is_refused_without_writing() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    app.update();
    let index = app.world().resource::<AssetIndex>().clone();
    {
        let mut wizard = app.world_mut().resource_mut::<NewMapWizard>();
        wizard.open(&index);
        wizard.name = common::FIXTURE_NAME.into();
        wizard.create_requested = true;
    }

    app.update();

    assert!(app.world().resource::<NewMapWizard>().error.is_some());
    assert!(!map_glb_path(data_folder.path(), common::FIXTURE_NAME).exists());
}
