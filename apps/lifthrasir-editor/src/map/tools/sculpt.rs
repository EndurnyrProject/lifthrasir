//! The Sculpt tool: raise, lower, smooth or flatten the terrain under a
//! round brush. Only vertex Y moves. A stroke (press to release) is one
//! `Batch` of chunk mesh edits plus the GAT heights under the touched cells,
//! rewritten with the retail rule so walking follows the new ground.

use super::{TerrainCursor, ToolMode, gat::refresh_gat_overlay};
use crate::{
    map::{
        layout::MapLayout,
        open::EditorScreen,
        terrain::{MeshSnapshot, TerrainChunks, ground_cell, native_quad, terrain_heights},
    },
    shell::{
        ui::EditorUiSet,
        undo::{Batch, EditOp, MapDataEdit, MeshChunkEdit, record},
    },
};
use bevy::{gizmos::config::GizmoConfigStore, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_runtime::world::LifMapData;
use lifthrasir_tooling::converters::{
    gltf_out::to_gltf_vec,
    map::{
        blank::gat_cell_heights,
        terrain::{CHUNK_CELLS, smooth_normals},
    },
};
use ro_formats::CELL_SIZE;
use std::{
    collections::{BTreeMap, BTreeSet},
    f32::consts::{FRAC_PI_2, PI},
};

/// How far past the brush radius a step reads chunks: a touched cell can lie
/// a cell diagonal outside the radius, its normals reach one ring further,
/// and computing those needs one more ring.
const REACH_CELLS: f32 = 4.0;
const BRUSH_COLOR: Color = Color::srgb(1.0, 0.85, 0.3);
const MAX_RADIUS: f32 = 500.0;

/// What a sculpt stroke does to the ground under the brush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptOp {
    Raise,
    Lower,
    Smooth,
    Flatten,
}

/// The sculpt brush: `radius` in world units; `strength` in world units per
/// second at the center for Raise/Lower, a blend rate per second for
/// Smooth/Flatten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SculptBrush {
    pub op: SculptOp,
    pub radius: f32,
    pub strength: f32,
}

impl Default for SculptBrush {
    fn default() -> Self {
        Self {
            op: SculptOp::Raise,
            radius: 15.0,
            strength: 20.0,
        }
    }
}

/// The stroke in progress: the flatten height (the press point), each
/// chunk's mesh before its first change, and the GND cells whose ground moved.
#[derive(Default)]
struct Stroke {
    flatten_y: f32,
    before: BTreeMap<Entity, (Handle<Mesh>, MeshSnapshot)>,
    touched: BTreeSet<(u32, u32)>,
}

#[derive(Resource, Default)]
struct SculptStroke(Option<Stroke>);

pub(super) fn plugin(app: &mut App) {
    let sculpting = |mode: Res<ToolMode>| matches!(*mode, ToolMode::Sculpt(_));
    app.init_resource::<SculptStroke>()
        .add_systems(
            Update,
            (
                stroke,
                draw_brush.run_if(sculpting.and_then(resource_exists::<GizmoConfigStore>)),
            )
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            options_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing).and_then(sculpting)),
        )
        .add_systems(
            OnExit(EditorScreen::Editing),
            |mut stroke: ResMut<SculptStroke>| stroke.0 = None,
        );
}

/// The new world Y of a vertex at old `y`. Raise/Lower move it by `weight`;
/// Smooth/Flatten blend it toward `target` by `weight` (clamped to 1).
pub fn sculpt_y(op: SculptOp, y: f32, weight: f32, target: f32) -> f32 {
    match op {
        SculptOp::Raise => y + weight,
        SculptOp::Lower => y - weight,
        SculptOp::Smooth | SculptOp::Flatten => y + (target - y) * weight.min(1.0),
    }
}

/// 1 at the center easing to 0 at `radius`.
pub(crate) fn falloff(distance: f32, radius: f32) -> f32 {
    0.5 * (1.0 + (PI * (distance / radius).min(1.0)).cos())
}

