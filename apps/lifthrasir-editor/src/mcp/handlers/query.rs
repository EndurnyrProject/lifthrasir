//! Read-only views of the open map in the grids the edit tools take: a
//! summary, per-cell region data, nodes and the asset catalogs.

use super::kind_name;
use crate::{
    map::{
        layout::{MapLayout, MapTextureList},
        open::OpenMap,
        terrain::ground_quad_of,
        tools::gat::{cell_type_label, gat_cell_at},
    },
    mcp::{
        call::{
            AgentError, AgentOutput, AgentResult, AssetKindParam, ListAssetsParams,
            ListNodesParams, NodeKindParam, NodeRef, RegionQuery,
        },
        coords::{
            Region, cell_center, from_wire, map_bounds, region_cells, tile_of_cell, to_tuple,
        },
    },
    shell::{
        browser::AssetIndex,
        node::{EditorNodeId, NodeKind, find_node, kind_of, light_entity},
    },
};
use bevy::{gltf::GltfMaterialName, prelude::*};
use lifthrasir_runtime::world::{
    LifAudioEmitter, LifEffectEmitter, LifMapData, LifPropRef, MapLightCandela,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

/// The largest side `query_region` reads, in cells.
pub const MAX_REGION: u32 = 64;
const DEFAULT_ASSET_LIMIT: u32 = 100;

/// Name, size, lighting, water, node counts and the textures in use.
pub fn map_summary(world: &mut World) -> AgentResult {
    let (layout, data) = open_map(world)?;
    let cells = map_bounds(world)?;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, kind) in nodes(world, &layout) {
        *counts.entry(kind_name(kind)).or_default() += 1;
    }
    let textures: Vec<&str> = world
        .get::<MapTextureList>(layout.root)
        .map(|list| list.0.iter().map(|texture| texture.uri.as_str()).collect())
        .unwrap_or_default();
    let water = data.water.as_ref().map(|water| {
        json!({
            "tiles": [water.width, water.height],
            "zones": water.zones,
            "painted_tiles": data.water_tiles.len(),
        })
    });
    Ok(AgentOutput::json(json!({
        "name": layout.name,
        "cells": [cells.x, cells.y],
        "tiles": [cells.x / 2, cells.y / 2],
        "indoor": data.meta.indoor,
        "ambient_color": data.meta.ambient_color,
        "ambient_brightness": data.meta.ambient_brightness,
        "exposure_ev100": data.meta.exposure_ev100,
        "water": water,
        "nodes": counts,
        "textures": textures,
    })))
}

/// Per cell of a rectangle: GAT corner heights, the walk height at its
/// centre, GAT type, ground texture of its tile and whether its tile has water.
pub fn query_region(world: &mut World, p: RegionQuery) -> AgentResult {
    let (layout, data) = open_map(world)?;
    let (min, max) = (from_wire(p.min), from_wire(p.max));
    let span = max.saturating_sub(min);
    if span.x >= MAX_REGION || span.y >= MAX_REGION {
        return Err(AgentError::Invalid(format!(
            "[{}, {}]..[{}, {}] exceeds {MAX_REGION}x{MAX_REGION} cells; query smaller rectangles",
            min.x, min.y, max.x, max.y
        )));
    }
    let cells = region_cells(
        &Region::Rect {
            min: p.min,
            max: p.max,
        },
        map_bounds(world)?,
    )?;
    let names = texture_uris(world, &layout);
    let mut tile_textures: HashMap<UVec2, Option<String>> = HashMap::new();
    let altitude = &data.altitude;
    let entries: Vec<Value> = cells
        .into_iter()
        .map(|cell| {
            let tile = tile_of_cell(cell);
            let texture = tile_textures
                .entry(tile)
                .or_insert_with(|| tile_texture(world, &names, tile))
                .clone();
            let gat = &altitude.cells[(cell.y * altitude.width + cell.x) as usize];
            let raw = gat.cell_type.raw();
            json!({
                "cell": [cell.x, cell.y],
                "heights": gat.height,
                "walk_y": cell_center(altitude, cell).map(|at| at.y).ok(),
                "gat": { "raw": raw, "label": cell_type_label(raw) },
                "tile_texture": texture,
                "water": data.water_tiles.contains(&(tile.x as usize, tile.y as usize)),
            })
        })
        .collect();
    Ok(AgentOutput::json(json!({ "cells": entries })))
}

