//! The Paint tile tool: moves a clicked cell's ground quad into the brush
//! texture's chunk mesh with full-texture UVs, turned in quarter steps and
//! optionally mirrored. A drag paints every cell it crosses as one `Batch`.

use super::{TerrainCursor, ToolMode};
use crate::{
    map::{
        layout::{MapLayout, MapTextureList},
        open::EditorScreen,
        terrain::{
            MeshSnapshot, TerrainChunks, append_quad, ground_quad_at, remove_quad, spawn_chunk,
            sync_chunk_visibility,
        },
    },
    shell::{
        browser::{AssetRef, BrowserSelection},
        ui::EditorUiSet,
        undo::{Batch, EditOp, MeshChunkEdit, record},
    },
};
use bevy::{gizmos::config::GizmoConfigStore, gltf::GltfMaterialName, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_tooling::converters::map::{document::TextureRef, terrain::CHUNK_CELLS};
use ro_formats::CELL_SIZE;
use std::{
    collections::{BTreeMap, BTreeSet},
    f32::consts::FRAC_PI_2,
    path::Path,
};

const CELL_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
/// Full-texture UVs in vertex order `[sw, se, ne, nw]`, unturned.
const BASE_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// The tile brush: a ground texture, its quarter turns, and a mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileBrush {
    pub texture_uri: String,
    pub rotation: u8,
    pub flip: bool,
}

impl TileBrush {
    pub fn new(texture_uri: String) -> Self {
        Self {
            texture_uri,
            rotation: 0,
            flip: false,
        }
    }
}

/// The stroke in progress: the cells painted so far and each chunk's mesh
/// before its first change.
#[derive(Default)]
struct Stroke {
    painted: BTreeSet<(u32, u32)>,
    before: BTreeMap<Entity, (Handle<Mesh>, MeshSnapshot)>,
}

#[derive(Resource, Default)]
struct TileStroke(Option<Stroke>);

pub(super) fn plugin(app: &mut App) {
    let painting = |mode: Res<ToolMode>| matches!(*mode, ToolMode::PaintTile(_));
    app.init_resource::<TileStroke>()
        .add_systems(
            Update,
            (
                follow_browser.run_if(painting),
                stroke,
                draw_cell.run_if(painting.and_then(resource_exists::<GizmoConfigStore>)),
            )
                .chain()
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            options_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing).and_then(painting)),
        )
        .add_systems(
            OnExit(EditorScreen::Editing),
            |mut stroke: ResMut<TileStroke>| stroke.0 = None,
        );
}

/// UVs covering the whole texture in vertex order `[sw, se, ne, nw]`, turned
/// by `rotation` quarter steps (mod 4), then mirrored in U when `flip`.
/// Rotation 0 is `[(0,0), (1,0), (1,1), (0,1)]`.
pub fn tile_uvs(rotation: u8, flip: bool) -> [[f32; 2]; 4] {
    std::array::from_fn(|corner| {
        let [u, v] = BASE_UVS[(corner + rotation as usize) % 4];
        match flip {
            true => [1.0 - u, v],
            false => [u, v],
        }
    })
}

/// Moves quad `quad` of `src` to the end of `dst`, keeping its positions,
/// normals, colors and index pattern and replacing its UVs.
pub fn move_quad(src: &mut Mesh, quad: usize, dst: &mut Mesh, uvs: [[f32; 2]; 4]) {
    let mut moved = remove_quad(src, quad);
    moved.uvs = uvs;
    append_quad(dst, &moved);
}

/// The map texture for `uri`, appended (named after the file stem, with
/// `_2`, `_3`... when another URI holds that name) if the map lacks it.
pub fn ensure_texture(list: &mut MapTextureList, uri: &str) -> TextureRef {
    if let Some(known) = list.0.iter().find(|texture| texture.uri == uri) {
        return known.clone();
    }
    let stem = Path::new(uri)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let taken = |name: &str| list.0.iter().any(|texture| texture.name == name);
    let name = (1..)
        .map(|n| match n {
            1 => stem.clone(),
            n => format!("{stem}_{n}"),
        })
        .find(|name| !taken(name))
        .expect("an unbounded suffix range always has a free name");
    let texture = TextureRef {
        name,
        uri: uri.to_string(),
    };
    list.0.push(texture.clone());
    texture
}

/// Starts a stroke.
pub fn begin_stroke(world: &mut World) {
    world.resource_mut::<TileStroke>().0 = Some(Stroke::default());
}

/// Paints GND cell `cell` with `brush` in the stroke in progress, once per
/// stroke. Holes, walls and cells off the map are left alone.
pub fn paint_cell(world: &mut World, cell: (u32, u32), brush: &TileBrush) {
    let Some(mut stroke) = world.resource_mut::<TileStroke>().0.take() else {
        return;
    };
    if stroke.painted.insert(cell) {
        paint(world, &mut stroke, cell, brush);
    }
    world.resource_mut::<TileStroke>().0 = Some(stroke);
}