fn in_brush(position: [f32; 3], hit: Vec3, radius: f32) -> Option<f32> {
    let distance = Vec2::new(position[0], position[2]).distance(hit.xz());
    (distance <= radius).then_some(distance)
}

/// The mean Y of the ground vertices under the brush (Smooth's target).
fn mean_ground_y(chunks: &[MeshSnapshot], hit: Vec3, radius: f32) -> Option<f32> {
    let (sum, count) = chunks
        .iter()
        .flat_map(|chunk| chunk.positions.as_chunks::<4>().0)
        .filter(|quad| ground_cell(native_quad(quad)).is_some())
        .flatten()
        .filter(|p| in_brush(**p, hit, radius).is_some())
        .fold((0.0, 0usize), |(sum, count), p| (sum + p[1], count + 1));
    (count > 0).then(|| sum / count as f32)
}

/// One frame of a stroke on chunk meshes in the glTF frame. Moves the Y of
/// every vertex (ground and wall) within the brush by [`sculpt_y`], so
/// vertices sharing a position stay welded; then recomputes the ground
/// normals of the touched cells and their neighbours with the converter's
/// `smooth_normals`. `chunks` must hold every chunk within the brush radius
/// plus [`REACH_CELLS`]; `map` is the GND size. Returns the touched cells.
pub fn stroke_step(
    chunks: &mut [MeshSnapshot],
    hit: Vec3,
    brush: &SculptBrush,
    amount: f32,
    target: f32,
    map: (usize, usize),
) -> BTreeSet<(u32, u32)> {
    let mut touched = BTreeSet::new();
    for chunk in chunks.iter_mut() {
        for quad in chunk.positions.as_chunks_mut::<4>().0 {
            let mut moved = false;
            for position in quad.iter_mut() {
                let Some(distance) = in_brush(*position, hit, brush.radius) else {
                    continue;
                };
                let weight = falloff(distance, brush.radius) * amount;
                let y = sculpt_y(brush.op, position[1], weight, target);
                moved |= y != position[1];
                position[1] = y;
            }
            if let Some(cell) = moved.then(|| ground_cell(native_quad(quad))).flatten() {
                touched.insert(cell);
            }
        }
    }
    rebuild_normals(chunks, &touched, map);
    touched
}

/// Recomputes the smooth normals of `touched` and the ring around it from a
/// window one more ring wide, writing only those cells' ground vertices.
fn rebuild_normals(
    chunks: &mut [MeshSnapshot],
    touched: &BTreeSet<(u32, u32)>,
    map: (usize, usize),
) {
    let (width, height) = (map.0 as u32, map.1 as u32);
    let ring = |(x, y): (u32, u32), by: u32| {
        let (x0, y0) = (x.saturating_sub(by), y.saturating_sub(by));
        let (x1, y1) = ((x + by).min(width - 1), (y + by).min(height - 1));
        (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
    };
    if width == 0 || height == 0 {
        return;
    }
    let affected: BTreeSet<(u32, u32)> = touched.iter().flat_map(|&cell| ring(cell, 1)).collect();
    let Some(window) = affected.iter().flat_map(|&cell| ring(cell, 1)).fold(
        None,
        |rect: Option<(UVec2, UVec2)>, (x, y)| {
            let cell = UVec2::new(x, y);
            Some(rect.map_or((cell, cell), |(min, max)| (min.min(cell), max.max(cell))))
        },
    ) else {
        return;
    };
    let (min, size) = (window.0, window.1 - window.0 + UVec2::ONE);
    let local = |(x, y): (u32, u32)| ((y - min.y) * size.x + (x - min.x)) as usize;
    let mut cells = vec![[Vec3::ZERO; 4]; (size.x * size.y) as usize];
    let mut owners = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        for (quad, positions) in chunk.positions.as_chunks::<4>().0.iter().enumerate() {
            let corners = native_quad(positions);
            let Some(cell) = ground_cell(corners) else {
                continue;
            };
            let inside = cell.0 >= min.x
                && cell.1 >= min.y
                && cell.0 < min.x + size.x
                && cell.1 < min.y + size.y;
            if !inside {
                continue;
            }
            cells[local(cell)] = corners;
            if affected.contains(&cell) {
                owners.push((cell, index, quad));
            }
        }
    }
    let normals = smooth_normals(&cells, size.x as usize, size.y as usize);
    for (cell, index, quad) in owners {
        let normal = normals[local(cell)];
        for (corner, normal) in normal.iter().enumerate() {
            chunks[index].normals[quad * 4 + corner] = to_gltf_vec(*normal).to_array();
        }
    }
}

