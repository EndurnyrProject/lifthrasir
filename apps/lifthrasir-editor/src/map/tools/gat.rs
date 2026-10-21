//! The GAT paint tool: a circular brush sets the raw cell type of the root's
//! `LifMapData.altitude`, and an unlit overlay colors every cell by type.
//! One stroke (press to release) is one [`MapDataEdit`].

use super::{TerrainCursor, ToolMode};
use crate::{
    EditorOnly,
    map::{layout::MapLayout, open::EditorScreen},
    shell::{
        ui::{EditorUiSet, viewport_has_keyboard},
        undo::{MapDataEdit, record},
    },
};
use bevy::{
    asset::RenderAssetUsages,
    gizmos::config::GizmoConfigStore,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
    tasks::{ComputeTaskPool, ParallelSlice, TaskPool},
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_runtime::world::LifMapData;
use ro_formats::{CELL_SIZE, RoAltitude, gat::GatCellType};
use std::{collections::HashMap, f32::consts::FRAC_PI_2};

/// One GAT cell's side in world units (a GND cell holds 2 x 2 GAT cells).
pub const GAT_CELL_SIZE: f32 = CELL_SIZE / 2.0;
/// The raw cell types the retail client knows.
pub const CELL_TYPES: std::ops::RangeInclusive<u32> = 0..=6;
/// How far the overlay floats above the walk height.
const OVERLAY_LIFT: f32 = 0.1;
/// How far inside a cell corner the overlay samples its height, so the sample
/// reads this cell and not its neighbour.
const CORNER_INSET: f32 = 1e-3;
const OVERLAY_ALPHA: f32 = 0.45;
const MAX_RADIUS: u32 = 32;
/// Overlay chunk side in GAT cells: a paint re-uploads only the chunks it
/// touches (4096 vertices each), not the whole map.
pub const OVERLAY_CHUNK: u32 = 32;
const BRUSH_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);

/// The brush: cells within `radius` (in GAT cells) of the center get
/// `raw_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatBrush {
    pub radius: u32,
    pub raw_type: u32,
}

impl Default for GatBrush {
    fn default() -> Self {
        Self {
            radius: 1,
            raw_type: 1,
        }
    }
}

/// Whether the walkability overlay shows; `G` toggles it, entering the tool
/// turns it on.
#[derive(Resource, Default)]
pub struct GatOverlay {
    pub visible: bool,
}

/// The overlay of map root `0`; its children are the [`GatOverlayChunk`]s.
#[derive(Component)]
pub struct GatOverlayMesh(pub Entity);

/// One overlay mesh: the cells of chunk `0` (in [`OVERLAY_CHUNK`] units).
#[derive(Component)]
pub struct GatOverlayChunk(pub UVec2);

/// The stroke in progress: the map data when it started and the last cell
/// painted (each paint re-uploads the chunks it touches, so a resting brush
/// does not).
#[derive(Resource, Default)]
struct GatStroke {
    before: Option<LifMapData>,
    last: Option<(u32, u32)>,
}

