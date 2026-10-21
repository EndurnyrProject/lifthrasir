//! The Water tool: add or remove the map's water, edit its zones, recompute
//! the tile mask from the terrain heights, and paint tiles in or out with a
//! GND-cell brush. Every change is one [`MapDataEdit`] whose hook rebuilds
//! the water surfaces.

use super::{TerrainCursor, ToolMode, gat::disc};
use crate::{
    map::{layout::MapLayout, open::EditorScreen, terrain::terrain_heights},
    shell::{
        ui::EditorUiSet,
        undo::{MapDataEdit, commit},
    },
};
use bevy::{gizmos::config::GizmoConfigStore, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_data::lif::{LifWater, LifWaterZone};
use lifthrasir_runtime::world::{
    LifMapData, SceneOwner,
    water::{WaterLoadingState, WaterSurface, respawn_water},
};
use lifthrasir_tooling::converters::map::water::select_tiles;
use ro_formats::CELL_SIZE;
use std::{collections::BTreeSet, f32::consts::FRAC_PI_2};

/// The zone new water starts with: the RSW defaults, at level 0 (which the
/// runtime does not draw until the level is set).
pub const NEW_ZONE: LifWaterZone = LifWaterZone {
    level: 0.0,
    water_type: 0,
    wave_height: 0.2,
    wave_speed: 2.0,
    wave_pitch: 50.0,
    anim_speed: 3,
};
const MAX_RADIUS: u32 = 16;
const BRUSH_COLOR: Color = Color::srgb(0.3, 0.7, 1.0);
const STROKE_ADD_COLOR: Color = Color::srgb(0.3, 0.7, 1.0);
const STROKE_REMOVE_COLOR: Color = Color::srgb(1.0, 0.4, 0.2);

/// The tile brush: GND cells within `radius` of the center are added to the
/// water mask, or removed from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterBrush {
    pub radius: u32,
    pub add: bool,
}

impl Default for WaterBrush {
    fn default() -> Self {
        Self {
            radius: 1,
            add: true,
        }
    }
}

/// The tile stroke in progress: whether it adds, and the cells it covered.
#[derive(Resource, Default)]
struct WaterStroke(Option<(bool, BTreeSet<(u32, u32)>)>);

/// The map data when the zone field drag or text edit in progress started.
#[derive(Resource, Default)]
struct WaterWindow {
    gesture: Option<LifMapData>,
}

#[derive(Clone, Copy)]
enum Action {
    Add,
    Remove,
    Recompute,
}

pub(super) fn plugin(app: &mut App) {
    let in_tool = |mode: Res<ToolMode>| matches!(*mode, ToolMode::Water(_));
    app.init_resource::<WaterStroke>()
        .init_resource::<WaterWindow>()
        .add_systems(
            Update,
            (
                stroke,
                draw_brush.run_if(in_tool.and_then(resource_exists::<GizmoConfigStore>)),
            )
                .chain()
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            water_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing)),
        )
        .add_systems(OnExit(EditorScreen::Editing), reset);
}

/// The [`EditHook`](crate::shell::undo::EditHook) of water edits: replaces
/// the surfaces of `root` with ones built from its current map data.
pub fn refresh_water(world: &mut World, root: Entity) {
    // A load still pending from the previous state would spawn stale water.
    world.entity_mut(root).remove::<WaterLoadingState>();
    if let Err(error) = world.run_system_cached_with(respawn_root_water, root) {
        error!("cannot respawn the water: {error}");
    }
}

fn respawn_root_water(
    In(root): In<Entity>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    data: Query<&LifMapData>,
    surfaces: Query<(Entity, &SceneOwner), With<WaterSurface>>,
) {
    let Ok(data) = data.get(root) else {
        return;
    };
    respawn_water(&mut commands, &assets, root, data, &surfaces);
}

/// One zone of [`NEW_ZONE`] over a `width` x `height` GND, with no tiles yet.
pub fn new_water(width: u32, height: u32) -> LifWater {
    LifWater {
        split_width: 1,
        split_height: 1,
        zones: vec![NEW_ZONE],
        width,
        height,
        buffer_view: 0,
    }
}

