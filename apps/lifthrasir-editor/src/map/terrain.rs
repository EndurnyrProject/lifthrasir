//! The live terrain: GND cell heights read back out of the chunk meshes, the
//! index of chunk entities by `(chunk, texture)`, mesh snapshots, and quad
//! helpers that keep `build_terrain`'s layout (quad `i` = vertices
//! `4i..4i+4`, indices `6i..6i+6`, each quad with its own index pattern).

use super::{layout::MapLayout, open::EditorScreen};
use crate::EditorOnly;
use bevy::{
    asset::RenderAssetUsages,
    gltf::GltfMaterialName,
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};
use lifthrasir_runtime::world::LifMapData;
use lifthrasir_tooling::converters::{
    gltf_out::from_gltf_vec, map::document::TextureRef, map::terrain::chunk_of_quad,
};
use ro_formats::CELL_SIZE;
use std::collections::BTreeMap;

/// Anisotropy both hosts give the glTF loader's default sampler.
const GROUND_ANISOTROPY: u16 = 16;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TerrainChunks>()
        .add_systems(Update, index_terrain.run_if(resource_added::<MapLayout>))
        .add_systems(
            OnExit(EditorScreen::Editing),
            |mut chunks: ResMut<TerrainChunks>| {
                *chunks = TerrainChunks::default();
            },
        );
}

/// Native GND corner heights (positive down) per cell, row-major, in GND
/// corner order `[(0,0), (1,0), (0,1), (1,1)]`. Cells without a ground quad
/// (holes) read `[0.0; 4]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainHeights {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<[f32; 4]>,
}

impl TerrainHeights {
    pub fn get(&self, x: usize, y: usize) -> [f32; 4] {
        self.cells[y * self.width + x]
    }
}

/// The heights of every ground quad of every chunk mesh under the terrain
/// node. Walls (quads without an XZ footprint) are skipped. The GND size is
/// half the GAT size.
pub fn terrain_heights(world: &World, layout: &MapLayout) -> TerrainHeights {
    let (width, height) = world
        .get::<LifMapData>(layout.root)
        .map(|data| {
            (
                data.altitude.width as usize / 2,
                data.altitude.height as usize / 2,
            )
        })
        .unwrap_or_default();
    let mut heights = TerrainHeights {
        width,
        height,
        cells: vec![[0.0; 4]; width * height],
    };
    let meshes = world.resource::<Assets<Mesh>>();
    let chunks = layout
        .terrain_node
        .and_then(|terrain| world.get::<Children>(terrain))
        .into_iter()
        .flat_map(|children| children.iter())
        .filter(|&chunk| world.get::<EditorOnly>(chunk).is_none())
        .filter_map(|chunk| meshes.get(&world.get::<Mesh3d>(chunk)?.0));
    for mesh in chunks {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        for quad in positions.as_chunks::<4>().0 {
            write_ground_quad(&mut heights, native_quad(quad));
        }
    }
    heights
}

/// Stores a ground quad's heights in its cell; vertex order is
/// `[sw, se, ne, nw]`, so GND order is vertices `[0, 1, 3, 2]`.
fn write_ground_quad(heights: &mut TerrainHeights, quad: [Vec3; 4]) {
    let Some((x, y)) = ground_cell(quad) else {
        return;
    };
    let (x, y) = (x as usize, y as usize);
    if x >= heights.width || y >= heights.height {
        return;
    }
    let [sw, se, ne, nw] = quad;
    heights.cells[y * heights.width + x] = [sw.y, se.y, nw.y, ne.y];
}

/// A quad's four glTF-frame positions in the native frame.
pub(crate) fn native_quad(quad: &[[f32; 3]; 4]) -> [Vec3; 4] {
    quad.map(|p| from_gltf_vec(Vec3::from(p)))
}

/// The GND cell of a ground quad (native corners): the minimum of its XZ
/// footprint over the cell size. `None` for a wall, which has no footprint.
pub(crate) fn ground_cell(quad: [Vec3; 4]) -> Option<(u32, u32)> {
    let min = quad.iter().copied().reduce(Vec3::min)?;
    let max = quad.iter().copied().reduce(Vec3::max)?;
    if max.x <= min.x || max.z <= min.z || min.x < 0.0 || min.z < 0.0 {
        return None;
    }
    Some((
        (min.x / CELL_SIZE).floor() as u32,
        (min.z / CELL_SIZE).floor() as u32,
    ))
}

/// The open map's terrain chunk entities and their meshes by
/// `(chunk, texture)`. Chunk entities are never despawned while the map is
/// open; an emptied one is hidden and skipped on save.
#[derive(Resource, Default, Debug)]
pub struct TerrainChunks(BTreeMap<([u32; 2], String), (Entity, Handle<Mesh>)>);