/// Ends the stroke, recording every chunk it changed as one undoable step.
pub fn end_stroke(world: &mut World) {
    let Some(stroke) = world.resource_mut::<TileStroke>().0.take() else {
        return;
    };
    let meshes = world.resource::<Assets<Mesh>>();
    let ops: Vec<Box<dyn EditOp>> = stroke
        .before
        .into_iter()
        .filter_map(|(entity, (handle, before))| {
            let after = MeshSnapshot::capture(meshes.get(&handle)?);
            (after != before).then(|| {
                Box::new(MeshChunkEdit::new(entity, handle, before, after)) as Box<dyn EditOp>
            })
        })
        .collect();
    if !ops.is_empty() {
        record(world, Box::new(Batch(ops)));
    }
}

fn paint(world: &mut World, stroke: &mut Stroke, cell: (u32, u32), brush: &TileBrush) {
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    let chunk = [cell.0 / CHUNK_CELLS, cell.1 / CHUNK_CELLS];
    let Some((source, source_mesh, quad)) = ground_quad_of(world, chunk, cell) else {
        return;
    };
    let Some(texture) = world
        .get_mut::<MapTextureList>(layout.root)
        .map(|mut list| ensure_texture(&mut list, &brush.texture_uri))
    else {
        return;
    };
    let uvs = tile_uvs(brush.rotation, brush.flip);
    remember(world, stroke, source, &source_mesh);
    let same_texture = world
        .get::<GltfMaterialName>(source)
        .is_some_and(|name| name.0 == texture.name);
    if same_texture {
        retexture(world, &source_mesh, quad, uvs);
        return;
    }
    let target = world.resource::<TerrainChunks>().get(chunk, &texture.name);
    let Some((target, target_mesh)) =
        target.or_else(|| spawn_chunk(world, &layout, chunk, &texture))
    else {
        return;
    };
    remember(world, stroke, target, &target_mesh);
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let Some(mut src) = meshes.get(&source_mesh).cloned() else {
        return;
    };
    let Some(mut dst) = meshes.get_mut(&target_mesh) else {
        return;
    };
    move_quad(&mut src, quad, &mut dst, uvs);
    drop(dst);
    let source_empty = src.count_vertices() == 0;
    if let Some(mut mesh) = meshes.get_mut(&source_mesh) {
        *mesh = src;
    }
    sync_chunk_visibility(world, source, source_empty);
    sync_chunk_visibility(world, target, false);
}

/// The chunk mesh (any texture) of `chunk` holding the ground quad of `cell`.
fn ground_quad_of(
    world: &World,
    chunk: [u32; 2],
    cell: (u32, u32),
) -> Option<(Entity, Handle<Mesh>, usize)> {
    let meshes = world.resource::<Assets<Mesh>>();
    world
        .resource::<TerrainChunks>()
        .chunks_in(chunk, chunk)
        .into_iter()
        .find_map(|(entity, handle)| {
            let quad = ground_quad_at(meshes.get(&handle)?, cell)?;
            Some((entity, handle, quad))
        })
}

/// Keeps `chunk`'s mesh as it was before the stroke first changed it.
fn remember(world: &World, stroke: &mut Stroke, chunk: Entity, mesh: &Handle<Mesh>) {
    if stroke.before.contains_key(&chunk) {
        return;
    }
    let Some(snapshot) = world
        .resource::<Assets<Mesh>>()
        .get(mesh)
        .map(MeshSnapshot::capture)
    else {
        return;
    };
    stroke.before.insert(chunk, (mesh.clone(), snapshot));
}

/// Replaces the UVs of quad `quad` in place (the cell already has the texture).
fn retexture(world: &mut World, mesh: &Handle<Mesh>, quad: usize, uvs: [[f32; 2]; 4]) {
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let Some(mut mesh) = meshes.get_mut(mesh) else {
        return;
    };
    let mut snapshot = MeshSnapshot::capture(&mesh);
    snapshot.uvs[quad * 4..quad * 4 + 4].copy_from_slice(&uvs);
    snapshot.restore(&mut mesh);
}

/// The GND cell under world point `at`; `None` west or north of the map.
fn gnd_cell(at: Vec3) -> Option<(u32, u32)> {
    let (x, y) = ((at.x / CELL_SIZE).floor(), (-at.z / CELL_SIZE).floor());
    (x >= 0.0 && y >= 0.0).then_some((x as u32, y as u32))
}

/// The brush texture follows the browser's ground texture selection.
fn follow_browser(browser: Res<BrowserSelection>, mut mode: ResMut<ToolMode>) {
    let (Some(AssetRef::GroundTexture(uri)), ToolMode::PaintTile(brush)) = (&browser.0, &*mode)
    else {
        return;
    };
    if brush.texture_uri == *uri {
        return;
    }
    let brush = TileBrush {
        texture_uri: uri.clone(),
        ..brush.clone()
    };
    *mode = ToolMode::PaintTile(brush);
}