/// Starts a stroke at terrain point `hit`.
pub fn begin_stroke(world: &mut World, hit: Vec3) {
    world.resource_mut::<SculptStroke>().0 = Some(Stroke {
        flatten_y: hit.y,
        ..default()
    });
}

/// One frame of the stroke in progress: `dt` seconds of `brush` at `hit`.
pub fn sculpt_step(world: &mut World, hit: Vec3, brush: SculptBrush, dt: f32) {
    let Some(root) = world.get_resource::<MapLayout>().map(|layout| layout.root) else {
        return;
    };
    let Some(map) = world.get::<LifMapData>(root).map(|data| {
        (
            data.altitude.width as usize / 2,
            data.altitude.height as usize / 2,
        )
    }) else {
        return;
    };
    let Some(mut stroke) = world.resource_mut::<SculptStroke>().0.take() else {
        return;
    };
    let reach = brush.radius + REACH_CELLS * CELL_SIZE;
    let chunk_of = |native: f32| (native.max(0.0) / (CHUNK_CELLS as f32 * CELL_SIZE)) as u32;
    let (native_x, native_z) = (hit.x, -hit.z);
    let entries = world.resource::<TerrainChunks>().chunks_in(
        [chunk_of(native_x - reach), chunk_of(native_z - reach)],
        [chunk_of(native_x + reach), chunk_of(native_z + reach)],
    );
    let meshes = world.resource::<Assets<Mesh>>();
    let (entries, originals): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .filter_map(|(entity, handle)| {
            let snapshot = MeshSnapshot::capture(meshes.get(&handle)?);
            Some(((entity, handle), snapshot))
        })
        .unzip();
    let mut chunks = originals.clone();
    let target = match brush.op {
        SculptOp::Flatten => stroke.flatten_y,
        SculptOp::Smooth => mean_ground_y(&chunks, hit, brush.radius).unwrap_or(hit.y),
        SculptOp::Raise | SculptOp::Lower => 0.0,
    };
    let touched = stroke_step(&mut chunks, hit, &brush, brush.strength * dt, target, map);
    let changed = entries.into_iter().zip(originals).zip(chunks);
    for (((entity, handle), original), edited) in changed {
        if edited == original {
            continue;
        }
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let Some(mut mesh) = meshes.get_mut(&handle) else {
            continue;
        };
        edited.restore(&mut mesh);
        stroke.before.entry(entity).or_insert((handle, original));
    }
    stroke.touched.extend(touched);
    world.resource_mut::<SculptStroke>().0 = Some(stroke);
}

