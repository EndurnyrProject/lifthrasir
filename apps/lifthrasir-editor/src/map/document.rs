//! Turns the live map scene back into the `MapDocument` the tooling writer
//! consumes. An untouched map collects into a document that writes the input
//! glb byte for byte.

use super::layout::{MapLayout, MapTextureList};
use crate::{EditorOnly, shell::node::light_entity};
use bevy::{
    gltf::GltfMaterialName,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
};
use lifthrasir_runtime::world::{
    LifAudioEmitter, LifEffectEmitter, LifMapData, LifPropRef, MapLightCandela,
};
use lifthrasir_tooling::converters::{
    gltf_out::{from_gltf_quat, from_gltf_vec, prop_rotation_from_gltf},
    map::{
        document::{MapDocument, MapLight, MapNode, MapWater},
        terrain::{TerrainPrimitive, chunk_of_quad},
    },
};
use thiserror::Error;

/// Why a map could not be collected or saved.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SaveError {
    #[error("the map is not loaded yet")]
    NotReady,
    #[error("map layout: {0}")]
    Layout(String),
    #[error("light node '{0}' has no MapLightCandela")]
    MissingCandela(String),
    #[error("terrain mesh: {0}")]
    Mesh(String),
    #[error("the map is invalid: {}", .0.join("; "))]
    Invalid(Vec<String>),
    #[error("writing the map: {0}")]
    Io(String),
}

/// Collects the open map. Walks the map node's direct children in order,
/// skipping `EditorOnly`, never descending (a prop's children are the
/// runtime's prop scene). Meta, water and GAT come from the request root's
/// `LifMapData`; `meta.source` survives only while the map is `pristine`.
/// Does not validate; the save path does.
pub fn collect_document(
    world: &mut World,
    layout: &MapLayout,
    pristine: bool,
) -> Result<MapDocument, SaveError> {
    let world: &World = world;
    let data = world
        .get::<LifMapData>(layout.root)
        .ok_or(SaveError::NotReady)?
        .clone();
    let textures = world
        .get::<MapTextureList>(layout.root)
        .ok_or(SaveError::NotReady)?
        .0
        .clone();

    let mut meta = data.meta;
    if !pristine {
        meta.source = None;
    }
    let mut doc = MapDocument {
        name: layout.name.clone(),
        meta,
        textures,
        terrain: Vec::new(),
        lights: Vec::new(),
        props: Vec::new(),
        sounds: Vec::new(),
        effects: Vec::new(),
        water: data.water.map(|zones| MapWater {
            zones,
            tiles: data.water_tiles,
        }),
        gat: data.altitude,
    };

    for node in direct_children(world, layout.map_node) {
        if world.get::<EditorOnly>(node).is_some() {
            continue;
        }
        if Some(node) == layout.terrain_node {
            doc.terrain = collect_terrain(world, node)?;
            continue;
        }
        collect_object(world, node, &mut doc)?;
    }
    Ok(doc)
}

fn direct_children(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map(|children| children.to_vec())
        .unwrap_or_default()
}

fn collect_object(world: &World, node: Entity, doc: &mut MapDocument) -> Result<(), SaveError> {
    let name = world
        .get::<Name>(node)
        .map(Name::to_string)
        .unwrap_or_default();
    let Some(transform) = world.get::<Transform>(node).copied() else {
        return Ok(());
    };
    if let Some(prop) = world.get::<LifPropRef>(node) {
        doc.props.push(MapNode {
            name,
            translation: from_gltf_vec(transform.translation),
            rotation: prop_rotation_from_gltf(transform.rotation),
            scale: transform.scale,
            payload: prop.0.clone(),
        });
    } else if let Some(sound) = world.get::<LifAudioEmitter>(node) {
        doc.sounds
            .push(emitter_node(name, &transform, sound.0.clone()));
    } else if let Some(effect) = world.get::<LifEffectEmitter>(node) {
        doc.effects
            .push(emitter_node(name, &transform, effect.0.clone()));
    } else if let Some(light) = light_entity(world, node) {
        doc.lights
            .push(invert_light(world, &name, &transform, light)?);
    }
    Ok(())
}

fn emitter_node<T>(name: String, transform: &Transform, payload: T) -> MapNode<T> {
    MapNode {
        name,
        translation: from_gltf_vec(transform.translation),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
        payload,
    }
}

