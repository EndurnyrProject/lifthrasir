//! The Wall tool: click two edge-adjacent GND cells and a vertical quad
//! bridging their shared edge is added to the brush texture's chunk of the
//! lower-index cell, with the converter's wall geometry (`wall_quad`).

use super::{TerrainCursor, ToolMode, paint::ensure_texture};
use crate::{
    map::{
        layout::{MapLayout, MapTextureList},
        open::EditorScreen,
        terrain::{
            MeshSnapshot, QuadVerts, TerrainChunks, append_quad, ground_quad_of, native_quad,
            spawn_chunk, sync_chunk_visibility, terrain_heights,
        },
    },
    shell::{
        browser::{AssetRef, BrowserSelection},
        ui::{EditorUiSet, viewport_has_pointer},
        undo::{Batch, MeshChunkEdit, record},
    },
};
use bevy::{gizmos::config::GizmoConfigStore, mesh::VertexAttributeValues, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_runtime::world::LifMapData;
use lifthrasir_tooling::converters::{
    gltf_out::to_gltf_vec,
    map::terrain::{WallDirection, chunk_of_quad, wall_quad},
};
use ro_formats::CELL_SIZE;
use std::f32::consts::FRAC_PI_2;

/// Full-texture wall UVs in `wall_quad`'s vertex order.
const WALL_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
/// The converter's wall index pattern.
const WALL_PATTERN: [u32; 6] = [0, 1, 2, 2, 1, 3];
const WHITE: [f32; 4] = [1.0; 4];
const FIRST_COLOR: Color = Color::srgb(1.0, 0.6, 0.1);
const HOVER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

/// The wall brush: the ground texture new walls get.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallBrush {
    pub texture_uri: String,
}

/// The first cell clicked, and why the last wall was refused.
#[derive(Resource, Default)]
struct WallPick {
    first: Option<(u32, u32)>,
    refused: Option<String>,
}

pub(super) fn plugin(app: &mut App) {
    let walling = |mode: Res<ToolMode>| matches!(*mode, ToolMode::Wall(_));
    app.init_resource::<WallPick>()
        .add_systems(
            Update,
            (
                follow_browser,
                forget_on_leave,
                pick_on_click.run_if(walling.and_then(viewport_has_pointer)),
                draw_cells.run_if(walling.and_then(resource_exists::<GizmoConfigStore>)),
            )
                .chain()
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            options_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing).and_then(walling)),
        )
        .add_systems(
            OnExit(EditorScreen::Editing),
            |mut pick: ResMut<WallPick>| *pick = WallPick::default(),
        );
}

/// The owner cell (the lower-index one) and side of the edge `a` and `b`
/// share; `None` unless they are edge-adjacent.
pub fn shared_edge(a: (u32, u32), b: (u32, u32)) -> Option<((u32, u32), WallDirection)> {
    let (owner, other) = if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    };
    match (other.0.checked_sub(owner.0), other.1.checked_sub(owner.1)) {
        (Some(0), Some(1)) => Some((owner, WallDirection::Front)),
        (Some(1), Some(0)) => Some((owner, WallDirection::Right)),
        _ => None,
    }
}

/// Adds a wall on the edge cells `a` and `b` share, as one undoable step.
/// Refused (with the reason) when they are not edge-adjacent, either is a
/// hole or off the map, or a wall already stands on that edge.
pub fn add_wall(
    world: &mut World,
    a: (u32, u32),
    b: (u32, u32),
    brush: &WallBrush,
) -> Result<(), String> {
    let layout = world
        .get_resource::<MapLayout>()
        .cloned()
        .ok_or("no map is open")?;
    let ((x, y), direction) = shared_edge(a, b).ok_or("the cells do not share an edge")?;
    if ground_quad_of(world, a).is_none() || ground_quad_of(world, b).is_none() {
        return Err("both cells need ground".into());
    }
    let heights = terrain_heights(world, &layout);
    let (x, y) = (x as usize, y as usize);
    let next = match direction {
        WallDirection::Front => (x, y + 1),
        WallDirection::Right => (x + 1, y),
    };
    let (positions, normal) = wall_quad(
        x,
        y,
        direction,
        heights.get(x, y),
        heights.get(next.0, next.1),
    );
    let chunk = chunk_of_quad(positions);
    if wall_exists(world, chunk, positions) {
        return Err("a wall already stands on that edge".into());
    }
    let texture = world
        .get_mut::<MapTextureList>(layout.root)
        .map(|mut list| ensure_texture(&mut list, &brush.texture_uri))
        .ok_or("the map has no texture list")?;
    let target = world.resource::<TerrainChunks>().get(chunk, &texture.name);
    let (entity, mesh) = target
        .or_else(|| spawn_chunk(world, &layout, chunk, &texture))
        .ok_or("no terrain chunk can take the wall")?;
    let quad = QuadVerts {
        positions: positions.map(|p| to_gltf_vec(Vec3::from(p)).to_array()),
        normals: [to_gltf_vec(Vec3::from(normal)).to_array(); 4],
        colors: [WHITE; 4],
        uvs: WALL_UVS,
        pattern: WALL_PATTERN,
    };
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let mut chunk_mesh = meshes.get_mut(&mesh).ok_or("the chunk mesh is gone")?;
    let before = MeshSnapshot::capture(&chunk_mesh);
    append_quad(&mut chunk_mesh, &quad);
    let after = MeshSnapshot::capture(&chunk_mesh);
    drop(chunk_mesh);
    sync_chunk_visibility(world, entity, false);
    record(
        world,
        Box::new(Batch(vec![Box::new(MeshChunkEdit::new(
            entity, mesh, before, after,
        ))])),
    );
    Ok(())
}