/// A left press on the terrain starts a stroke; while held, each cell the
/// cursor crosses is painted; the release (or leaving the tool) records it.
fn stroke(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    stroke: Res<TileStroke>,
    mut cursor: TerrainCursor,
) {
    let brush = match &*mode {
        ToolMode::PaintTile(brush) if mouse.pressed(MouseButton::Left) => brush.clone(),
        _ => {
            if stroke.0.is_some() {
                commands.queue(end_stroke);
            }
            return;
        }
    };
    let Some(cell) = cursor.hit().and_then(gnd_cell) else {
        return;
    };
    if stroke.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        commands.queue(begin_stroke);
    }
    commands.queue(move |world: &mut World| paint_cell(world, cell, &brush));
}

/// Outlines the cell under the cursor.
fn draw_cell(mut gizmos: Gizmos, mut cursor: TerrainCursor) {
    let Some(hit) = cursor.hit() else {
        return;
    };
    let Some((x, y)) = gnd_cell(hit) else {
        return;
    };
    let center = Vec3::new(
        (x as f32 + 0.5) * CELL_SIZE,
        hit.y + 0.2,
        -(y as f32 + 0.5) * CELL_SIZE,
    );
    let flat = Isometry3d::new(center, Quat::from_rotation_x(FRAC_PI_2));
    gizmos.rect(flat, Vec2::splat(CELL_SIZE), CELL_COLOR);
}

fn options_window(mut contexts: EguiContexts, mut mode: ResMut<ToolMode>) -> Result {
    let ToolMode::PaintTile(brush) = &*mode else {
        return Ok(());
    };
    let mut brush = brush.clone();
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Paint tile").show(ctx, |ui| {
        ui.label(&brush.texture_uri)
            .on_hover_text("select another ground texture in the browser to change it");
        ui.horizontal(|ui| {
            ui.label("Rotation");
            for (turns, label) in [(0, "0°"), (1, "90°"), (2, "180°"), (3, "270°")] {
                ui.selectable_value(&mut brush.rotation, turns, label);
            }
        });
        ui.checkbox(&mut brush.flip, "Mirror");
    });
    mode.set_if_neq(ToolMode::PaintTile(brush));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::terrain::{QuadVerts, quad_count};

    fn texture(name: &str, uri: &str) -> TextureRef {
        TextureRef {
            name: name.into(),
            uri: uri.into(),
        }
    }

    #[test]
    fn uvs_turn_in_quarter_steps_and_mirror_in_u() {
        let base = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert_eq!(tile_uvs(0, false), base);
        assert_eq!(tile_uvs(1, false), [base[1], base[2], base[3], base[0]]);
        assert_eq!(tile_uvs(2, false), [base[2], base[3], base[0], base[1]]);
        assert_eq!(tile_uvs(3, false), [base[3], base[0], base[1], base[2]]);
        assert_eq!(tile_uvs(4, false), base);
        let mirror = |uvs: [[f32; 2]; 4]| uvs.map(|[u, v]| [1.0 - u, v]);
        for turns in 0..4 {
            assert_eq!(tile_uvs(turns, true), mirror(tile_uvs(turns, false)));
        }
        assert_eq!(
            tile_uvs(0, true),
            [[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        );
    }

    #[test]
    fn a_moved_quad_keeps_everything_but_its_uvs() {
        let quad = QuadVerts {
            positions: [
                [0.0, 1.0, 0.0],
                [10.0, 2.0, 0.0],
                [10.0, 3.0, -10.0],
                [0.0, 4.0, -10.0],
            ],
            normals: [[0.0, 1.0, 0.0]; 4],
            colors: [[0.5, 0.6, 0.7, 1.0]; 4],
            uvs: [[0.2, 0.2]; 4],
            pattern: [0, 1, 2, 2, 3, 0],
        };
        let mut src = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        MeshSnapshot::default().restore(&mut src);
        let mut dst = src.clone();
        append_quad(&mut src, &quad);
        append_quad(&mut dst, &quad);
        let uvs = tile_uvs(1, true);

        move_quad(&mut src, 0, &mut dst, uvs);

        assert_eq!(quad_count(&src), 0);
        let moved = remove_quad(&mut dst, 1);
        assert_eq!(moved, QuadVerts { uvs, ..quad });
    }

    #[test]
    fn textures_are_reused_by_uri_and_named_apart_by_stem() {
        let mut list = MapTextureList(vec![
            texture("grass01", "ro://ground/tex/grass01.ktx2"),
            texture("dirt", "ro://ground/tex/a/dirt.ktx2"),
        ]);

        let reused = ensure_texture(&mut list, "ro://ground/tex/grass01.ktx2");
        assert_eq!(reused, list.0[0]);
        assert_eq!(list.0.len(), 2);

        let appended = ensure_texture(&mut list, "ro://ground/tex/sand.ktx2");
        assert_eq!(appended, texture("sand", "ro://ground/tex/sand.ktx2"));

        let collided = ensure_texture(&mut list, "ro://ground/tex/b/dirt.ktx2");
        assert_eq!(collided.name, "dirt_2");
        let again = ensure_texture(&mut list, "ro://ground/tex/c/dirt.ktx2");
        assert_eq!(again.name, "dirt_3");
        assert_eq!(list.0.len(), 5);
    }
}