/// Every node, optionally of one kind and standing in a cell rectangle.
pub fn list_nodes(world: &mut World, p: ListNodesParams) -> AgentResult {
    let (layout, data) = open_map(world)?;
    let rect = match (p.min, p.max) {
        (None, None) => None,
        (Some(min), Some(max)) => Some((from_wire(min), from_wire(max))),
        _ => {
            return Err(AgentError::Invalid(
                "give both min and max, or neither".into(),
            ));
        }
    };
    let listed: Vec<Value> = nodes(world, &layout)
        .into_iter()
        .filter(|&(_, kind)| p.kind.is_none_or(|wanted| wanted == kind_param(kind)))
        .filter_map(|(entity, kind)| {
            let translation = world.get::<Transform>(entity)?.translation;
            let cell = gat_cell_at(&data.altitude, translation).map(|(x, y)| UVec2::new(x, y));
            if let Some((min, max)) = rect {
                let inside =
                    cell.is_some_and(|cell| cell.cmpge(min).all() && cell.cmple(max).all());
                if !inside {
                    return None;
                }
            }
            Some(json!({
                "id": world.get::<EditorNodeId>(entity)?.0,
                "name": name(world, entity),
                "kind": kind_name(kind),
                "cell": cell.map(|cell| [cell.x, cell.y]),
                "translation": translation.to_array(),
            }))
        })
        .collect();
    Ok(AgentOutput::json(json!({ "nodes": listed })))
}

/// One node with its transform and payload fields.
pub fn get_node(world: &mut World, p: NodeRef) -> AgentResult {
    let (_, data) = open_map(world)?;
    let entity = find_node_or_err(world, p.node_id)?;
    node_dto(world, entity, &data)
        .map(AgentOutput::json)
        .ok_or_else(|| AgentError::NotFound(format!("node {}", p.node_id)))
}

/// A slice of one asset catalog, with the total number of matches.
pub fn list_assets(world: &mut World, p: ListAssetsParams) -> AgentResult {
    let index = world.resource::<AssetIndex>();
    let all: Vec<Value> = match p.kind {
        AssetKindParam::Model => strings(&index.models),
        AssetKindParam::GroundTexture => strings(&index.ground_textures),
        AssetKindParam::Sound => strings(&index.sounds),
        AssetKindParam::Effect => index.effects.iter().map(|&id| json!(id)).collect(),
    };
    let matches: Vec<Value> = all
        .into_iter()
        .filter(|asset| {
            p.filter.as_deref().is_none_or(|filter| match asset {
                Value::String(uri) => uri.contains(filter),
                other => other.to_string().contains(filter),
            })
        })
        .collect();
    let limit = p.limit.unwrap_or(DEFAULT_ASSET_LIMIT) as usize;
    Ok(AgentOutput::json(json!({
        "total": matches.len(),
        "assets": matches.into_iter().take(limit).collect::<Vec<_>>(),
    })))
}

/// The entity of node `id`, or `NotFound("node <id>")`.
pub fn find_node_or_err(world: &mut World, id: u64) -> Result<Entity, AgentError> {
    find_node(world, EditorNodeId(id)).ok_or_else(|| AgentError::NotFound(format!("node {id}")))
}

/// What `get_node` reports about `entity`; `None` when it is not a node.
pub fn node_dto(world: &World, entity: Entity, data: &LifMapData) -> Option<Value> {
    let kind = kind_of(world, entity)?;
    let transform = world.get::<Transform>(entity)?;
    let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
    let cell = gat_cell_at(&data.altitude, transform.translation).map(|(x, y)| [x, y]);
    Some(json!({
        "id": world.get::<EditorNodeId>(entity)?.0,
        "name": name(world, entity),
        "kind": kind_name(kind),
        "cell": cell,
        "translation": transform.translation.to_array(),
        "rotation_deg": [x.to_degrees(), y.to_degrees(), z.to_degrees()],
        "scale": transform.scale.to_array(),
        "payload": payload(world, entity, kind),
    }))
}