impl TerrainChunks {
    pub fn get(&self, chunk: [u32; 2], texture: &str) -> Option<(Entity, Handle<Mesh>)> {
        self.0.get(&(chunk, texture.to_string())).cloned()
    }

    /// Every chunk (any texture) with `min <= chunk <= max` on both axes.
    pub fn chunks_in(&self, min: [u32; 2], max: [u32; 2]) -> Vec<(Entity, Handle<Mesh>)> {
        self.0
            .iter()
            .filter(|((chunk, _), _)| {
                (0..2).all(|axis| (min[axis]..=max[axis]).contains(&chunk[axis]))
            })
            .map(|(_, entry)| entry.clone())
            .collect()
    }

    pub fn insert(&mut self, chunk: [u32; 2], texture: String, entity: Entity, mesh: Handle<Mesh>) {
        self.0.insert((chunk, texture), (entity, mesh));
    }
}

/// Indexes the open map's terrain chunks. Each chunk gets its own copy of its
/// mesh, so edits never write into the `Gltf` asset's cached primitives (a
/// reopen after Discard would otherwise show the discarded edits).
fn index_terrain(world: &mut World) {
    let Some(terrain) = world
        .get_resource::<MapLayout>()
        .and_then(|layout| layout.terrain_node)
    else {
        world.insert_resource(TerrainChunks::default());
        return;
    };
    let chunks: Vec<(Entity, Handle<Mesh>, String)> = world
        .get::<Children>(terrain)
        .into_iter()
        .flat_map(|children| children.iter())
        .filter(|&chunk| world.get::<EditorOnly>(chunk).is_none())
        .filter_map(|chunk| {
            let mesh = world.get::<Mesh3d>(chunk)?.0.clone();
            let texture = world.get::<GltfMaterialName>(chunk)?.0.clone();
            Some((chunk, mesh, texture))
        })
        .collect();
    let mut index = TerrainChunks::default();
    for (entity, handle, texture) in chunks {
        let Some(mesh) = world.resource::<Assets<Mesh>>().get(&handle).cloned() else {
            error!("terrain chunk '{texture}' mesh is not loaded");
            continue;
        };
        let Some(chunk) = mesh_chunk(&mesh) else {
            continue;
        };
        let own = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world.entity_mut(entity).insert(Mesh3d(own.clone()));
        index.insert(chunk, texture, entity, own);
    }
    world.insert_resource(index);
}

/// The chunk of a terrain mesh, from its first quad; `None` when empty.
fn mesh_chunk(mesh: &Mesh) -> Option<[u32; 2]> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return None;
    };
    let first = positions.as_chunks::<4>().0.first()?;
    Some(chunk_of_quad(
        native_quad(first).map(|corner| corner.to_array()),
    ))
}

/// The attribute arrays and indices of one terrain chunk mesh.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshSnapshot {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// One quad's vertices and its index pattern relative to its first vertex
/// (ground `0,1,2,2,3,0`, walls `0,1,2,2,1,3`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadVerts {
    pub positions: [[f32; 3]; 4],
    pub normals: [[f32; 3]; 4],
    pub colors: [[f32; 4]; 4],
    pub uvs: [[f32; 2]; 4],
    pub pattern: [u32; 6],
}

impl MeshSnapshot {
    /// Reads `mesh`; a missing attribute reads as empty.
    pub fn capture(mesh: &Mesh) -> Self {
        let indices = match mesh.indices() {
            Some(Indices::U32(values)) => values.clone(),
            Some(Indices::U16(values)) => values.iter().map(|&i| u32::from(i)).collect(),
            None => Vec::new(),
        };
        Self {
            positions: float3(mesh, Mesh::ATTRIBUTE_POSITION),
            normals: float3(mesh, Mesh::ATTRIBUTE_NORMAL),
            colors: match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
                Some(VertexAttributeValues::Float32x4(values)) => values.clone(),
                _ => Vec::new(),
            },
            uvs: match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
                Some(VertexAttributeValues::Float32x2(values)) => values.clone(),
                _ => Vec::new(),
            },
            indices,
        }
    }

    /// Writes the snapshot into `mesh` (`u32` indices, as the writer emits).
    pub fn restore(&self, mesh: &mut Mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs.clone());
        mesh.insert_indices(Indices::U32(self.indices.clone()));
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn quad(&self, quad: usize) -> QuadVerts {
        let (v, i) = (quad * 4, quad * 6);
        let first = v as u32;
        QuadVerts {
            positions: self.positions[v..v + 4].try_into().unwrap(),
            normals: self.normals[v..v + 4].try_into().unwrap(),
            colors: self.colors[v..v + 4].try_into().unwrap(),
            uvs: self.uvs[v..v + 4].try_into().unwrap(),
            pattern: std::array::from_fn(|k| self.indices[i + k] - first),
        }
    }

    fn remove_quad(&mut self, quad: usize) -> QuadVerts {
        let removed = self.quad(quad);
        let (v, i) = (quad * 4, quad * 6);
        self.positions.drain(v..v + 4);
        self.normals.drain(v..v + 4);
        self.colors.drain(v..v + 4);
        self.uvs.drain(v..v + 4);
        self.indices.drain(i..i + 6);
        for index in &mut self.indices[i..] {
            *index -= 4;
        }
        removed
    }

    fn append_quad(&mut self, quad: &QuadVerts) {
        let first = self.positions.len() as u32;
        self.positions.extend(quad.positions);
        self.normals.extend(quad.normals);
        self.colors.extend(quad.colors);
        self.uvs.extend(quad.uvs);
        self.indices.extend(quad.pattern.map(|k| first + k));
    }
}