/// Commits `water` and its `tiles` as the open map's water, as one undoable
/// step. Nothing is recorded when they are unchanged.
pub fn commit_water(world: &mut World, water: Option<LifWater>, tiles: Vec<(usize, usize)>) {
    let Some((root, before)) = open_data(world) else {
        return;
    };
    commit_change(world, root, before, water, tiles);
}

/// Adds new water sized to the GND (half the GAT), with no tiles.
pub fn add_water(world: &mut World) {
    let Some((root, before)) = open_data(world) else {
        return;
    };
    let water = new_water(before.altitude.width / 2, before.altitude.height / 2);
    commit_change(world, root, before, Some(water), Vec::new());
}

/// The tiles `select_tiles` picks with native heights read from the terrain
/// meshes; `None` without water, or when the water is not GND-sized.
pub fn recompute_tiles(world: &World, layout: &MapLayout) -> Option<Vec<(usize, usize)>> {
    let water = world.get::<LifMapData>(layout.root)?.water.as_ref()?;
    let heights = terrain_heights(world, layout);
    if (heights.width, heights.height) != (water.width as usize, water.height as usize) {
        error!(
            "water is {}x{} but the terrain is {}x{} cells",
            water.width, water.height, heights.width, heights.height
        );
        return None;
    }
    Some(select_tiles(water, |x, y| heights.get(x, y)))
}

/// Replaces the open map's tiles with [`recompute_tiles`], as one step.
pub fn commit_recomputed_tiles(world: &mut World) {
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    let Some(tiles) = recompute_tiles(world, &layout) else {
        return;
    };
    let Some((root, before)) = open_data(world) else {
        return;
    };
    let water = before.water.clone();
    commit_change(world, root, before, water, tiles);
}

/// `tiles` with `cells` added (or removed), row-major and without duplicates.
pub fn paint_tiles(
    tiles: &[(usize, usize)],
    cells: impl IntoIterator<Item = (usize, usize)>,
    add: bool,
) -> Vec<(usize, usize)> {
    let mut rows: BTreeSet<(usize, usize)> = tiles.iter().map(|&(x, y)| (y, x)).collect();
    for (x, y) in cells {
        match add {
            true => rows.insert((y, x)),
            false => rows.remove(&(y, x)),
        };
    }
    rows.into_iter().map(|(y, x)| (x, y)).collect()
}

/// The GND cell under world point `at`, or `None` outside the water grid.
pub fn gnd_cell_at(water: &LifWater, at: Vec3) -> Option<(u32, u32)> {
    let x = (at.x / CELL_SIZE).floor();
    let y = (-at.z / CELL_SIZE).floor();
    if x < 0.0 || y < 0.0 || x >= water.width as f32 || y >= water.height as f32 {
        return None;
    }
    Some((x as u32, y as u32))
}

fn open_data(world: &World) -> Option<(Entity, LifMapData)> {
    let root = world.get_resource::<MapLayout>()?.root;
    Some((root, world.get::<LifMapData>(root)?.clone()))
}

fn commit_change(
    world: &mut World,
    root: Entity,
    before: LifMapData,
    water: Option<LifWater>,
    tiles: Vec<(usize, usize)>,
) {
    if before.water == water && before.water_tiles == tiles {
        return;
    }
    let after = LifMapData {
        water,
        water_tiles: tiles,
        ..before.clone()
    };
    commit(
        world,
        Box::new(MapDataEdit::new(root, before, after).with_hook(refresh_water)),
    );
}

fn paint_stroke(world: &mut World, add: bool, cells: BTreeSet<(u32, u32)>) {
    let Some((root, before)) = open_data(world) else {
        return;
    };
    let Some(water) = before.water.clone() else {
        return;
    };
    let cells = cells.into_iter().map(|(x, y)| (x as usize, y as usize));
    let tiles = paint_tiles(&before.water_tiles, cells, add);
    commit_change(world, root, before, Some(water), tiles);
}

