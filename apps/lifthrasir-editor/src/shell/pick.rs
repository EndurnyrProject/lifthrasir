//! Which object node the cursor points at, and the hover/selection outlines.
//!
//! The editor picks on its own instead of through `Pointer<Click>`: the ray
//! only tests object-node meshes and the terrain (so water and effect sprites
//! never block a pick), double-sided props are hit from both sides, and nodes
//! whose origin is a few pixels from the cursor (sound and effect handles,
//! thin props) win when they are not behind the first surface hit.

use super::{
    node::EditorNodeId,
    selection::{Selection, object_node},
    ui::ViewportCursor,
};
use crate::map::{layout::MapLayout, open::EditorScreen, tools::ToolMode};
use bevy::{
    camera::primitives::Aabb,
    ecs::system::SystemParam,
    gizmos::config::GizmoConfigStore,
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastBackfaces},
    prelude::*,
};

/// How close (logical px) the cursor must be to a node's origin to pick it
/// without hitting its mesh.
const PICK_RADIUS: f32 = 12.0;
/// How far behind the first surface hit a node origin may sit and still be
/// picked (one GAT cell), so origins on or just under the ground still count.
const OCCLUSION_SLACK: f32 = 5.0;
const HOVER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const SELECTED_COLOR: Color = Color::srgb(1.0, 0.6, 0.1);
/// Outline radius for a node without mesh bounds (the sun).
const POINT_OUTLINE_RADIUS: f32 = 2.0;

/// The object node under the cursor, if any.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hovered(pub Option<Entity>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Hovered>().add_systems(
        Update,
        (
            pick_backfaces,
            hover.run_if(in_state(EditorScreen::Editing)),
            draw_outlines.run_if(
                in_state(EditorScreen::Editing).and_then(resource_exists::<GizmoConfigStore>),
            ),
        )
            .chain(),
    );
}

/// The node a click means: a node origin within `radius` pixels of the
/// cursor (closest on screen) unless it is more than [`OCCLUSION_SLACK`]
/// behind the first surface hit, otherwise the node of that hit. Candidates
/// are `(node, screen position, camera distance)`; `first_hit` is
/// `(node, ray distance)` with `None` for the terrain.
pub fn pick_target(
    cursor: Vec2,
    first_hit: Option<(Option<Entity>, f32)>,
    candidates: impl IntoIterator<Item = (Entity, Vec2, f32)>,
    radius: f32,
) -> Option<Entity> {
    let near = candidates
        .into_iter()
        .map(|(entity, screen, depth)| (entity, screen.distance(cursor), depth))
        .filter(|(_, pixels, _)| *pixels <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match (near, first_hit) {
        (Some((node, _, depth)), Some((_, distance))) if depth < distance + OCCLUSION_SLACK => {
            Some(node)
        }
        (Some((node, ..)), None) => Some(node),
        (_, hit) => hit.and_then(|(node, _)| node),
    }
}

/// Picks object nodes under the cursor.
#[derive(SystemParam)]
pub struct ObjectPicker<'w, 's> {
    cursor: ViewportCursor<'w, 's>,
    ray_cast: MeshRayCast<'w, 's>,
    parents: Query<'w, 's, &'static ChildOf>,
    children: Query<'w, 's, &'static Children>,
    globals: Query<'w, 's, &'static GlobalTransform>,
    layout: Option<Res<'w, MapLayout>>,
}

impl ObjectPicker<'_, '_> {
    /// The object node under the cursor; `None` over egui panels or without
    /// an open map.
    pub fn pick(&mut self) -> Option<Entity> {
        let layout = self.layout.as_deref()?;
        let (cursor, ray) = self.cursor.ray()?;
        let parents = &self.parents;
        let parent_of = |entity| parents.get(entity).ok().map(ChildOf::parent);
        let on_terrain =
            |entity| layout.terrain_node.is_some() && parent_of(entity) == layout.terrain_node;
        let pickable =
            |entity| on_terrain(entity) || object_node(entity, parent_of, layout).is_some();
        let settings = MeshRayCastSettings::default().with_filter(&pickable);
        let first_hit = self
            .ray_cast
            .cast_ray(ray, &settings)
            .first()
            .map(|(entity, hit)| (object_node(*entity, parent_of, layout), hit.distance));
        let candidates = self
            .children
            .get(layout.map_node)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&node| Some(node) != layout.terrain_node)
            .filter_map(|node| {
                let origin = self.globals.get(node).ok()?.translation();
                let (screen, depth) = self.cursor.to_screen(origin)?;
                Some((node, screen, depth))
            });
        pick_target(cursor, first_hit, candidates, PICK_RADIUS)
    }
}

/// RO props are often double-sided; without this the ray misses them from
/// behind.
fn pick_backfaces(
    mut commands: Commands,
    meshes: Query<Entity, (Added<Mesh3d>, Without<RayCastBackfaces>)>,
) {
    for mesh in &meshes {
        commands.entity(mesh).insert(RayCastBackfaces);
    }
}