fn float3(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<[f32; 3]> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
        _ => Vec::new(),
    }
}

pub fn quad_count(mesh: &Mesh) -> usize {
    mesh.count_vertices() / 4
}

/// Takes quad `quad` out of `mesh`; later quads' indices move down by 4.
pub fn remove_quad(mesh: &mut Mesh, quad: usize) -> QuadVerts {
    let mut snapshot = MeshSnapshot::capture(mesh);
    let removed = snapshot.remove_quad(quad);
    snapshot.restore(mesh);
    removed
}

/// Adds `quad` after the last quad of `mesh`, re-basing its pattern.
pub fn append_quad(mesh: &mut Mesh, quad: &QuadVerts) {
    let mut snapshot = MeshSnapshot::capture(mesh);
    snapshot.append_quad(quad);
    snapshot.restore(mesh);
}

/// The ground (non-wall) quad of GND cell `cell` in `mesh`.
pub fn ground_quad_at(mesh: &Mesh, cell: (u32, u32)) -> Option<usize> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return None;
    };
    positions
        .as_chunks::<4>()
        .0
        .iter()
        .position(|quad| ground_cell(native_quad(quad)) == Some(cell))
}

/// True when quad `quad` of `mesh` is a wall: it has no XZ footprint.
pub fn is_wall_quad(mesh: &Mesh, quad: usize) -> bool {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return false;
    };
    positions.as_chunks::<4>().0.get(quad).is_some_and(|quad| {
        let [min, max] = native_quad(quad)
            .iter()
            .fold([Vec3::INFINITY, Vec3::NEG_INFINITY], |[min, max], &p| {
                [min.min(p), max.max(p)]
            });
        max.x <= min.x || max.z <= min.z
    })
}

/// The sampler the glTF loader gives a ground texture: the writer's
/// clamp-to-edge, linear filtering, under both hosts' anisotropic default.
fn ground_sampler() -> ImageSamplerDescriptor {
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(GROUND_ANISOTROPY);
    sampler.address_mode_u = ImageAddressMode::ClampToEdge;
    sampler.address_mode_v = ImageAddressMode::ClampToEdge;
    sampler
}

/// Loads a pooled ground texture as the map's own terrain textures load.
pub fn load_ground_texture(asset_server: &AssetServer, uri: &str) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ground_sampler());
        })
        .load(uri.to_string())
}

/// Adds an empty, hidden chunk mesh for `texture` under the terrain node and
/// registers it. Its material is a clone of an existing terrain chunk's with
/// only `base_color_texture` swapped, since every terrain material shares the
/// writer's constants. `None` (logged) without a terrain node or any terrain
/// chunk to clone.
pub fn spawn_chunk(
    world: &mut World,
    layout: &MapLayout,
    chunk: [u32; 2],
    texture: &TextureRef,
) -> Option<(Entity, Handle<Mesh>)> {
    let Some(terrain) = layout.terrain_node else {
        error!(
            "cannot add a '{}' chunk: the map has no terrain node",
            texture.name
        );
        return None;
    };
    let source = world
        .resource::<TerrainChunks>()
        .0
        .values()
        .find_map(|(entity, _)| world.get::<MeshMaterial3d<StandardMaterial>>(*entity))
        .and_then(|material| {
            world
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
        })
        .cloned();
    let Some(mut material) = source else {
        error!(
            "cannot add a '{}' chunk: no terrain material to clone",
            texture.name
        );
        return None;
    };
    material.base_color_texture = Some(load_ground_texture(
        world.resource::<AssetServer>(),
        &texture.uri,
    ));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(material);
    let mesh = world.resource_mut::<Assets<Mesh>>().add(empty_chunk_mesh());
    let entity = world
        .spawn((
            Name::new(texture.name.clone()),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            GltfMaterialName(texture.name.clone()),
            Visibility::Hidden,
            ChildOf(terrain),
        ))
        .id();
    world
        .resource_mut::<TerrainChunks>()
        .insert(chunk, texture.name.clone(), entity, mesh.clone());
    Some((entity, mesh))
}