fn payload(world: &World, entity: Entity, kind: NodeKind) -> Value {
    match kind {
        NodeKind::Prop => json!(world.get::<LifPropRef>(entity).map(|prop| &prop.0)),
        NodeKind::Sound => json!(world.get::<LifAudioEmitter>(entity).map(|sound| &sound.0)),
        NodeKind::Effect => json!(
            world
                .get::<LifEffectEmitter>(entity)
                .map(|effect| &effect.0)
        ),
        NodeKind::PointLight | NodeKind::SpotLight | NodeKind::Sun => light(world, entity),
    }
}

fn light(world: &World, node: Entity) -> Value {
    let Some(light) = light_entity(world, node) else {
        return Value::Null;
    };
    let candela = world.get::<MapLightCandela>(light).map(|candela| candela.0);
    let rgb = |color: Color| {
        let srgba = color.to_srgba();
        [srgba.red, srgba.green, srgba.blue]
    };
    if let Some(sun) = world.get::<DirectionalLight>(light) {
        return json!({ "color": rgb(sun.color), "lux": sun.illuminance });
    }
    if let Some(spot) = world.get::<SpotLight>(light) {
        return json!({
            "color": rgb(spot.color),
            "candela": candela,
            "range": spot.range,
            "inner_angle_deg": spot.inner_angle.to_degrees(),
            "outer_angle_deg": spot.outer_angle.to_degrees(),
        });
    }
    world.get::<PointLight>(light).map_or(
        Value::Null,
        |point| json!({ "color": rgb(point.color), "candela": candela, "range": point.range }),
    )
}

/// The open map's layout and data, or `NoMapOpen`.
fn open_map(world: &World) -> Result<(MapLayout, LifMapData), AgentError> {
    world
        .get_resource::<OpenMap>()
        .ok_or(AgentError::NoMapOpen)?;
    let layout = world
        .get_resource::<MapLayout>()
        .cloned()
        .ok_or(AgentError::NoMapOpen)?;
    let data = world
        .get::<LifMapData>(layout.root)
        .cloned()
        .ok_or(AgentError::NoMapOpen)?;
    Ok((layout, data))
}

/// The object nodes under the map node, in child order, with their kinds.
fn nodes(world: &World, layout: &MapLayout) -> Vec<(Entity, NodeKind)> {
    world
        .get::<Children>(layout.map_node)
        .into_iter()
        .flat_map(|children| children.iter())
        .filter(|&node| world.get::<EditorNodeId>(node).is_some())
        .filter_map(|node| Some((node, kind_of(world, node)?)))
        .collect()
}

fn texture_uris(world: &World, layout: &MapLayout) -> HashMap<String, String> {
    world
        .get::<MapTextureList>(layout.root)
        .map(|list| {
            list.0
                .iter()
                .map(|texture| (texture.name.clone(), texture.uri.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The texture URI of ground tile `tile`; `None` for a hole.
fn tile_texture(world: &World, names: &HashMap<String, String>, tile: UVec2) -> Option<String> {
    let (chunk, _, _) = ground_quad_of(world, to_tuple(tile))?;
    let name = &world.get::<GltfMaterialName>(chunk)?.0;
    Some(names.get(name).cloned().unwrap_or_else(|| name.clone()))
}

fn name(world: &World, entity: Entity) -> String {
    world
        .get::<Name>(entity)
        .map(Name::to_string)
        .unwrap_or_default()
}

fn strings(list: &[String]) -> Vec<Value> {
    list.iter().map(|uri| json!(uri)).collect()
}

fn kind_param(kind: NodeKind) -> NodeKindParam {
    match kind {
        NodeKind::Prop => NodeKindParam::Prop,
        NodeKind::Sound => NodeKindParam::Sound,
        NodeKind::Effect => NodeKindParam::Effect,
        NodeKind::PointLight => NodeKindParam::PointLight,
        NodeKind::SpotLight => NodeKindParam::SpotLight,
        NodeKind::Sun => NodeKindParam::Sun,
    }
}
