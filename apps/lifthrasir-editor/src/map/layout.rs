//! Where a loaded map lives in the world, and the texture list the meshes
//! alone cannot reproduce.

use super::document::SaveError;
use bevy::{
    gltf::{Gltf, GltfMaterial, GltfSceneName},
    prelude::*,
};
use lifthrasir_tooling::converters::map::document::{TextureRef, is_valid_map_name};
use std::collections::VecDeque;

/// The open map's entities: the request root (holding the authoritative
/// `LifMapData`), the map node whose direct children are the object nodes,
/// and the `terrain` node among them.
#[derive(Resource, Debug, Clone)]
pub struct MapLayout {
    pub root: Entity,
    pub map_node: Entity,
    pub terrain_node: Option<Entity>,
    pub name: String,
}

/// The map's ground textures in material order, on the request root. Seeded
/// from the `Gltf` asset because unused textures and their order are not
/// visible in the terrain meshes.
#[derive(Component, Debug, Clone)]
pub struct MapTextureList(pub Vec<TextureRef>);

/// Name of the object-node child holding the terrain primitives.
pub const TERRAIN_NODE: &str = "terrain";

/// Finds the map node under `root`: the first entity, breadth-first, named
/// `name` that is not the glTF scene root. The writer names the scene after
/// the map as well, and the scene root spawns as `GltfSceneName` + `Name`.
pub fn resolve_layout(world: &World, root: Entity, name: &str) -> Result<MapLayout, SaveError> {
    let map_node = find_map_node(world, root, name)
        .ok_or_else(|| SaveError::Layout(format!("no node named '{name}' under the map root")))?;
    let terrain_node =
        children(world, map_node).find(|&child| has_name(world, child, TERRAIN_NODE));
    Ok(MapLayout {
        root,
        map_node,
        terrain_node,
        name: name.to_string(),
    })
}

fn find_map_node(world: &World, root: Entity, name: &str) -> Option<Entity> {
    let mut queue: VecDeque<Entity> = children(world, root).collect();
    while let Some(entity) = queue.pop_front() {
        if has_name(world, entity, name) && world.get::<GltfSceneName>(entity).is_none() {
            return Some(entity);
        }
        queue.extend(children(world, entity));
    }
    None
}

fn children(world: &World, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
    world
        .get::<Children>(entity)
        .into_iter()
        .flat_map(|children| children.iter())
}

fn has_name(world: &World, entity: Entity, name: &str) -> bool {
    world
        .get::<Name>(entity)
        .is_some_and(|n| n.as_str() == name)
}

/// The glTF materials in index order as texture refs: the name is the
/// material's name, the URI its base color texture's asset path.
pub fn texture_list_from_gltf(
    gltf: &Gltf,
    materials: &Assets<GltfMaterial>,
) -> Result<Vec<TextureRef>, SaveError> {
    gltf.materials
        .iter()
        .enumerate()
        .map(|(index, handle)| {
            let name = gltf
                .named_materials
                .iter()
                .find_map(|(name, named)| (named == handle).then(|| name.to_string()))
                .ok_or_else(|| SaveError::Layout(format!("material {index} has no name")))?;
            let uri = materials
                .get(handle)
                .and_then(|material| material.base_color_texture.as_ref())
                .and_then(|texture| texture.path())
                .map(|path| path.to_string())
                .ok_or_else(|| {
                    SaveError::Layout(format!("material '{name}' has no base color texture path"))
                })?;
            Ok(TextureRef { name, uri })
        })
        .collect()
}

/// `ro://maps/<n>/<n>.glb` -> `n`, when both segments agree and `n` is a valid
/// map name.
pub fn map_name_from_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("ro://maps/")?;
    let (folder, file) = rest.split_once('/')?;
    let stem = file.strip_suffix(".glb")?;
    (folder == stem && is_valid_map_name(stem)).then(|| stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_names_come_from_matching_path_segments() {
        assert_eq!(
            map_name_from_path("ro://maps/prontera/prontera.glb").as_deref(),
            Some("prontera")
        );
        assert_eq!(
            map_name_from_path("ro://maps/1@pump/1@pump.glb").as_deref(),
            Some("1@pump")
        );
        assert_eq!(map_name_from_path("ro://maps/prontera/izlude.glb"), None);
        assert_eq!(map_name_from_path("ro://maps/prontera.glb"), None);
        assert_eq!(map_name_from_path("maps/prontera/prontera.glb"), None);
        assert_eq!(map_name_from_path("ro://maps/a/b/b.glb"), None);
    }
}
