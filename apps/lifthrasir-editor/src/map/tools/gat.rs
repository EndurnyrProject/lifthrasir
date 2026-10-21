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
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_runtime::world::LifMapData;
use ro_formats::{CELL_SIZE, RoAltitude, gat::GatCellType};
use std::f32::consts::FRAC_PI_2;

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

/// The overlay entity of map root `0`.
#[derive(Component)]
pub struct GatOverlayMesh(pub Entity);

/// The stroke in progress: the map data when it started and the last cell
/// painted (each paint re-uploads the overlay, so a resting brush does not).
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

/// Indices of the cells within `radius` of `center`, clipped to the map.
fn brush_cells(
    altitude: &RoAltitude,
    center: (u32, u32),
    radius: u32,
) -> impl Iterator<Item = usize> {
    let (width, height) = (altitude.width as i64, altitude.height as i64);
    let (cx, cy, r) = (center.0 as i64, center.1 as i64, radius as i64);
    (-r..=r)
        .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(move |(dx, dy)| dx * dx + dy * dy <= r * r)
        .map(move |(dx, dy)| (cx + dx, cy + dy))
        .filter(move |(x, y)| (0..width).contains(x) && (0..height).contains(y))
        .map(move |(x, y)| (y * width + x) as usize)
}

/// Sets every cell within the brush circle to the brush type; heights stay.
pub fn paint_cells(altitude: &mut RoAltitude, center: (u32, u32), brush: &GatBrush) {
    let cells: Vec<usize> = brush_cells(altitude, center, brush.radius).collect();
    for index in cells {
        altitude.cells[index].cell_type = GatCellType::from(brush.raw_type);
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

/// One quad per GAT cell, in cell order, colored by raw type and lying on the
/// height the client walks.
pub fn gat_overlay_mesh(altitude: &RoAltitude) -> Mesh {
    let count = altitude.cells.len();
    let mut positions = Vec::with_capacity(count * 4);
    let mut colors = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for (index, cell) in altitude.cells.iter().enumerate() {
        let x = (index % altitude.width as usize) as f32 * GAT_CELL_SIZE;
        let z = (index / altitude.width as usize) as f32 * GAT_CELL_SIZE;
        let (x1, z1) = (x + GAT_CELL_SIZE, z + GAT_CELL_SIZE);
        let (inner_x0, inner_x1) = (x + CORNER_INSET, x1 - CORNER_INSET);
        let (inner_z0, inner_z1) = (z + CORNER_INSET, z1 - CORNER_INSET);
        positions.extend([
            [x, corner_height(altitude, inner_x0, inner_z0), -z],
            [x1, corner_height(altitude, inner_x1, inner_z0), -z],
            [x1, corner_height(altitude, inner_x1, inner_z1), -z1],
            [x, corner_height(altitude, inner_x0, inner_z1), -z1],
        ]);
        colors.extend([cell_color(cell.cell_type.raw()); 4]);
        let base = (index * 4) as u32;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
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

/// The [`EditHook`](crate::shell::undo::EditHook) of GAT edits: rebuilds the
/// overlay of `root` from its altitude, spawning it on first use.
pub fn rebuild_gat_overlay(world: &mut World, root: Entity) {
    let Some(mesh) = world
        .get::<LifMapData>(root)
        .map(|data| gat_overlay_mesh(&data.altitude))
    else {
        return;
    };
    let mesh = Mesh3d(world.resource_mut::<Assets<Mesh>>().add(mesh));
    let existing = world
        .query::<(Entity, &GatOverlayMesh)>()
        .iter(world)
        .find_map(|(entity, overlay)| (overlay.0 == root).then_some(entity));
    if let Some(overlay) = existing {
        world.entity_mut(overlay).insert(mesh);
        return;
    }
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
    world.spawn((
        Name::new("gat overlay"),
        GatOverlayMesh(root),
        EditorOnly,
        mesh,
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Hidden,
        NotShadowCaster,
    ));
}

/// Records the stroke that started from `before` as one undoable step (when
/// it changed a cell) and rebuilds the overlay.
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
    rebuild_gat_overlay(world, root);
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
    let colors: Vec<(usize, [f32; 4])> = brush_cells(&data.altitude, center, radius)
        .map(|index| {
            (
                index,
                cell_color(data.altitude.cells[index].cell_type.raw()),
            )
        })
        .collect();
    let Some(mesh) = world
        .query::<(&GatOverlayMesh, &Mesh3d)>()
        .iter(world)
        .find_map(|(overlay, mesh)| (overlay.0 == root).then_some(mesh.id()))
    else {
        return;
    };
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let Some(mut mesh) = meshes.get_mut(mesh) else {
        return;
    };
    let Ok(VertexAttributeValues::Float32x4(quads)) = mesh.try_attribute_mut(Mesh::ATTRIBUTE_COLOR)
    else {
        return;
    };
    for (index, color) in colors {
        if let Some(quad) = quads.get_mut(index * 4..index * 4 + 4) {
            quad.fill(color);
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
    let visibility = match overlay.visible {
        true => Visibility::Inherited,
        false => Visibility::Hidden,
    };
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
        let mesh = gat_overlay_mesh(&altitude);
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
    fn labels_follow_the_runtime_flags() {
        assert_eq!(cell_type_label(0), "0: walkable");
        assert_eq!(cell_type_label(1), "1: blocked");
        assert_eq!(cell_type_label(3), "3: walkable water");
        assert_eq!(cell_type_label(5), "5: blocked, snipable");
    }
}