/// True when a wall quad of any `chunk` mesh spans the same edge as the
/// native wall `positions`.
fn wall_exists(world: &World, chunk: [u32; 2], positions: [[f32; 3]; 4]) -> bool {
    let footprint = |corners: [Vec3; 4]| {
        let flat = corners.map(|corner| corner.xz());
        let min = flat.iter().copied().reduce(Vec2::min);
        let max = flat.iter().copied().reduce(Vec2::max);
        (min, max)
    };
    let edge = footprint(positions.map(Vec3::from));
    let meshes = world.resource::<Assets<Mesh>>();
    world
        .resource::<TerrainChunks>()
        .chunks_in(chunk, chunk)
        .iter()
        .filter_map(
            |(_, handle)| match meshes.get(handle)?.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(positions)) => Some(positions.clone()),
                _ => None,
            },
        )
        .any(|positions| {
            positions.as_chunks::<4>().0.iter().any(|quad| {
                let corners = native_quad(quad);
                let (min, max) = footprint(corners);
                let wall = min
                    .zip(max)
                    .is_some_and(|(min, max)| min.x == max.x || min.y == max.y);
                wall && footprint(corners) == edge
            })
        })
}

/// The GND cell under world point `at`; `None` west or north of the map.
fn gnd_cell(at: Vec3) -> Option<(u32, u32)> {
    let (x, y) = ((at.x / CELL_SIZE).floor(), (-at.z / CELL_SIZE).floor());
    (x >= 0.0 && y >= 0.0).then_some((x as u32, y as u32))
}

/// The brush texture follows the browser's ground-texture selection.
fn follow_browser(browser: Res<BrowserSelection>, mut mode: ResMut<ToolMode>) {
    let (Some(AssetRef::GroundTexture(uri)), ToolMode::Wall(brush)) = (&browser.0, &*mode) else {
        return;
    };
    if brush.texture_uri != *uri {
        *mode = ToolMode::Wall(WallBrush {
            texture_uri: uri.clone(),
        });
    }
}

/// Leaving the tool forgets the first cell.
fn forget_on_leave(mode: Res<ToolMode>, mut pick: ResMut<WallPick>) {
    if !matches!(*mode, ToolMode::Wall(_)) && pick.first.is_some() {
        *pick = WallPick::default();
    }
}

/// The first click picks a cell; a second, edge-adjacent click adds the wall
/// (any other second click picks again).
fn pick_on_click(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut pick: ResMut<WallPick>,
    mut cursor: TerrainCursor,
) {
    let ToolMode::Wall(brush) = &*mode else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cell) = cursor.hit().and_then(gnd_cell) else {
        return;
    };
    let Some(first) = pick
        .first
        .filter(|&first| shared_edge(first, cell).is_some())
    else {
        pick.first = Some(cell);
        return;
    };
    pick.first = None;
    let brush = brush.clone();
    commands.queue(move |world: &mut World| {
        let refused = add_wall(world, first, cell, &brush).err();
        world.resource_mut::<WallPick>().refused = refused;
    });
}

/// Outlines the first cell and the cell under the cursor.
fn draw_cells(mut gizmos: Gizmos, pick: Res<WallPick>, mut cursor: TerrainCursor) {
    let Some(hit) = cursor.hit() else {
        return;
    };
    let hover = gnd_cell(hit);
    let cells = [(pick.first, FIRST_COLOR), (hover, HOVER_COLOR)];
    for (cell, color) in cells {
        let Some((x, y)) = cell else {
            continue;
        };
        let center = Vec3::new(
            (x as f32 + 0.5) * CELL_SIZE,
            hit.y + 0.2,
            -(y as f32 + 0.5) * CELL_SIZE,
        );
        let flat = Isometry3d::new(center, Quat::from_rotation_x(FRAC_PI_2));
        gizmos.rect(flat, Vec2::splat(CELL_SIZE), color);
    }
}

fn options_window(
    mut contexts: EguiContexts,
    mode: Res<ToolMode>,
    pick: Res<WallPick>,
    layout: Option<Res<MapLayout>>,
    data: Query<&LifMapData>,
) -> Result {
    let ToolMode::Wall(brush) = &*mode else {
        return Ok(());
    };
    let size = layout
        .and_then(|layout| data.get(layout.root).ok())
        .map(|data| (data.altitude.width / 2, data.altitude.height / 2));
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Wall").show(ctx, |ui| {
        ui.label(&brush.texture_uri)
            .on_hover_text("select another ground texture in the browser to change it");
        match pick.first {
            None => ui.label("click a cell, then an edge-adjacent cell"),
            Some((x, y)) => ui.label(format!("first cell ({x}, {y}); click a neighbour")),
        };
        if let Some((width, height)) = size {
            ui.weak(format!("{width} x {height} cells"));
        }
        if let Some(refused) = &pick.refused {
            ui.colored_label(egui::Color32::LIGHT_RED, refused);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_edge_neighbours_share_an_edge_owned_by_the_lower_cell() {
        assert_eq!(
            shared_edge((2, 3), (2, 4)),
            Some(((2, 3), WallDirection::Front))
        );
        assert_eq!(
            shared_edge((2, 4), (2, 3)),
            Some(((2, 3), WallDirection::Front))
        );
        assert_eq!(
            shared_edge((3, 3), (2, 3)),
            Some(((2, 3), WallDirection::Right))
        );
        assert_eq!(shared_edge((2, 3), (3, 4)), None);
        assert_eq!(shared_edge((2, 3), (2, 3)), None);
        assert_eq!(shared_edge((2, 3), (4, 3)), None);
    }
}