/// Ends the stroke: rewrites every GAT cell under a touched GND cell with
/// `gat_cell_heights` of its new corners (cell types stay) and records the
/// chunk meshes and map data as one undoable step.
pub fn end_stroke(world: &mut World) {
    let Some(stroke) = world.resource_mut::<SculptStroke>().0.take() else {
        return;
    };
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    if stroke.before.is_empty() {
        return;
    }
    let Some(before) = world.get::<LifMapData>(layout.root).cloned() else {
        return;
    };
    let heights = terrain_heights(world, &layout);
    let mut after = before.clone();
    let gat_width = after.altitude.width as usize;
    for &(x, y) in &stroke.touched {
        let corners = heights.get(x as usize, y as usize);
        for (sub_x, sub_y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let index = (2 * y as usize + sub_y) * gat_width + 2 * x as usize + sub_x;
            if let Some(cell) = after.altitude.cells.get_mut(index) {
                cell.height = gat_cell_heights(corners, sub_x, sub_y);
            }
        }
    }
    world.entity_mut(layout.root).insert(after.clone());
    let meshes = world.resource::<Assets<Mesh>>();
    let mut ops: Vec<Box<dyn EditOp>> = stroke
        .before
        .into_iter()
        .filter_map(|(entity, (handle, original))| {
            let edited = MeshSnapshot::capture(meshes.get(&handle)?);
            Some(Box::new(MeshChunkEdit::new(entity, handle, original, edited)) as Box<dyn EditOp>)
        })
        .collect();
    ops.push(Box::new(
        MapDataEdit::new(layout.root, before, after).with_hook(refresh_gat_overlay),
    ));
    record(world, Box::new(Batch(ops)));
    refresh_gat_overlay(world, layout.root);
}

/// A left press on the terrain starts a stroke; each held frame sculpts under
/// the cursor; the release (or leaving the tool) ends it.
fn stroke(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    stroke: Res<SculptStroke>,
    mut cursor: TerrainCursor,
) {
    let ToolMode::Sculpt(brush) = *mode else {
        if stroke.0.is_some() {
            commands.queue(end_stroke);
        }
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        if stroke.0.is_some() {
            commands.queue(end_stroke);
        }
        return;
    }
    let Some(hit) = cursor.hit() else {
        return;
    };
    if stroke.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        commands.queue(move |world: &mut World| begin_stroke(world, hit));
    }
    let dt = time.delta_secs();
    commands.queue(move |world: &mut World| sculpt_step(world, hit, brush, dt));
}

fn draw_brush(mut gizmos: Gizmos, mode: Res<ToolMode>, mut cursor: TerrainCursor) {
    let ToolMode::Sculpt(brush) = *mode else {
        return;
    };
    let Some(hit) = cursor.hit() else {
        return;
    };
    let flat = Isometry3d::new(hit + Vec3::Y * 0.2, Quat::from_rotation_x(FRAC_PI_2));
    gizmos
        .circle(flat, brush.radius, BRUSH_COLOR)
        .resolution(64);
}

