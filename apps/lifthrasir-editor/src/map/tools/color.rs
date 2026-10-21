//! The Color tool: blends the terrain's vertex colors (`COLOR_0`) toward a
//! brush color. The new color depends only on the vertex position and its old
//! color, so vertices sharing a position stay in step across quads and
//! chunks. A stroke (press to release) is one `Batch` of chunk mesh edits.

use super::{TerrainCursor, ToolMode, sculpt::falloff};
use crate::{
    map::{
        open::EditorScreen,
        terrain::{MeshSnapshot, TerrainChunks},
    },
    shell::{
        ui::EditorUiSet,
        undo::{Batch, EditOp, MeshChunkEdit, record},
    },
};
use bevy::{gizmos::config::GizmoConfigStore, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_tooling::converters::map::terrain::CHUNK_CELLS;
use ro_formats::CELL_SIZE;
use std::{collections::BTreeMap, f32::consts::FRAC_PI_2};

const MAX_RADIUS: f32 = 500.0;

/// The color brush: a linear RGBA target, `radius` in world units, and
/// `strength` as a blend rate per second at the center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorBrush {
    pub color: [f32; 4],
    pub radius: f32,
    pub strength: f32,
}

impl Default for ColorBrush {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 1.0],
            radius: 15.0,
            strength: 2.0,
        }
    }
}

/// Each chunk's mesh before the stroke in progress first changed it.
#[derive(Resource, Default)]
struct ColorStroke(Option<BTreeMap<Entity, (Handle<Mesh>, MeshSnapshot)>>);

pub(super) fn plugin(app: &mut App) {
    let coloring = |mode: Res<ToolMode>| matches!(*mode, ToolMode::Color(_));
    app.init_resource::<ColorStroke>()
        .add_systems(
            Update,
            (
                stroke,
                draw_brush.run_if(coloring.and_then(resource_exists::<GizmoConfigStore>)),
            )
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            options_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing).and_then(coloring)),
        )
        .add_systems(
            OnExit(EditorScreen::Editing),
            |mut stroke: ResMut<ColorStroke>| stroke.0 = None,
        );
}

/// `old` moved toward `target` by `weight` (clamped to `0..=1`), per channel.
pub fn blend_color(old: [f32; 4], target: [f32; 4], weight: f32) -> [f32; 4] {
    let weight = weight.clamp(0.0, 1.0);
    std::array::from_fn(|channel| old[channel] + (target[channel] - old[channel]) * weight)
}

/// Blends the colors of every vertex of `chunk` (glTF frame) within the
/// brush by `amount` at the center, easing to 0 at the radius. Only colors
/// change. Returns whether any did.
pub fn paint_colors(chunk: &mut MeshSnapshot, hit: Vec3, brush: &ColorBrush, amount: f32) -> bool {
    let mut changed = false;
    for (position, color) in chunk.positions.iter().zip(&mut chunk.colors) {
        let distance = Vec2::new(position[0], position[2]).distance(hit.xz());
        if distance > brush.radius {
            continue;
        }
        let blended = blend_color(
            *color,
            brush.color,
            falloff(distance, brush.radius) * amount,
        );
        changed |= blended != *color;
        *color = blended;
    }
    changed
}

/// Starts a stroke.
pub fn begin_stroke(world: &mut World) {
    world.resource_mut::<ColorStroke>().0 = Some(BTreeMap::new());
}

/// One frame of the stroke in progress: `dt` seconds of `brush` at `hit`.
pub fn color_step(world: &mut World, hit: Vec3, brush: ColorBrush, dt: f32) {
    let Some(mut before) = world.resource_mut::<ColorStroke>().0.take() else {
        return;
    };
    let chunk_of = |native: f32| (native.max(0.0) / (CHUNK_CELLS as f32 * CELL_SIZE)) as u32;
    let (native_x, native_z) = (hit.x, -hit.z);
    let chunks = world.resource::<TerrainChunks>().chunks_in(
        [
            chunk_of(native_x - brush.radius),
            chunk_of(native_z - brush.radius),
        ],
        [
            chunk_of(native_x + brush.radius),
            chunk_of(native_z + brush.radius),
        ],
    );
    for (entity, handle) in chunks {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let Some(original) = meshes.get(&handle).map(MeshSnapshot::capture) else {
            continue;
        };
        let mut edited = original.clone();
        if !paint_colors(&mut edited, hit, &brush, brush.strength * dt) {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&handle) else {
            continue;
        };
        edited.restore(&mut mesh);
        before.entry(entity).or_insert((handle, original));
    }
    world.resource_mut::<ColorStroke>().0 = Some(before);
}