/// Hides a chunk whose mesh is empty and shows any other.
pub fn sync_chunk_visibility(world: &mut World, chunk: Entity, empty: bool) {
    let visibility = match empty {
        true => Visibility::Hidden,
        false => Visibility::Inherited,
    };
    if let Ok(mut chunk) = world.get_entity_mut(chunk) {
        chunk.insert(visibility);
    }
}

fn empty_chunk_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    MeshSnapshot::default().restore(&mut mesh);
    mesh
}

/// Synthetic grounds and their chunk meshes as the glTF loader hands them over.
#[cfg(test)]
pub(crate) mod test_support {
    use super::MeshSnapshot;
    use bevy::math::Vec3;
    use lifthrasir_tooling::converters::{
        gltf_out::to_gltf_vec,
        map::terrain::{TerrainPrimitive, build_terrain},
    };
    use ro_formats::{GndSurface, GndTile, RoGround};

    /// A one-texture, wall-free ground with per-corner native heights.
    pub fn ground(width: u32, height: u32, corners: impl Fn(usize, usize) -> [f32; 4]) -> RoGround {
        let tile = GndTile {
            u1: 0.0,
            u2: 1.0,
            u3: 0.0,
            u4: 1.0,
            v1: 0.0,
            v2: 0.0,
            v3: 1.0,
            v4: 1.0,
            texture: 0,
            color: [255, 255, 255, 255],
        };
        let surfaces = (0..(width * height) as usize)
            .map(|index| GndSurface {
                height: corners(index % width as usize, index / width as usize),
                tile_up: 0,
                tile_front: -1,
                tile_right: -1,
            })
            .collect();
        RoGround {
            version: "1.7".into(),
            raw_version: 0x0107,
            width,
            height,
            textures: vec!["grass".into()],
            texture_indexes: vec![0],
            tiles: vec![tile],
            surfaces,
            water: None,
        }
    }

    /// The ground's chunk primitives as glTF-frame snapshots, chunk order.
    pub fn chunk_snapshots(ground: &RoGround) -> Vec<MeshSnapshot> {
        let mut primitives = build_terrain(ground).unwrap();
        primitives.sort_by_key(|primitive| primitive.chunk);
        primitives.iter().map(snapshot_of).collect()
    }