fn options_window(mut contexts: EguiContexts, mut mode: ResMut<ToolMode>) -> Result {
    let ToolMode::Sculpt(mut brush) = *mode else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Sculpt").show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (op, label) in [
                (SculptOp::Raise, "Raise"),
                (SculptOp::Lower, "Lower"),
                (SculptOp::Smooth, "Smooth"),
                (SculptOp::Flatten, "Flatten"),
            ] {
                ui.selectable_value(&mut brush.op, op, label);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Radius");
            ui.add(egui::DragValue::new(&mut brush.radius).range(1.0..=MAX_RADIUS));
        });
        ui.horizontal(|ui| {
            ui.label("Strength");
            ui.add(
                egui::DragValue::new(&mut brush.strength)
                    .range(0.1..=200.0)
                    .speed(0.5),
            )
            .on_hover_text("units per second (Raise/Lower), blend rate (Smooth/Flatten)");
        });
    });
    mode.set_if_neq(ToolMode::Sculpt(brush));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::terrain::test_support::{chunk_snapshots, ground};

    /// Native corner positions of every cell, from glTF-frame chunks.
    fn full_grid(chunks: &[MeshSnapshot], width: usize, height: usize) -> Vec<[Vec3; 4]> {
        let mut cells = vec![[Vec3::ZERO; 4]; width * height];
        for chunk in chunks {
            for quad in chunk.positions.as_chunks::<4>().0 {
                let corners = native_quad(quad);
                if let Some((x, y)) = ground_cell(corners) {
                    cells[y as usize * width + x as usize] = corners;
                }
            }
        }
        cells
    }

    fn raise(radius: f32) -> SculptBrush {
        SculptBrush {
            op: SculptOp::Raise,
            radius,
            strength: 1.0,
        }
    }

    /// World point over the native point `(x, z)`.
    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, -z)
    }

    #[test]
    fn ops_move_y_as_documented() {
        assert_eq!(sculpt_y(SculptOp::Raise, 1.0, 0.5, 9.0), 1.5);
        assert_eq!(sculpt_y(SculptOp::Lower, 1.0, 0.5, 9.0), 0.5);
        assert_eq!(sculpt_y(SculptOp::Flatten, 1.0, 0.5, 9.0), 5.0);
        assert_eq!(sculpt_y(SculptOp::Smooth, 1.0, 3.0, 9.0), 9.0);
        assert_eq!(falloff(0.0, 5.0), 1.0);
        assert!(falloff(5.0, 5.0).abs() < 1e-6);
    }

    #[test]
    fn a_raise_across_a_chunk_border_keeps_shared_vertices_welded_and_xz_fixed() {
        let width = CHUNK_CELLS + 2;
        let ground = ground(width, 2, |_, _| [0.0; 4]);
        let mut chunks = chunk_snapshots(&ground);
        assert_eq!(
            chunks.len(),
            2,
            "the border splits the ground into two chunks"
        );
        let before = chunks.clone();
        let border = CHUNK_CELLS as f32 * CELL_SIZE;

        let touched = stroke_step(
            &mut chunks,
            at(border, CELL_SIZE),
            &raise(15.0),
            2.0,
            0.0,
            (width as usize, 2),
        );

        assert!(touched.contains(&(CHUNK_CELLS - 1, 0)) && touched.contains(&(CHUNK_CELLS, 0)));
        let mut by_xz: BTreeMap<(u32, u32), f32> = BTreeMap::new();
        for (chunk, original) in chunks.iter().zip(&before) {
            for (p, q) in chunk.positions.iter().zip(&original.positions) {
                assert_eq!((p[0], p[2]), (q[0], q[2]), "X and Z never change");
                let key = (p[0].to_bits(), p[2].to_bits());
                let y = *by_xz.entry(key).or_insert(p[1]);
                assert_eq!(y.to_bits(), p[1].to_bits(), "vertices at {p:?} stay welded");
            }
        }
        let center = by_xz[&(border.to_bits(), (-CELL_SIZE).to_bits())];
        assert_eq!(center, 2.0);
    }

    #[test]
    fn stroke_normals_match_smooth_normals_over_the_whole_grid() {
        let (width, height) = (CHUNK_CELLS + 4, 6);
        let ground = ground(width, height, |x, y| {
            let h = |dx: usize, dy: usize| ((x + dx) as f32 * 0.7).sin() * 4.0 + (y + dy) as f32;
            [h(0, 0), h(1, 0), h(0, 1), h(1, 1)]
        });
        let mut chunks = chunk_snapshots(&ground);
        let hit = at(CHUNK_CELLS as f32 * CELL_SIZE - 3.0, 3.0 * CELL_SIZE);
        let map = (width as usize, height as usize);

        for _ in 0..3 {
            stroke_step(&mut chunks, hit, &raise(18.0), 1.5, 0.0, map);
        }

        let grid = full_grid(&chunks, map.0, map.1);
        let expected = smooth_normals(&grid, map.0, map.1);
        for chunk in &chunks {
            for (quad, positions) in chunk.positions.as_chunks::<4>().0.iter().enumerate() {
                let (x, y) = ground_cell(native_quad(positions)).unwrap();
                let want =
                    expected[y as usize * map.0 + x as usize].map(|n| to_gltf_vec(n).to_array());
                assert_eq!(
                    &chunk.normals[quad * 4..quad * 4 + 4],
                    &want,
                    "cell ({x}, {y})"
                );
            }
        }
    }
}