/// A left press on the terrain starts a tile stroke; while held, the brush
/// collects GND cells; the release (or leaving the tool) commits them.
fn stroke(
    mut commands: Commands,
    mode: Res<ToolMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut stroke: ResMut<WaterStroke>,
    mut cursor: TerrainCursor,
    layout: Option<Res<MapLayout>>,
    data: Query<&LifMapData>,
) {
    let painting = matches!(*mode, ToolMode::Water(_)) && mouse.pressed(MouseButton::Left);
    if !painting {
        if let Some((add, cells)) = stroke.0.take() {
            commands.queue(move |world: &mut World| paint_stroke(world, add, cells));
        }
        return;
    }
    let ToolMode::Water(brush) = *mode else {
        return;
    };
    let Some(water) = layout
        .and_then(|layout| data.get(layout.root).ok())
        .and_then(|data| data.water.as_ref())
    else {
        return;
    };
    let Some(center) = cursor.hit().and_then(|hit| gnd_cell_at(water, hit)) else {
        return;
    };
    if stroke.0.is_none() && !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let (_, cells) = stroke.0.get_or_insert_with(|| (brush.add, BTreeSet::new()));
    cells.extend(disc(center, brush.radius, water.width, water.height));
}

/// The brush circle under the cursor and the cells of the stroke so far.
fn draw_brush(
    mut gizmos: Gizmos,
    mode: Res<ToolMode>,
    stroke: Res<WaterStroke>,
    mut cursor: TerrainCursor,
) {
    let ToolMode::Water(brush) = *mode else {
        return;
    };
    let Some(hit) = cursor.hit() else {
        return;
    };
    let flat = Quat::from_rotation_x(FRAC_PI_2);
    let lift = hit.y + 0.2;
    let radius = (brush.radius as f32 + 0.5) * CELL_SIZE;
    gizmos
        .circle(Isometry3d::new(hit.with_y(lift), flat), radius, BRUSH_COLOR)
        .resolution(64);
    let Some((add, cells)) = &stroke.0 else {
        return;
    };
    let color = if *add {
        STROKE_ADD_COLOR
    } else {
        STROKE_REMOVE_COLOR
    };
    for &(x, y) in cells {
        let center = Vec3::new(
            (x as f32 + 0.5) * CELL_SIZE,
            lift,
            -(y as f32 + 0.5) * CELL_SIZE,
        );
        gizmos.rect(Isometry3d::new(center, flat), Vec2::splat(CELL_SIZE), color);
    }
}

fn reset(mut stroke: ResMut<WaterStroke>, mut window: ResMut<WaterWindow>) {
    stroke.0 = None;
    window.gesture = None;
}

/// Commits the zone gesture in progress from its start to the live data.
fn finish_gesture(window: &mut WaterWindow, commands: &mut Commands, root: Entity) {
    let Some(before) = window.gesture.take() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let Some(live) = world.get::<LifMapData>(root).cloned() else {
            return;
        };
        commit_change(world, root, before, live.water, live.water_tiles);
    });
}

