//! Stable identity for map object nodes, and snapshots that respawn a node in
//! the exact entity shape the glTF loader produces.

use bevy::prelude::*;
use lifthrasir_data::lif::{LifAudio, LifEffect, LifProp};
use lifthrasir_runtime::world::{
    LifAudioEmitter, LifEffectEmitter, LifPropRef, MapLightCandela, PropSceneContext, SceneOwner,
    gltf::{tune_map_point_light, tune_map_spot_light},
};
use std::f32::consts::PI;

/// Identifies a map object node across delete and undo, which entity ids do
/// not survive.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorNodeId(pub u64);

/// Hands out [`EditorNodeId`]s, never reusing one within a session.
#[derive(Resource, Default)]
pub struct EditorNodeIds {
    next: u64,
}

impl EditorNodeIds {
    pub fn allocate(&mut self) -> EditorNodeId {
        let id = EditorNodeId(self.next);
        self.next += 1;
        id
    }
}

/// The editable payload of an object node. Lights keep the authored candela,
/// since `intensity` cannot be divided back bit-exactly.
#[derive(Debug, Clone)]
pub enum NodePayload {
    Prop(LifProp),
    Sound(LifAudio),
    Effect(LifEffect),
    PointLight { light: PointLight, candela: f32 },
    SpotLight { light: SpotLight, candela: f32 },
}

/// Everything needed to respawn one object node.
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    pub id: EditorNodeId,
    pub name: String,
    pub transform: Transform,
    pub payload: NodePayload,
}

impl NodeSnapshot {
    /// Captures `node`. `None` when it has no [`EditorNodeId`] or no payload
    /// a snapshot can restore (the terrain, the sun, a light without candela).
    pub fn capture(world: &World, node: Entity) -> Option<NodeSnapshot> {
        let entity = world.get_entity(node).ok()?;
        Some(NodeSnapshot {
            id: *entity.get::<EditorNodeId>()?,
            name: entity
                .get::<Name>()
                .map(Name::to_string)
                .unwrap_or_default(),
            transform: *entity.get::<Transform>()?,
            payload: capture_payload(world, node)?,
        })
    }
}

fn capture_payload(world: &World, node: Entity) -> Option<NodePayload> {
    let entity = world.get_entity(node).ok()?;
    if let Some(prop) = entity.get::<LifPropRef>() {
        return Some(NodePayload::Prop(prop.0.clone()));
    }
    if let Some(sound) = entity.get::<LifAudioEmitter>() {
        return Some(NodePayload::Sound(sound.0.clone()));
    }
    if let Some(effect) = entity.get::<LifEffectEmitter>() {
        return Some(NodePayload::Effect(effect.0.clone()));
    }
    let light = world.get_entity(light_entity(world, node)?).ok()?;
    let candela = light.get::<MapLightCandela>()?.0;
    if let Some(point) = light.get::<PointLight>() {
        return Some(NodePayload::PointLight {
            light: *point,
            candela,
        });
    }
    light.get::<SpotLight>().map(|spot| NodePayload::SpotLight {
        light: *spot,
        candela,
    })
}

/// Spawns `snapshot` as a child of `parent` (at `index` when given) in the
/// shape the glTF loader plus scene adoption produce: the node carries `Name`,
/// `Transform`, `SceneOwner(root)`, the id and the payload; a light sits on a
/// child entity tuned like a loaded map light.
///
/// Props copy the root's `PropSceneContext`, which the runtime inserts when it
/// adopts the scene; editing only starts after that.
pub fn spawn_node(
    world: &mut World,
    root: Entity,
    parent: Entity,
    snapshot: &NodeSnapshot,
    index: Option<usize>,
) -> Entity {
    let context = world.get::<PropSceneContext>(root).copied();
    let node = world
        .spawn((
            Name::new(snapshot.name.clone()),
            snapshot.transform,
            Visibility::default(),
            SceneOwner(root),
            snapshot.id,
        ))
        .id();

    match &snapshot.payload {
        NodePayload::Prop(prop) => {
            let mut entity = world.entity_mut(node);
            entity.insert(LifPropRef(prop.clone()));
            if let Some(context) = context {
                entity.insert(context);
            }
        }
        NodePayload::Sound(sound) => {
            world
                .entity_mut(node)
                .insert(LifAudioEmitter(sound.clone()));
        }
        NodePayload::Effect(effect) => {
            world
                .entity_mut(node)
                .insert(LifEffectEmitter(effect.clone()));
        }
        NodePayload::PointLight { light, candela } => {
            let mut light = *light;
            light.intensity = candela * PI * 4.0;
            tune_map_point_light(&mut light);
            spawn_light_child(
                world,
                root,
                node,
                "point_light",
                (light, MapLightCandela(*candela)),
            );
        }
        NodePayload::SpotLight { light, candela } => {
            let mut light = *light;
            light.intensity = candela * PI * 4.0;
            tune_map_spot_light(&mut light);
            spawn_light_child(
                world,
                root,
                node,
                "spot_light",
                (light, MapLightCandela(*candela)),
            );
        }
    }

    let mut parent = world.entity_mut(parent);
    match index {
        Some(index) => parent.insert_children(index, &[node]),
        None => parent.add_child(node),
    };
    node
}

