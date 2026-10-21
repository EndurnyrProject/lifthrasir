//! The Add light tool: a click places a point or spot light above the terrain
//! with the converter's default values. Also keeps every map light's
//! `intensity` in step with its authored candela.

use super::{TerrainCursor, ToolMode};
use crate::{
    map::{layout::MapLayout, open::EditorScreen},
    shell::{
        gizmo::GizmoChoice,
        node::{EditorNodeId, EditorNodeIds, NodePayload, NodeSnapshot},
        selection::Selection,
        ui::viewport_has_pointer,
        undo::{SpawnNode, commit},
    },
};
use bevy::prelude::*;
use lifthrasir_runtime::world::{LifMapData, MapLightCandela};
use lifthrasir_tooling::converters::map::blank::{LightDefaults, default_light};
use std::f32::consts::{FRAC_PI_2, PI};
use transform_gizmo_bevy::config::GizmoModeKind;

/// How far above the terrain hit a new light sits, in world units.
pub const LIGHT_HEIGHT: f32 = 10.0;

/// The light the Add light tool places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    Point,
    Spot,
}

pub(super) fn plugin(app: &mut App) {
    let adding = |mode: Res<ToolMode>| matches!(*mode, ToolMode::AddLight(_));
    app.add_systems(
        Update,
        (
            add_on_click.run_if(
                in_state(EditorScreen::Editing)
                    .and_then(adding)
                    .and_then(viewport_has_pointer),
            ),
            sync_light_intensity,
        ),
    );
}

/// The spot cone glTF and Bevy accept: `0 < inner <= outer <= PI/2`.
pub fn spot_angles_valid(inner: f32, outer: f32) -> bool {
    0.0 < inner && inner <= outer && outer <= FRAC_PI_2
}

/// The node a new light becomes at `at`: `defaults` with the loader's
/// `intensity = candela * PI * 4.0`; a spot points straight down.
pub fn light_snapshot(
    kind: LightKind,
    defaults: LightDefaults,
    id: EditorNodeId,
    at: Vec3,
) -> NodeSnapshot {
    let color = Color::srgb_from_array(defaults.color);
    let intensity = defaults.candela * PI * 4.0;
    let candela = defaults.candela;
    let (name, rotation, payload) = match kind {
        LightKind::Point => (
            "point_light",
            Quat::IDENTITY,
            NodePayload::PointLight {
                light: PointLight {
                    color,
                    intensity,
                    range: defaults.range,
                    ..default()
                },
                candela,
            },
        ),
        LightKind::Spot => (
            "spot_light",
            // Spot lights shine along local -Z; this turns -Z to -Y.
            Quat::from_rotation_x(-FRAC_PI_2),
            NodePayload::SpotLight {
                light: SpotLight {
                    color,
                    intensity,
                    range: defaults.range,
                    inner_angle: defaults.inner_angle,
                    outer_angle: defaults.outer_angle,
                    ..default()
                },
                candela,
            },
        ),
    };
    NodeSnapshot {
        id,
        name: name.to_string(),
        transform: Transform::from_translation(at).with_rotation(rotation),
        payload,
    }
}

/// Adds a `kind` light [`LIGHT_HEIGHT`] above the terrain point `hit` as one
/// undoable step, with the defaults for the map's indoor flag, selects it and
/// returns to the Select tool with the Move gizmo, ready to position it.
pub fn add_light(world: &mut World, kind: LightKind, hit: Vec3) {
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    let Some(indoor) = world
        .get::<LifMapData>(layout.root)
        .map(|data| data.meta.indoor)
    else {
        return;
    };
    let id = world.resource_mut::<EditorNodeIds>().allocate();
    let at = hit + Vec3::Y * LIGHT_HEIGHT;
    let snapshot = light_snapshot(kind, default_light(indoor), id, at);
    commit(
        world,
        Box::new(SpawnNode::new(layout.root, layout.map_node, snapshot, None)),
    );
    world.resource_mut::<Selection>().0 = Some(id);
    world
        .resource_mut::<ToolMode>()
        .set_if_neq(ToolMode::Select);
    if let Some(mut gizmo) = world.get_resource_mut::<GizmoChoice>() {
        gizmo.set_if_neq(GizmoChoice(GizmoModeKind::Translate));
    }
}

fn add_on_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mode: Res<ToolMode>,
    mut cursor: TerrainCursor,
    mut commands: Commands,
) {
    let ToolMode::AddLight(kind) = *mode else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(hit) = cursor.hit() else {
        return;
    };
    commands.queue(move |world: &mut World| add_light(world, kind, hit));
}

type CandelaLight = (
    &'static MapLightCandela,
    Option<&'static mut PointLight>,
    Option<&'static mut SpotLight>,
);

/// Recomputes `intensity` the way the glTF loader does whenever a light's
/// candela changes, so a candela drag, its commit and its undo all relight it.
fn sync_light_intensity(mut lights: Query<CandelaLight, Changed<MapLightCandela>>) {
    for (candela, point, spot) in &mut lights {
        let intensity = candela.0 * PI * 4.0;
        if let Some(mut point) = point {
            point.intensity = intensity;
        }
        if let Some(mut spot) = spot {
            spot.intensity = intensity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> LightDefaults {
        default_light(false)
    }

    #[test]
    fn a_point_snapshot_carries_the_defaults() {
        let defaults = defaults();
        let snapshot = light_snapshot(LightKind::Point, defaults, EditorNodeId(1), Vec3::ONE);

        let NodePayload::PointLight { light, candela } = snapshot.payload else {
            panic!("expected a point light");
        };
        assert_eq!(candela, defaults.candela);
        assert_eq!(
            light.intensity.to_bits(),
            (defaults.candela * PI * 4.0).to_bits()
        );
        assert_eq!(light.range, defaults.range);
        assert_eq!(snapshot.transform.translation, Vec3::ONE);
    }

    #[test]
    fn a_spot_snapshot_carries_the_defaults_and_points_down() {
        let defaults = defaults();
        let snapshot = light_snapshot(LightKind::Spot, defaults, EditorNodeId(1), Vec3::ZERO);

        let NodePayload::SpotLight { light, candela } = snapshot.payload else {
            panic!("expected a spot light");
        };
        assert_eq!(candela, defaults.candela);
        assert_eq!(
            light.intensity.to_bits(),
            (defaults.candela * PI * 4.0).to_bits()
        );
        assert_eq!(light.range, defaults.range);
        assert_eq!(light.inner_angle, 0.3);
        assert_eq!(light.outer_angle, 0.6);
        let forward = snapshot.transform.rotation * Vec3::NEG_Z;
        assert!(forward.abs_diff_eq(Vec3::NEG_Y, 1e-6), "{forward}");
    }

    #[test]
    fn spot_angles_must_be_ordered_and_within_a_right_angle() {
        assert!(spot_angles_valid(0.3, 0.6));
        assert!(spot_angles_valid(0.6, 0.6));
        assert!(!spot_angles_valid(0.7, 0.6));
        assert!(!spot_angles_valid(0.0, 0.6));
        assert!(!spot_angles_valid(0.3, FRAC_PI_2 + 0.01));
    }

    #[test]
    fn a_candela_change_relights_the_light() {
        let mut world = World::new();
        let light = world
            .spawn((MapLightCandela(1.0), PointLight::default()))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_light_intensity);
        schedule.run(&mut world);

        world.entity_mut(light).insert(MapLightCandela(2.0));
        schedule.run(&mut world);

        assert_eq!(
            world.get::<PointLight>(light).unwrap().intensity.to_bits(),
            (2.0 * PI * 4.0_f32).to_bits()
        );
    }
}