fn water_window(
    mut contexts: EguiContexts,
    mut mode: ResMut<ToolMode>,
    mut window: ResMut<WaterWindow>,
    layout: Option<Res<MapLayout>>,
    data: Query<&LifMapData>,
    mut commands: Commands,
) -> Result {
    let Some(root) = layout.map(|layout| layout.root) else {
        return Ok(());
    };
    let Ok(data) = data.get(root) else {
        return Ok(());
    };
    let ToolMode::Water(mut brush) = *mode else {
        finish_gesture(&mut window, &mut commands, root);
        return Ok(());
    };
    let mut water = data.water.clone();
    let mut action = None;
    let mut responses = Vec::new();
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Water").show(ctx, |ui| {
        let Some(water) = &mut water else {
            if ui.button("Add water").clicked() {
                action = Some(Action::Add);
            }
            return;
        };
        if ui.button("Remove water").clicked() {
            action = Some(Action::Remove);
        }
        zones_ui(ui, water, &mut responses);
        ui.label(format!("{} tiles", data.water_tiles.len()));
        if ui
            .button("Recompute tiles from heights")
            .on_hover_text("every cell with a corner below the zone's wave threshold")
            .clicked()
        {
            action = Some(Action::Recompute);
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Tile brush radius (cells)");
            ui.add(egui::DragValue::new(&mut brush.radius).range(0..=MAX_RADIUS));
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut brush.add, true, "Add tiles");
            ui.radio_value(&mut brush.add, false, "Remove tiles");
        });
    });
    mode.set_if_neq(ToolMode::Water(brush));

    if let Some(action) = action {
        finish_gesture(&mut window, &mut commands, root);
        commands.queue(move |world: &mut World| match action {
            Action::Add => add_water(world),
            Action::Remove => commit_water(world, None, Vec::new()),
            Action::Recompute => commit_recomputed_tiles(world),
        });
        return Ok(());
    }
    let started = responses
        .iter()
        .any(|r| r.drag_started() || r.gained_focus());
    let stopped = responses.iter().any(|r| r.drag_stopped() || r.lost_focus());
    if started && window.gesture.is_none() {
        window.gesture = Some(data.clone());
    }
    if water != data.water {
        let before = data.clone();
        let tiles = data.water_tiles.clone();
        match window.gesture {
            // Live without respawning: rebuilding the water every frame of a
            // drag is too slow; the commit at the end respawns it.
            Some(_) => commands.queue(move |world: &mut World| {
                if let Some(mut data) = world.get_mut::<LifMapData>(root) {
                    data.water = water;
                }
            }),
            None => commands
                .queue(move |world: &mut World| commit_change(world, root, before, water, tiles)),
        }
    }
    if stopped {
        finish_gesture(&mut window, &mut commands, root);
    }
    Ok(())
}

fn zones_ui(ui: &mut egui::Ui, water: &mut LifWater, responses: &mut Vec<egui::Response>) {
    ui.label(format!(
        "{} x {} zones over {} x {} cells",
        water.split_width, water.split_height, water.width, water.height
    ));
    egui::Grid::new("water_zones")
        .num_columns(7)
        .striped(true)
        .show(ui, |ui| {
            for header in [
                "Zone",
                "Level",
                "Type",
                "Wave h",
                "Wave speed",
                "Pitch",
                "Anim",
            ] {
                ui.label(header);
            }
            ui.end_row();
            for (index, zone) in water.zones.iter_mut().enumerate() {
                ui.label(index.to_string());
                responses.push(
                    ui.add(egui::DragValue::new(&mut zone.level).speed(0.1))
                        .on_hover_text("native, positive down; 0 hides the zone"),
                );
                responses.push(ui.add(egui::DragValue::new(&mut zone.water_type)));
                responses.push(ui.add(egui::DragValue::new(&mut zone.wave_height).speed(0.05)));
                responses.push(ui.add(egui::DragValue::new(&mut zone.wave_speed).speed(0.05)));
                responses.push(ui.add(egui::DragValue::new(&mut zone.wave_pitch).speed(0.5)));
                responses.push(ui.add(egui::DragValue::new(&mut zone.anim_speed)));
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painting_tiles_keeps_them_row_major_and_unique() {
        let tiles = vec![(1, 0), (0, 1)];
        assert_eq!(
            paint_tiles(&tiles, [(0, 0), (1, 0)], true),
            vec![(0, 0), (1, 0), (0, 1)]
        );
        assert_eq!(paint_tiles(&tiles, [(1, 0), (5, 5)], false), vec![(0, 1)]);
    }

    #[test]
    fn a_world_point_maps_to_its_gnd_cell() {
        let water = new_water(3, 2);
        let center = |x: f32, y: f32| Vec3::new((x + 0.5) * CELL_SIZE, 0.0, -(y + 0.5) * CELL_SIZE);
        assert_eq!(gnd_cell_at(&water, center(2.0, 1.0)), Some((2, 1)));
        assert_eq!(gnd_cell_at(&water, center(3.0, 0.0)), None);
        assert_eq!(gnd_cell_at(&water, Vec3::new(1.0, 0.0, 1.0)), None);
    }
}