/// Ends the stroke, recording every chunk it changed as one undoable step.
pub fn end_stroke(world: &mut World) {
    let Some(before) = world.resource_mut::<ColorStroke>().0.take() else {
        return;
    };
    let meshes = world.resource::<Assets<Mesh>>();
    let ops: Vec<Box<dyn EditOp>> = before
        .into_iter()
        .filter_map(|(entity, (handle, before))| {
            let after = MeshSnapshot::capture(meshes.get(&handle)?);
            Some(Box::new(MeshChunkEdit::new(entity, handle, before, after)) as Box<dyn EditOp>)
        })
        .collect();
    if !ops.is_empty() {
        record(world, Box::new(Batch(ops)));
    }
}

/// A left press on the terrain starts a stroke; each held frame paints under
/// the cursor; the release (or leaving the tool) ends it.
fn stroke(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    stroke: Res<ColorStroke>,
    mut cursor: TerrainCursor,
) {
    let brush = match *mode {
        ToolMode::Color(brush) if mouse.pressed(MouseButton::Left) => brush,
        _ => {
            if stroke.0.is_some() {
                commands.queue(end_stroke);
            }
            return;
        }
    };
    let Some(hit) = cursor.hit() else {
        return;
    };
    if stroke.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        commands.queue(begin_stroke);
    }
    let dt = time.delta_secs();
    commands.queue(move |world: &mut World| color_step(world, hit, brush, dt));
}

fn draw_brush(mut gizmos: Gizmos, mode: Res<ToolMode>, mut cursor: TerrainCursor) {
    let ToolMode::Color(brush) = *mode else {
        return;
    };
    let Some(hit) = cursor.hit() else {
        return;
    };
    let [r, g, b, _] = brush.color;
    let flat = Isometry3d::new(hit + Vec3::Y * 0.2, Quat::from_rotation_x(FRAC_PI_2));
    gizmos
        .circle(flat, brush.radius, Color::linear_rgb(r, g, b))
        .resolution(64);
}

fn options_window(mut contexts: EguiContexts, mut mode: ResMut<ToolMode>) -> Result {
    let ToolMode::Color(mut brush) = *mode else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Vertex color").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.color_edit_button_rgba_unmultiplied(&mut brush.color);
        });
        ui.horizontal(|ui| {
            ui.label("Radius");
            ui.add(egui::DragValue::new(&mut brush.radius).range(1.0..=MAX_RADIUS));
        });
        ui.horizontal(|ui| {
            ui.label("Strength");
            ui.add(
                egui::DragValue::new(&mut brush.strength)
                    .range(0.05..=50.0)
                    .speed(0.05),
            )
            .on_hover_text("blend rate per second at the center");
        });
    });
    mode.set_if_neq(ToolMode::Color(brush));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::terrain::test_support::{chunk_snapshots, ground};

    #[test]
    fn blending_goes_from_the_old_to_the_target_color() {
        let (old, target) = ([1.0, 0.5, 0.0, 1.0], [0.0, 1.0, 1.0, 0.5]);
        assert_eq!(blend_color(old, target, 0.0), old);
        assert_eq!(blend_color(old, target, 0.5), [0.5, 0.75, 0.5, 0.75]);
        assert_eq!(blend_color(old, target, 1.0), target);
        assert_eq!(blend_color(old, target, 7.0), target);
    }

    #[test]
    fn a_stroke_changes_only_colors_and_keeps_shared_vertices_in_step() {
        let mut chunks = chunk_snapshots(&ground(CHUNK_CELLS + 2, 2, |x, _| [x as f32; 4]));
        let before = chunks.clone();
        let hit = Vec3::new(CHUNK_CELLS as f32 * CELL_SIZE, 0.0, -CELL_SIZE);
        let brush = ColorBrush::default();

        for chunk in &mut chunks {
            paint_colors(chunk, hit, &brush, 0.5);
        }

        let mut by_position: BTreeMap<[u32; 3], [f32; 4]> = BTreeMap::new();
        for (chunk, original) in chunks.iter().zip(&before) {
            assert_eq!(chunk.positions, original.positions);
            assert_eq!(chunk.normals, original.normals);
            assert_eq!(chunk.uvs, original.uvs);
            assert_eq!(chunk.indices, original.indices);
            for (position, color) in chunk.positions.iter().zip(&chunk.colors) {
                let key = position.map(f32::to_bits);
                assert_eq!(*by_position.entry(key).or_insert(*color), *color);
            }
        }
        assert_ne!(chunks, before);
    }
}