    pub fn snapshot_of(primitive: &TerrainPrimitive) -> MeshSnapshot {
        let gltf = |values: &[[f32; 3]]| -> Vec<[f32; 3]> {
            values
                .iter()
                .map(|v| to_gltf_vec(Vec3::from(*v)).to_array())
                .collect()
        };
        MeshSnapshot {
            positions: gltf(&primitive.positions),
            normals: gltf(&primitive.normals),
            colors: primitive.colors.clone(),
            uvs: primitive.uvs.clone(),
            indices: primitive.indices.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_tooling::converters::{
        gltf_out::to_gltf_vec,
        map::terrain::{TerrainPrimitive, build_terrain},
    };
    use ro_formats::{GndSurface, GndTile, RoGround};

    /// A 3 x 3 ground, cell `(x, y)` at height `x + 3y`, with a front wall
    /// under `(0, 0)` and a right wall beside `(1, 1)`.
    fn ground() -> RoGround {
        let tile = GndTile {
            u1: 0.0,
            u2: 1.0,
            u3: 0.0,
            u4: 1.0,
            v1: 0.0,
            v2: 0.0,
            v3: 1.0,
            v4: 1.0,
            texture: 0,
            color: [255, 255, 255, 255],
        };
        let surfaces = (0..9)
            .map(|index| GndSurface {
                height: [index as f32; 4],
                tile_up: 0,
                tile_front: if index == 0 { 0 } else { -1 },
                tile_right: if index == 4 { 0 } else { -1 },
            })
            .collect();
        RoGround {
            version: "1.7".into(),
            raw_version: 0x0107,
            width: 3,
            height: 3,
            textures: vec!["grass".into()],
            texture_indexes: vec![0],
            tiles: vec![tile],
            surfaces,
            water: None,
        }
    }

    /// The primitive as the glTF loader hands it to Bevy.
    fn mesh_of(primitive: &TerrainPrimitive) -> Mesh {
        let gltf = |values: &[[f32; 3]]| -> Vec<[f32; 3]> {
            values
                .iter()
                .map(|v| to_gltf_vec(Vec3::from(*v)).to_array())
                .collect()
        };
        let mut mesh = empty_chunk_mesh();
        MeshSnapshot {
            positions: gltf(&primitive.positions),
            normals: gltf(&primitive.normals),
            colors: primitive.colors.clone(),
            uvs: primitive.uvs.clone(),
            indices: primitive.indices.clone(),
        }
        .restore(&mut mesh);
        mesh
    }

    fn built() -> (TerrainPrimitive, Mesh) {
        let mut primitives = build_terrain(&ground()).unwrap();
        assert_eq!(primitives.len(), 1);
        let primitive = primitives.remove(0);
        let mesh = mesh_of(&primitive);
        (primitive, mesh)
    }

    #[test]
    fn quads_agree_with_build_terrain() {
        let (primitive, mesh) = built();
        assert_eq!(quad_count(&mesh), primitive.positions.len() / 4);
        assert_eq!(quad_count(&mesh), 9 + 2);
        for y in 0..3u32 {
            for x in 0..3u32 {
                let quad = ground_quad_at(&mesh, (x, y)).unwrap();
                let native = &primitive.positions[quad * 4];
                assert_eq!(native[0], x as f32 * CELL_SIZE);
                assert_eq!(native[2], y as f32 * CELL_SIZE);
                assert_eq!(native[1], (x + 3 * y) as f32);
            }
        }
        assert_eq!(ground_quad_at(&mesh, (3, 0)), None);
    }

    #[test]
    fn only_wall_quads_are_walls() {
        let (_, mesh) = built();
        let walls: Vec<usize> = (0..quad_count(&mesh))
            .filter(|&quad| is_wall_quad(&mesh, quad))
            .collect();
        assert_eq!(walls, vec![9, 10]);
    }

    #[test]
    fn a_moved_quad_keeps_its_attributes_and_pattern_and_compacts_indices() {
        let (_, mut source) = built();
        let before = MeshSnapshot::capture(&source);
        let wall = before.quad(9);
        assert_eq!(wall.pattern, [0, 1, 2, 2, 1, 3]);

        let moved = remove_quad(&mut source, 9);
        assert_eq!(moved, wall);
        let after = MeshSnapshot::capture(&source);
        assert_eq!(quad_count(&source), 10);
        assert_eq!(after.indices.len(), 60);
        assert_eq!(after.quad(9), before.quad(10));
        assert_eq!(after.quad(8), before.quad(8));
        assert!(after.indices.iter().all(|&index| index < 40));

        let mut target = mesh_of(&build_terrain(&ground()).unwrap()[0]);
        let ground_quad = remove_quad(&mut target, 0);
        assert_eq!(ground_quad.pattern, [0, 1, 2, 2, 3, 0]);
        append_quad(&mut target, &moved);
        let target = MeshSnapshot::capture(&target);
        assert_eq!(target.quad(10), wall);
        assert_eq!(&target.indices[60..], &[40, 41, 42, 42, 41, 43]);
    }

    fn empty(width: usize, height: usize) -> TerrainHeights {
        TerrainHeights {
            width,
            height,
            cells: vec![[0.0; 4]; width * height],
        }
    }

    #[test]
    fn a_ground_quad_lands_in_its_cell_in_gnd_corner_order() {
        let mut heights = empty(3, 3);
        let (x, z) = (CELL_SIZE, 2.0 * CELL_SIZE);
        write_ground_quad(
            &mut heights,
            [
                Vec3::new(x, 1.0, z),
                Vec3::new(x + CELL_SIZE, 2.0, z),
                Vec3::new(x + CELL_SIZE, 4.0, z + CELL_SIZE),
                Vec3::new(x, 3.0, z + CELL_SIZE),
            ],
        );
        assert_eq!(heights.get(1, 2), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn walls_are_skipped() {
        let mut heights = empty(2, 2);
        let z = CELL_SIZE;
        write_ground_quad(
            &mut heights,
            [
                Vec3::new(0.0, 1.0, z),
                Vec3::new(CELL_SIZE, 1.0, z),
                Vec3::new(0.0, 5.0, z),
                Vec3::new(CELL_SIZE, 5.0, z),
            ],
        );
        assert_eq!(heights, empty(2, 2));
    }
}
