//! Map settings edits are undoable map data edits that re-apply lighting and
//! re-shade props, and never touch light values.

mod common;

use bevy::{camera::Exposure, prelude::*};
use lifthrasir_editor::{
    map::{
        layout::MapLayout,
        settings::{commit_settings, set_indoor},
    },
    shell::{camera::EditorCamera, undo::undo},
};
use lifthrasir_runtime::world::{
    LifMapData, LifPropRef, PropSceneContext, SceneOwner, props::MapModel,
};
use lifthrasir_tooling::converters::map::document::{
    INDOOR_AMBIENT_BRIGHTNESS, INDOOR_EXPOSURE_EV100,
};

fn meta(app: &App, layout: &MapLayout) -> lifthrasir_data::lif::LifMap {
    app.world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .meta
        .clone()
}

fn exposure(app: &mut App) -> f32 {
    app.world_mut()
        .query_filtered::<&Exposure, With<EditorCamera>>()
        .single(app.world())
        .unwrap()
        .ev100
}

/// Every map light component, formatted, in a stable order.
fn lights(app: &mut App, root: Entity) -> Vec<String> {
    let world = app.world_mut();
    let mut lights: Vec<(Entity, String)> = world
        .query::<(
            Entity,
            &SceneOwner,
            AnyOf<(&PointLight, &SpotLight, &DirectionalLight)>,
        )>()
        .iter(world)
        .filter(|(_, owner, _)| owner.0 == root)
        .map(|(entity, _, light)| (entity, format!("{light:?}")))
        .collect();
    lights.sort_by_key(|(entity, _)| *entity);
    lights.into_iter().map(|(_, light)| light).collect()
}

#[test]
fn an_exposure_change_sets_the_camera_until_undone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    app.update();
    let original = exposure(&mut app);
    assert_eq!(original, meta(&app, &layout).exposure_ev100);

    let mut changed = meta(&app, &layout);
    changed.exposure_ev100 = original + 1.5;
    commit_settings(app.world_mut(), changed);

    assert_eq!(exposure(&mut app), original + 1.5);
    undo(app.world_mut());
    assert_eq!(exposure(&mut app), original);
    assert_eq!(meta(&app, &layout).exposure_ev100, original);
}

#[test]
fn going_indoor_sets_the_presets_and_leaves_lights_alone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    app.update();
    assert!(!meta(&app, &layout).indoor);
    let before = lights(&mut app, layout.root);
    assert!(before.len() >= 3, "fixture has a sun, a point and a spot");

    let mut indoor = meta(&app, &layout);
    set_indoor(&mut indoor, true);
    commit_settings(app.world_mut(), indoor);
    app.update();

    let after = meta(&app, &layout);
    assert!(after.indoor);
    assert_eq!(after.ambient_brightness, INDOOR_AMBIENT_BRIGHTNESS);
    assert_eq!(after.exposure_ev100, INDOOR_EXPOSURE_EV100);
    assert_eq!(exposure(&mut app), INDOOR_EXPOSURE_EV100);
    assert_eq!(
        app.world().resource::<GlobalAmbientLight>().brightness,
        INDOOR_AMBIENT_BRIGHTNESS
    );
    assert_eq!(lights(&mut app, layout.root), before);
}

#[test]
fn a_tint_change_reshades_every_prop() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    app.update();
    let props: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<LifPropRef>>()
        .iter(app.world())
        .collect();
    assert!(!props.is_empty());
    for &prop in &props {
        assert!(app.world().get::<MapModel>(prop).is_some());
    }

    let tint = [0.1, 0.2, 0.3];
    let mut tinted = meta(&app, &layout);
    tinted.no_shade_tint = tint;
    commit_settings(app.world_mut(), tinted);

    let context = Some(PropSceneContext {
        no_shade_tint: tint,
    });
    assert_eq!(
        app.world().get::<PropSceneContext>(layout.root).copied(),
        context
    );
    for &prop in &props {
        assert_eq!(app.world().get::<PropSceneContext>(prop).copied(), context);
        assert!(app.world().get::<MapModel>(prop).is_none());
    }
}