/// The node holds the transform, its child the light. Candela comes from
/// `MapLightCandela`, because `intensity / 4π` is not bit-exact.
fn invert_light(
    world: &World,
    name: &str,
    transform: &Transform,
    light: Entity,
) -> Result<MapLight, SaveError> {
    let candela = || {
        world
            .get::<MapLightCandela>(light)
            .map(|candela| candela.0)
            .ok_or_else(|| SaveError::MissingCandela(name.to_string()))
    };
    if let Some(sun) = world.get::<DirectionalLight>(light) {
        return Ok(MapLight::Sun {
            rotation: from_gltf_quat(transform.rotation),
            color: rgb(sun.color),
            lux: sun.illuminance,
        });
    }
    if let Some(point) = world.get::<PointLight>(light) {
        return Ok(MapLight::Point {
            translation: from_gltf_vec(transform.translation),
            color: rgb(point.color),
            candela: candela()?,
            range: point.range,
        });
    }
    let spot = world
        .get::<SpotLight>(light)
        .ok_or_else(|| SaveError::Layout(format!("light node '{name}' lost its light")))?;
    Ok(MapLight::Spot {
        translation: from_gltf_vec(transform.translation),
        rotation: from_gltf_quat(transform.rotation),
        color: rgb(spot.color),
        candela: candela()?,
        range: spot.range,
        inner_angle: spot.inner_angle,
        outer_angle: spot.outer_angle,
    })
}

fn rgb(color: Color) -> [f32; 3] {
    let srgba = color.to_srgba();
    [srgba.red, srgba.green, srgba.blue]
}

/// Every non-empty primitive under the terrain node, in child order.
fn collect_terrain(world: &World, terrain: Entity) -> Result<Vec<TerrainPrimitive>, SaveError> {
    let meshes = world.resource::<Assets<Mesh>>();
    let mut primitives = Vec::new();
    for chunk in direct_children(world, terrain) {
        if world.get::<EditorOnly>(chunk).is_some() {
            continue;
        }
        let (Some(mesh), Some(texture)) = (
            world.get::<Mesh3d>(chunk),
            world.get::<GltfMaterialName>(chunk),
        ) else {
            continue;
        };
        let mesh = meshes
            .get(&mesh.0)
            .ok_or_else(|| SaveError::Mesh(format!("'{}' mesh is not loaded", texture.0)))?;
        if mesh.count_vertices() == 0 {
            continue;
        }
        primitives.push(read_primitive(mesh, &texture.0)?);
    }
    Ok(primitives)
}

/// One terrain primitive back in the native frame. The chunk is derived from
/// the first quad, since the glb does not store it.
pub fn read_primitive(mesh: &Mesh, texture: &str) -> Result<TerrainPrimitive, SaveError> {
    let missing = |attribute: &str| SaveError::Mesh(format!("'{texture}' has no {attribute}"));
    let positions: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(values)) => values
            .iter()
            .map(|p| from_gltf_vec(Vec3::from(*p)).to_array())
            .collect(),
        _ => return Err(missing("Float32x3 POSITION")),
    };
    let normals: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(values)) => values
            .iter()
            .map(|n| from_gltf_vec(Vec3::from(*n)).to_array())
            .collect(),
        _ => return Err(missing("Float32x3 NORMAL")),
    };
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(values)) => values.clone(),
        _ => return Err(missing("Float32x4 COLOR_0")),
    };
    let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(values)) => values.clone(),
        _ => return Err(missing("Float32x2 UV_0")),
    };
    let indices = match mesh.indices() {
        Some(Indices::U32(values)) => values.clone(),
        Some(Indices::U16(values)) => values.iter().map(|&i| u32::from(i)).collect(),
        None => return Err(missing("indices")),
    };
    let first_quad: [[f32; 3]; 4] = positions
        .get(..4)
        .and_then(|quad| quad.try_into().ok())
        .ok_or_else(|| SaveError::Mesh(format!("'{texture}' has fewer than 4 vertices")))?;

    Ok(TerrainPrimitive {
        texture: texture.to_string(),
        chunk: chunk_of_quad(first_quad),
        positions,
        normals,
        colors,
        uvs,
        indices,
    })
}