pub(super) fn plugin(app: &mut App) {
    let painting = |mode: Res<ToolMode>| matches!(*mode, ToolMode::GatPaint(_));
    app.init_resource::<GatOverlay>()
        .init_resource::<GatStroke>()
        .add_systems(
            Update,
            (
                show_on_enter,
                toggle_overlay.run_if(viewport_has_keyboard),
                stroke,
                sync_overlay,
                draw_brush.run_if(painting.and_then(resource_exists::<GizmoConfigStore>)),
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
            (drop_stroke, despawn_overlays),
        );
}

/// The GAT cell under world point `world`, or `None` off the map.
pub fn gat_cell_at(altitude: &RoAltitude, world: Vec3) -> Option<(u32, u32)> {
    let x = (world.x / GAT_CELL_SIZE).floor();
    let z = (-world.z / GAT_CELL_SIZE).floor();
    if x < 0.0 || z < 0.0 || x >= altitude.width as f32 || z >= altitude.height as f32 {
        return None;
    }
    Some((x as u32, z as u32))
}

/// The cells within `radius` of `center` (`dx² + dy² <= r²`) on a
/// `width` x `height` grid. Shared by the cell brushes.
pub(crate) fn disc(
    center: (u32, u32),
    radius: u32,
    width: u32,
    height: u32,
) -> impl Iterator<Item = (u32, u32)> {
    let (width, height) = (i64::from(width), i64::from(height));
    let (cx, cy, r) = (i64::from(center.0), i64::from(center.1), i64::from(radius));
    (-r..=r)
        .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(move |(dx, dy)| dx * dx + dy * dy <= r * r)
        .map(move |(dx, dy)| (cx + dx, cy + dy))
        .filter(move |(x, y)| (0..width).contains(x) && (0..height).contains(y))
        .map(|(x, y)| (x as u32, y as u32))
}

/// The cells within `radius` of `center`, clipped to the map.
fn brush_cells(
    altitude: &RoAltitude,
    center: (u32, u32),
    radius: u32,
) -> impl Iterator<Item = (u32, u32)> {
    disc(center, radius, altitude.width, altitude.height)
}

/// Sets every cell within the brush circle to the brush type; heights stay.
pub fn paint_cells(altitude: &mut RoAltitude, center: (u32, u32), brush: &GatBrush) {
    let cells: Vec<(u32, u32)> = brush_cells(altitude, center, brush.radius).collect();
    let width = altitude.width as usize;
    for (x, y) in cells {
        altitude.cells[y as usize * width + x as usize].cell_type =
            GatCellType::from(brush.raw_type);
    }
}

/// What the runtime makes of `raw`, for the type picker.
pub fn cell_type_label(raw: u32) -> String {
    let cell = GatCellType::from(raw);
    let kind = match (cell.is_walkable(), cell.is_water(), cell.is_snipable()) {
        (true, true, _) => "walkable water",
        (true, false, _) => "walkable",
        (false, _, true) => "blocked, snipable",
        (false, _, false) => "blocked",
    };
    format!("{raw}: {kind}")
}

/// The overlay color of raw type `raw` (linear RGBA).
fn cell_color(raw: u32) -> [f32; 4] {
    let [r, g, b] = match raw {
        0 => [0.1, 0.8, 0.1],
        1 => [0.9, 0.05, 0.05],
        2 => [0.6, 0.8, 0.1],
        3 => [0.1, 0.3, 0.9],
        4 => [0.1, 0.7, 0.7],
        5 => [0.9, 0.5, 0.05],
        6 => [0.6, 0.2, 0.8],
        _ => [0.9, 0.1, 0.9],
    };
    [r, g, b, OVERLAY_ALPHA]
}

/// The walk height (world Y) just inside a cell corner, plus the lift.
fn corner_height(altitude: &RoAltitude, native_x: f32, native_z: f32) -> f32 {
    let at = Vec3::new(native_x, 0.0, -native_z);
    altitude.get_terrain_height_at_position(at).unwrap_or(0.0) + OVERLAY_LIFT
}

/// Every overlay chunk of `altitude`, row-major.
pub fn overlay_chunks(altitude: &RoAltitude) -> Vec<UVec2> {
    let (columns, rows) = (
        altitude.width.div_ceil(OVERLAY_CHUNK),
        altitude.height.div_ceil(OVERLAY_CHUNK),
    );
    (0..rows)
        .flat_map(|y| (0..columns).map(move |x| UVec2::new(x, y)))
        .collect()
}

/// The first cell of `chunk` and its size in cells, clipped to the map.
fn chunk_bounds(altitude: &RoAltitude, chunk: UVec2) -> (UVec2, UVec2) {
    let origin = chunk * OVERLAY_CHUNK;
    let size =
        (UVec2::new(altitude.width, altitude.height) - origin).min(UVec2::splat(OVERLAY_CHUNK));
    (origin, size)
}

/// Where cell `(x, y)` sits: its chunk and the index of its quad there.
fn quad_of(altitude: &RoAltitude, (x, y): (u32, u32)) -> (UVec2, usize) {
    let chunk = UVec2::new(x, y) / OVERLAY_CHUNK;
    let (origin, size) = chunk_bounds(altitude, chunk);
    let local = UVec2::new(x, y) - origin;
    (chunk, (local.y * size.x + local.x) as usize)
}

/// One quad per GAT cell of `chunk`, row-major, colored by raw type and
/// lying on the height the client walks.
pub fn gat_overlay_chunk_mesh(altitude: &RoAltitude, chunk: UVec2) -> Mesh {
    let (origin, size) = chunk_bounds(altitude, chunk);
    let count = (size.x * size.y) as usize;
    let mut positions = Vec::with_capacity(count * 4);
    let mut colors = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for cell_y in origin.y..origin.y + size.y {
        for cell_x in origin.x..origin.x + size.x {
            let cell = &altitude.cells[(cell_y * altitude.width + cell_x) as usize];
            let (x, z) = (cell_x as f32 * GAT_CELL_SIZE, cell_y as f32 * GAT_CELL_SIZE);
            let (x1, z1) = (x + GAT_CELL_SIZE, z + GAT_CELL_SIZE);
            let (inner_x0, inner_x1) = (x + CORNER_INSET, x1 - CORNER_INSET);
            let (inner_z0, inner_z1) = (z + CORNER_INSET, z1 - CORNER_INSET);
            let base = positions.len() as u32;
            positions.extend([
                [x, corner_height(altitude, inner_x0, inner_z0), -z],
                [x1, corner_height(altitude, inner_x1, inner_z0), -z],
                [x1, corner_height(altitude, inner_x1, inner_z1), -z1],
                [x, corner_height(altitude, inner_x0, inner_z1), -z1],
            ]);
            colors.extend([cell_color(cell.cell_type.raw()); 4]);
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count * 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Every chunk mesh of `altitude`, built across the compute task pool.
pub fn gat_overlay_meshes(altitude: &RoAltitude) -> Vec<(UVec2, Mesh)> {
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    overlay_chunks(altitude)
        .par_splat_map(pool, None, |_, chunks| {
            chunks
                .iter()
                .map(|&chunk| (chunk, gat_overlay_chunk_mesh(altitude, chunk)))
                .collect::<Vec<_>>()
        })
        .into_iter()
        .flatten()
        .collect()
}

/// The [`EditHook`](crate::shell::undo::EditHook) of GAT edits: replaces the
/// overlay of `root` with one built from its altitude.
// NOTE: re-uploads every chunk (one hitch on undo of a large map); diff
// against the live chunk colors if that shows.
pub fn rebuild_gat_overlay(world: &mut World, root: Entity) {
    let Some(chunks) = world
        .get::<LifMapData>(root)
        .map(|data| gat_overlay_meshes(&data.altitude))
    else {
        return;
    };
    let stale: Vec<Entity> = world
        .query::<(Entity, &GatOverlayMesh)>()
        .iter(world)
        .filter_map(|(entity, overlay)| (overlay.0 == root).then_some(entity))
        .collect();
    for overlay in stale {
        world.entity_mut(overlay).despawn();
    }
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
    let visibility = overlay_visibility(world.resource::<GatOverlay>());
    let overlay = world
        .spawn((
            Name::new("gat overlay"),
            GatOverlayMesh(root),
            EditorOnly,
            Transform::IDENTITY,
            visibility,
        ))
        .id();
    for (chunk, mesh) in chunks {
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world.spawn((
            GatOverlayChunk(chunk),
            EditorOnly,
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            ChildOf(overlay),
        ));
    }
}

/// Rebuilds the overlay of `root` only if it was built; the hook of edits that
/// move GAT heights without being GAT edits (sculpting).
pub fn refresh_gat_overlay(world: &mut World, root: Entity) {
    let built = world
        .query::<&GatOverlayMesh>()
        .iter(world)
        .any(|overlay| overlay.0 == root);
    if built {
        rebuild_gat_overlay(world, root);
    }
}

fn overlay_visibility(overlay: &GatOverlay) -> Visibility {
    match overlay.visible {
        true => Visibility::Inherited,
        false => Visibility::Hidden,
    }
}

/// Records the stroke that started from `before` as one undoable step, when
/// it changed a cell. The overlay already shows it (painting recolors live).
pub fn end_stroke(world: &mut World, before: LifMapData) {
    let Some(root) = world.get_resource::<MapLayout>().map(|layout| layout.root) else {
        return;
    };
    let Some(after) = world.get::<LifMapData>(root).cloned() else {
        return;
    };
    let changed = before
        .altitude
        .cells
        .iter()
        .zip(&after.altitude.cells)
        .any(|(old, new)| old.cell_type != new.cell_type);
    if changed {
        record(
            world,
            Box::new(MapDataEdit::new(root, before, after).with_hook(rebuild_gat_overlay)),
        );
    }
}

/// Shows the overlay when the tool is entered, not on brush changes, so `G`
/// can hide it while painting.
fn show_on_enter(
    mode: Res<ToolMode>,
    mut was_painting: Local<bool>,
    mut overlay: ResMut<GatOverlay>,
) {
    let painting = matches!(*mode, ToolMode::GatPaint(_));
    if painting && !*was_painting {
        overlay.visible = true;
    }
    *was_painting = painting;
}

fn toggle_overlay(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<GatOverlay>) {
    if keys.just_pressed(KeyCode::KeyG) {
        overlay.visible = !overlay.visible;
    }
}

/// A left press on the terrain starts a stroke; while held, the brush paints
/// under the cursor live; the release (or leaving the tool) records it.
fn stroke(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut stroke: ResMut<GatStroke>,
    mut cursor: TerrainCursor,
    layout: Option<Res<MapLayout>>,
    mut data: Query<&mut LifMapData>,
) {
    let Some(root) = layout.map(|layout| layout.root) else {
        return;
    };
    let ToolMode::GatPaint(brush) = *mode else {
        return finish(&mut commands, &mut stroke);
    };
    if !mouse.pressed(MouseButton::Left) {
        return finish(&mut commands, &mut stroke);
    }
    let Some(hit) = cursor.hit() else {
        return;
    };
    let Ok(mut data) = data.get_mut(root) else {
        return;
    };
    if stroke.before.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        stroke.before = Some(data.clone());
    }
    let Some(center) = gat_cell_at(&data.altitude, hit) else {
        return;
    };
    if stroke.last.replace(center) == Some(center) {
        return;
    }
    paint_cells(&mut data.altitude, center, &brush);
    commands.queue(move |world: &mut World| recolor(world, root, center, brush.radius));
}

fn finish(commands: &mut Commands, stroke: &mut GatStroke) {
    stroke.last = None;
    if let Some(before) = stroke.before.take() {
        commands.queue(move |world: &mut World| end_stroke(world, before));
    }
}

/// Recolors the overlay quads of the brush's cells in place, so a stroke
/// shows while painting.
fn recolor(world: &mut World, root: Entity, center: (u32, u32), radius: u32) {
    let Some(data) = world.get::<LifMapData>(root) else {
        return;
    };
    let altitude = &data.altitude;
    let quads: Vec<(UVec2, usize, [f32; 4])> = brush_cells(altitude, center, radius)
        .map(|(x, y)| {
            let (chunk, quad) = quad_of(altitude, (x, y));
            let raw = altitude.cells[(y * altitude.width + x) as usize]
                .cell_type
                .raw();
            (chunk, quad, cell_color(raw))
        })
        .collect();
    let chunks: HashMap<UVec2, AssetId<Mesh>> = world
        .query::<(&GatOverlayChunk, &Mesh3d, &ChildOf)>()
        .iter(world)
        .filter(|(_, _, parent)| {
            world
                .get::<GatOverlayMesh>(parent.parent())
                .is_some_and(|overlay| overlay.0 == root)
        })
        .map(|(chunk, mesh, _)| (chunk.0, mesh.id()))
        .collect();
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    for (chunk, quad, color) in quads {
        let Some(mut mesh) = chunks.get(&chunk).and_then(|&id| meshes.get_mut(id)) else {
            continue;
        };
        let Ok(VertexAttributeValues::Float32x4(colors)) =
            mesh.try_attribute_mut(Mesh::ATTRIBUTE_COLOR)
        else {
            continue;
        };
        if let Some(vertices) = colors.get_mut(quad * 4..quad * 4 + 4) {
            vertices.fill(color);
        }
    }
}

/// Shows or hides the overlay, building it the first time it shows.
fn sync_overlay(
    mut commands: Commands,
    overlay: Res<GatOverlay>,
    layout: Option<Res<MapLayout>>,
    mut overlays: Query<(&GatOverlayMesh, &mut Visibility)>,
) {
    let Some(root) = layout.map(|layout| layout.root) else {
        return;
    };
    let visibility = overlay_visibility(&overlay);
    let mut built = false;
    for (mesh, mut current) in &mut overlays {
        built |= mesh.0 == root;
        current.set_if_neq(visibility);
    }
    if overlay.visible && !built {
        commands.queue(move |world: &mut World| rebuild_gat_overlay(world, root));
    }
}

/// The brush circle on the terrain under the cursor.
fn draw_brush(mut gizmos: Gizmos, mode: Res<ToolMode>, mut cursor: TerrainCursor) {
    let ToolMode::GatPaint(brush) = *mode else {
        return;
    };
    let Some(hit) = cursor.hit() else {
        return;
    };
    let radius = (brush.radius as f32 + 0.5) * GAT_CELL_SIZE;
    let flat = Isometry3d::new(hit + Vec3::Y * 0.2, Quat::from_rotation_x(FRAC_PI_2));
    gizmos.circle(flat, radius, BRUSH_COLOR).resolution(64);
}

fn options_window(
    mut contexts: EguiContexts,
    mut mode: ResMut<ToolMode>,
    mut overlay: ResMut<GatOverlay>,
) -> Result {
    let ToolMode::GatPaint(mut brush) = *mode else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("GAT paint").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Radius (cells)");
            ui.add(egui::DragValue::new(&mut brush.radius).range(0..=MAX_RADIUS));
        });
        for raw in CELL_TYPES {
            let [r, g, b, _] = cell_color(raw);
            let swatch = egui::Rgba::from_rgb(r, g, b);
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, swatch);
                ui.radio_value(&mut brush.raw_type, raw, cell_type_label(raw));
            });
        }
        ui.checkbox(&mut overlay.visible, "Overlay (G)");
    });
    mode.set_if_neq(ToolMode::GatPaint(brush));
    Ok(())
}

