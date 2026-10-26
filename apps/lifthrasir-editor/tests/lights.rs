//! The Add light tool places a light the document collects with the
//! converter's defaults; the inspector refuses an inverted spot cone.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        tools::{
            ToolMode,
            lights::{LightKind, add_light, spot_angles_valid},
        },
    },
    shell::{selection::Selection, undo::undo},
};
use lifthrasir_tooling::converters::{
    gltf_out::from_gltf_vec,
    map::{blank::default_light, document::MapLight},
};

fn spots(lights: &[MapLight]) -> Vec<&MapLight> {
    lights
        .iter()
        .filter(|light| matches!(light, MapLight::Spot { .. }))
        .collect()
}

#[test]
fn a_placed_spot_light_collects_with_the_defaults_until_undone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = collect_document(app.world_mut(), &layout, true).unwrap();

    add_light(
        app.world_mut(),
        LightKind::Spot,
        Vec3::new(10.0, 0.0, -10.0),
    );

    let doc = collect_document(app.world_mut(), &layout, false).unwrap();
    assert_eq!(doc.lights.len(), before.lights.len() + 1);
    assert_eq!(spots(&doc.lights).len(), spots(&before.lights).len() + 1);
    let Some(MapLight::Spot {
        translation,
        rotation,
        candela,
        range,
        inner_angle,
        outer_angle,
        ..
    }) = doc.lights.last()
    else {
        panic!("the placed light is not the last spot");
    };
    let defaults = default_light(doc.meta.indoor);
    assert_eq!(*candela, defaults.candela);
    assert_eq!(*range, defaults.range);
    assert_eq!(*inner_angle, defaults.inner_angle);
    assert_eq!(*outer_angle, defaults.outer_angle);
    assert_eq!(*translation, from_gltf_vec(Vec3::new(10.0, 10.0, -10.0)));
    // The native rotation is `ROOT_FIX * q`: it sends the light's local -Z
    // straight to the native frame.
    let native_down = from_gltf_vec(Vec3::NEG_Y);
    assert!(
        (*rotation * Vec3::NEG_Z).abs_diff_eq(native_down, 1e-6),
        "{rotation:?}"
    );
    assert_eq!(doc.validate(), Ok(()));

    undo(app.world_mut());
    let undone = collect_document(app.world_mut(), &layout, false).unwrap();
    assert_eq!(undone.lights, before.lights);
}

#[test]
fn the_inspector_rejects_an_inner_angle_past_the_outer() {
    let defaults = default_light(false);
    assert!(spot_angles_valid(
        defaults.inner_angle,
        defaults.outer_angle
    ));
    assert!(!spot_angles_valid(
        defaults.outer_angle + 0.1,
        defaults.outer_angle
    ));
}

#[test]
fn placing_a_light_returns_to_select_with_the_light_selected() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    *app.world_mut().resource_mut::<ToolMode>() = ToolMode::AddLight(LightKind::Point);

    add_light(app.world_mut(), LightKind::Point, Vec3::ZERO);

    assert_eq!(*app.world().resource::<ToolMode>(), ToolMode::Select);
    assert!(app.world().resource::<Selection>().active().is_some());
}