/// Tracks the node under the cursor while the Select tool is active.
pub fn hover(mut picker: ObjectPicker, mode: Res<ToolMode>, mut hovered: ResMut<Hovered>) {
    let node = (*mode == ToolMode::Select).then(|| picker.pick()).flatten();
    hovered.set_if_neq(Hovered(node));
}

/// World-space bounds of `entity` and its descendants' meshes.
pub fn bounds_of(
    entity: Entity,
    children: &Query<&Children>,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    std::iter::once(entity)
        .chain(children.iter_descendants(entity))
        .filter_map(|entity| aabbs.get(entity).ok())
        .flat_map(|(aabb, global)| {
            let (center, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            [-1.0, 1.0].into_iter().flat_map(move |x| {
                [-1.0, 1.0].into_iter().flat_map(move |y| {
                    [-1.0, 1.0]
                        .map(move |z| global.transform_point(center + half * Vec3::new(x, y, z)))
                })
            })
        })
        .fold(None, |bounds, point| {
            Some(bounds.map_or((point, point), |(min, max): (Vec3, Vec3)| {
                (min.min(point), max.max(point))
            }))
        })
}

/// [`bounds_of`] as a system, for `World::run_system_cached_with`.
pub fn node_bounds(
    In(entity): In<Entity>,
    children: Query<&Children>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    bounds_of(entity, &children, &aabbs)
}

fn draw_outlines(
    mut gizmos: Gizmos,
    hovered: Res<Hovered>,
    selection: Res<Selection>,
    ids: Query<(Entity, &EditorNodeId)>,
    children: Query<&Children>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
    globals: Query<&GlobalTransform>,
) {
    let selected = selection
        .0
        .and_then(|id| ids.iter().find(|(_, node)| **node == id))
        .map(|(entity, _)| entity);
    let outlines = [(hovered.0, HOVER_COLOR), (selected, SELECTED_COLOR)];
    for (node, color) in outlines {
        let Some(node) = node else {
            continue;
        };
        if let Some((min, max)) = bounds_of(node, &children, &aabbs) {
            let box_transform =
                Transform::from_translation((min + max) * 0.5).with_scale(max - min);
            gizmos.cube(box_transform, color);
        } else if let Ok(global) = globals.get(node) {
            gizmos.sphere(
                Isometry3d::from_translation(global.translation()),
                POINT_OUTLINE_RADIUS,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nearby_origin_wins_unless_it_is_behind_the_first_hit() {
        let [prop, sound, effect] = [1, 2, 3].map(Entity::from_raw_u32).map(Option::unwrap);
        let cursor = Vec2::new(100.0, 100.0);
        let mesh = Some((Some(prop), 50.0));
        let near_sound = (sound, Vec2::new(104.0, 100.0), 30.0);

        assert_eq!(pick_target(cursor, mesh, [], 10.0), Some(prop));
        assert_eq!(pick_target(cursor, mesh, [near_sound], 10.0), Some(sound));
        assert_eq!(
            pick_target(cursor, mesh, [(sound, Vec2::new(104.0, 100.0), 80.0)], 10.0),
            Some(prop)
        );
        assert_eq!(
            pick_target(
                cursor,
                None,
                [near_sound, (effect, Vec2::new(101.0, 101.0), 90.0)],
                10.0
            ),
            Some(effect)
        );
        assert_eq!(
            pick_target(cursor, None, [(sound, Vec2::new(120.0, 100.0), 30.0)], 10.0),
            None
        );
    }

    #[test]
    fn the_terrain_hides_origins_behind_it_but_not_those_on_it() {
        let sound = Entity::from_raw_u32(2).unwrap();
        let cursor = Vec2::new(100.0, 100.0);
        let terrain = Some((None, 50.0));

        assert_eq!(
            pick_target(
                cursor,
                terrain,
                [(sound, Vec2::new(104.0, 100.0), 80.0)],
                10.0
            ),
            None
        );
        assert_eq!(
            pick_target(
                cursor,
                terrain,
                [(sound, Vec2::new(104.0, 100.0), 52.0)],
                10.0
            ),
            Some(sound)
        );
    }

    #[test]
    fn bounds_cover_every_descendant_mesh_in_world_space() {
        let mut world = World::new();
        let aabb = Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0));
        let node = world
            .spawn((
                aabb,
                GlobalTransform::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            ))
            .id();
        world.spawn((
            aabb,
            GlobalTransform::from(Transform::from_xyz(0.0, 5.0, 0.0).with_scale(Vec3::splat(2.0))),
            ChildOf(node),
        ));

        let bounds = world.run_system_cached_with(node_bounds, node).unwrap();

        assert_eq!(
            bounds,
            Some((Vec3::new(-2.0, -1.0, -2.0), Vec3::new(11.0, 7.0, 2.0)))
        );
    }
}