fn drop_stroke(mut stroke: ResMut<GatStroke>) {
    *stroke = GatStroke::default();
}

fn despawn_overlays(mut commands: Commands, overlays: Query<Entity, With<GatOverlayMesh>>) {
    for overlay in &overlays {
        commands.entity(overlay).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_formats::gat::GatCell;

    fn altitude(width: u32, height: u32) -> RoAltitude {
        RoAltitude {
            version: "1.2".into(),
            width,
            height,
            cells: vec![
                GatCell {
                    height: [0.0; 4],
                    cell_type: GatCellType::from(0),
                };
                (width * height) as usize
            ],
        }
    }

    fn center_of(x: u32, z: u32) -> Vec3 {
        Vec3::new(
            (x as f32 + 0.5) * GAT_CELL_SIZE,
            0.0,
            -(z as f32 + 0.5) * GAT_CELL_SIZE,
        )
    }

    #[test]
    fn a_cell_center_maps_to_its_cell_and_off_map_to_none() {
        let altitude = altitude(4, 3);
        assert_eq!(gat_cell_at(&altitude, center_of(0, 0)), Some((0, 0)));
        assert_eq!(gat_cell_at(&altitude, center_of(3, 2)), Some((3, 2)));
        assert_eq!(gat_cell_at(&altitude, center_of(4, 0)), None);
        assert_eq!(gat_cell_at(&altitude, center_of(0, 3)), None);
        assert_eq!(gat_cell_at(&altitude, Vec3::new(-0.1, 0.0, -1.0)), None);
        assert_eq!(gat_cell_at(&altitude, Vec3::new(1.0, 0.0, 0.1)), None);
    }

    #[test]
    fn a_radius_one_brush_paints_exactly_the_circle() {
        let mut altitude = altitude(5, 5);
        paint_cells(
            &mut altitude,
            (2, 2),
            &GatBrush {
                radius: 1,
                raw_type: 5,
            },
        );
        let painted: Vec<(u32, u32)> = (0..25u32)
            .filter(|&index| altitude.cells[index as usize].cell_type.raw() == 5)
            .map(|index| (index % 5, index / 5))
            .collect();
        assert_eq!(painted, vec![(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)]);
        assert!(!altitude.cells[12].cell_type.is_walkable());
    }

    #[test]
    fn a_brush_at_the_edge_is_clipped_to_the_map() {
        let mut altitude = altitude(3, 3);
        let brush = GatBrush {
            radius: 2,
            raw_type: 1,
        };
        paint_cells(&mut altitude, (0, 0), &brush);
        let painted = altitude
            .cells
            .iter()
            .filter(|cell| cell.cell_type.raw() == 1)
            .count();
        assert_eq!(painted, 6);
    }

    #[test]
    fn the_overlay_has_one_quad_per_cell_on_the_walk_height() {
        let mut altitude = altitude(3, 2);
        altitude.cells[0].height = [-4.0; 4];
        let mesh = gat_overlay_chunk_mesh(&altitude, UVec2::ZERO);
        assert_eq!(mesh.count_vertices(), 4 * 3 * 2);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let expected = altitude
            .get_terrain_height_at_position(center_of(0, 0))
            .unwrap()
            + OVERLAY_LIFT;
        assert_eq!(positions[0], [0.0, expected, 0.0]);
        assert_eq!(positions[2][0], GAT_CELL_SIZE);
        assert_eq!(positions[2][2], -GAT_CELL_SIZE);
    }

    #[test]
    fn the_chunks_cover_every_cell_once() {
        let (width, height) = (OVERLAY_CHUNK + 8, OVERLAY_CHUNK + 3);
        let altitude = altitude(width, height);
        let meshes = gat_overlay_meshes(&altitude);
        let chunks: Vec<UVec2> = meshes.iter().map(|(chunk, _)| *chunk).collect();
        assert_eq!(
            chunks,
            [
                UVec2::new(0, 0),
                UVec2::new(1, 0),
                UVec2::new(0, 1),
                UVec2::new(1, 1)
            ]
        );
        let vertices: usize = meshes.iter().map(|(_, mesh)| mesh.count_vertices()).sum();
        assert_eq!(vertices, 4 * (width * height) as usize);
        assert_eq!(meshes[3].1.count_vertices(), 4 * 8 * 3);
    }

    #[test]
    fn a_cell_maps_to_its_chunk_quad() {
        let altitude = altitude(OVERLAY_CHUNK + 8, OVERLAY_CHUNK + 3);
        assert_eq!(quad_of(&altitude, (0, 0)), (UVec2::ZERO, 0));
        assert_eq!(
            quad_of(&altitude, (OVERLAY_CHUNK + 1, OVERLAY_CHUNK + 2)),
            (UVec2::ONE, 2 * 8 + 1)
        );
        let (chunk, quad) = quad_of(&altitude, (OVERLAY_CHUNK + 1, OVERLAY_CHUNK + 2));
        let mesh = gat_overlay_chunk_mesh(&altitude, chunk);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let corner = positions[quad * 4];
        assert_eq!(corner[0], (OVERLAY_CHUNK + 1) as f32 * GAT_CELL_SIZE);
        assert_eq!(corner[2], -((OVERLAY_CHUNK + 2) as f32) * GAT_CELL_SIZE);
    }

    #[test]
    fn painting_recolors_only_the_quads_of_the_brush_cells() {
        use lifthrasir_data::lif::LifMap;
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<GatOverlay>();
        let root = world
            .spawn(LifMapData {
                altitude: altitude(OVERLAY_CHUNK + 8, 4),
                water: None,
                water_tiles: Vec::new(),
                meta: LifMap {
                    format_version: 0,
                    source: None,
                    ambient_color: [0.0; 3],
                    no_shade_tint: [1.0; 3],
                    indoor: false,
                    ambient_brightness: 0.0,
                    exposure_ev100: 0.0,
                },
            })
            .id();
        rebuild_gat_overlay(&mut world, root);
        let center = (OVERLAY_CHUNK, 1);
        let brush = GatBrush {
            radius: 1,
            raw_type: 3,
        };
        paint_cells(
            &mut world.get_mut::<LifMapData>(root).unwrap().altitude,
            center,
            &brush,
        );

        recolor(&mut world, root, center, brush.radius);

        let altitude = world.get::<LifMapData>(root).unwrap().altitude.clone();
        let meshes: HashMap<UVec2, Handle<Mesh>> = world
            .query::<(&GatOverlayChunk, &Mesh3d)>()
            .iter(&world)
            .map(|(chunk, mesh)| (chunk.0, mesh.0.clone()))
            .collect();
        let assets = world.resource::<Assets<Mesh>>();
        for (index, cell) in altitude.cells.iter().enumerate() {
            let at = (index as u32 % altitude.width, index as u32 / altitude.width);
            let (chunk, quad) = quad_of(&altitude, at);
            let Some(VertexAttributeValues::Float32x4(colors)) = assets
                .get(&meshes[&chunk])
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("colors");
            };
            assert_eq!(colors[quad * 4], cell_color(cell.cell_type.raw()), "{at:?}");
        }
        assert_eq!(
            altitude
                .cells
                .iter()
                .filter(|cell| cell.cell_type.raw() == 3)
                .count(),
            5
        );
    }

    #[test]
    fn labels_follow_the_runtime_flags() {
        assert_eq!(cell_type_label(0), "0: walkable");
        assert_eq!(cell_type_label(1), "1: blocked");
        assert_eq!(cell_type_label(3), "3: walkable water");
        assert_eq!(cell_type_label(5), "5: blocked, snipable");
    }
}