/// The light child, named like the writer names its KHR lights.
fn spawn_light_child(
    world: &mut World,
    root: Entity,
    node: Entity,
    name: &'static str,
    light: impl Bundle,
) {
    world.spawn((Name::new(name), light, SceneOwner(root), ChildOf(node)));
}

/// The live entity carrying `id`, if any.
pub fn find_node(world: &mut World, id: EditorNodeId) -> Option<Entity> {
    world
        .query::<(Entity, &EditorNodeId)>()
        .iter(world)
        .find_map(|(entity, node)| (*node == id).then_some(entity))
}

/// The child of `node` holding its `DirectionalLight`/`PointLight`/`SpotLight`.
pub fn light_entity(world: &World, node: Entity) -> Option<Entity> {
    world.get::<Children>(node)?.iter().find(|&child| {
        world.get::<DirectionalLight>(child).is_some()
            || world.get::<PointLight>(child).is_some()
            || world.get::<SpotLight>(child).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_snapshot(id: EditorNodeId) -> NodeSnapshot {
        NodeSnapshot {
            id,
            name: "lamp".into(),
            transform: Transform::from_xyz(1.0, 2.0, 3.0),
            payload: NodePayload::PointLight {
                light: PointLight::default(),
                candela: 123.456,
            },
        }
    }

    #[test]
    fn spawned_point_light_matches_the_loaded_light_shape() {
        let mut world = World::new();
        let root = world.spawn(PropSceneContext::default()).id();
        let parent = world.spawn_empty().id();
        let snapshot = point_snapshot(EditorNodeId(7));

        let node = spawn_node(&mut world, root, parent, &snapshot, None);

        assert_eq!(world.get::<Transform>(node), Some(&snapshot.transform));
        assert_eq!(world.get::<SceneOwner>(node), Some(&SceneOwner(root)));
        assert_eq!(world.get::<EditorNodeId>(node), Some(&EditorNodeId(7)));
        assert_eq!(
            world.get::<ChildOf>(node).map(ChildOf::parent),
            Some(parent)
        );
        let light = light_entity(&world, node).unwrap();
        let point = world.get::<PointLight>(light).unwrap();
        assert_eq!(point.radius, 1.5);
        assert!(!point.shadow_maps_enabled);
        assert_eq!(
            point.intensity.to_bits(),
            (123.456_f32 * std::f32::consts::PI * 4.0).to_bits()
        );
        assert_eq!(
            world.get::<MapLightCandela>(light),
            Some(&MapLightCandela(123.456))
        );
    }

    #[test]
    fn capture_round_trips_a_spawned_light() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let parent = world.spawn_empty().id();
        let node = spawn_node(
            &mut world,
            root,
            parent,
            &point_snapshot(EditorNodeId(1)),
            None,
        );

        let captured = NodeSnapshot::capture(&world, node).unwrap();

        assert_eq!(captured.id, EditorNodeId(1));
        assert_eq!(captured.name, "lamp");
        assert!(matches!(
            captured.payload,
            NodePayload::PointLight { candela, .. } if candela == 123.456
        ));
    }

    #[test]
    fn ids_are_never_reused() {
        let mut ids = EditorNodeIds::default();
        assert_ne!(ids.allocate(), ids.allocate());
    }
}
