//! Opening maps: the start screen and Open picker, the File menu, the
//! unsaved-changes guard, error dialogs, and turning a ready map scene into
//! an editable one.

use super::{
    document::SaveError,
    layout::{
        MapLayout, MapTextureList, map_name_from_path, resolve_layout, texture_list_from_gltf,
    },
    lighting::apply_map_lighting,
    save::SaveAsDialog,
    save::SaveRequest,
    session::{EditorSession, persist_preferences, record_recent_map},
    settings::MapSettingsWindow,
};
use crate::shell::{
    camera::{EditorCamera, frame_bounds},
    node::EditorNodeIds,
    ui::{EditorUiArea, EditorUiSet},
    undo::UndoStack,
};
use bevy::{
    asset::LoadState,
    gltf::{Gltf, GltfMaterial},
    prelude::*,
    window::{PrimaryWindow, WindowCloseRequested},
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_runtime::world::{
    LifMapData, MapSceneRequest, MapSceneStatus, MapSceneSystems, terrain::RO_UNITS_PER_CELL,
    water::begin_water_loading,
};

/// Which screen the editor shows.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EditorScreen {
    #[default]
    Start,
    Loading,
    Editing,
}

/// The map being loaded or edited.
#[derive(Resource, Debug, Clone)]
pub struct OpenMap {
    pub path: String,
    pub name: String,
    pub root: Entity,
}

/// Work currently being performed by the editor. Loading can span many frames;
/// saving also reports completion so an expensive write never looks ignored.
#[derive(Resource, Debug, Clone, Default)]
pub enum EditorOperation {
    #[default]
    Idle,
    Loading {
        name: String,
    },
    Saving {
        name: String,
    },
    Saved {
        name: String,
    },
}

/// Opens `path` (`ro://maps/<n>/<n>.glb`), through the unsaved-changes guard.
#[derive(Message, Debug, Clone)]
pub struct OpenMapRequest {
    pub path: String,
}

/// Start screen and menu actions other modules act on.
#[derive(Resource, Default)]
pub struct StartScreenActions {
    pub new_map_requested: bool,
}

/// A modal listing every message of a failed open or save; hidden while
/// `messages` is empty.
#[derive(Resource, Default)]
pub struct ErrorModal {
    pub title: String,
    pub messages: Vec<String>,
}

impl ErrorModal {
    pub fn show(&mut self, title: impl Into<String>, messages: Vec<String>) {
        self.title = title.into();
        self.messages = messages;
    }
}

/// An action that would discard unsaved changes.
#[derive(Debug, Clone)]
pub enum GuardedAction {
    Open(String),
    NewMap,
    Quit,
}

/// The action waiting for the user to discard unsaved changes or cancel.
#[derive(Resource, Default)]
pub struct UnsavedGuard(pub Option<GuardedAction>);

/// The map's `Gltf`, kept until the scene is ready to seed `MapTextureList`.
#[derive(Resource)]
struct OpeningGltf(Handle<Gltf>);

/// Open picker state; `maps` is listed from the source when first shown.
#[derive(Resource, Default)]
struct MapPicker {
    open: bool,
    filter: String,
    maps: Option<Vec<String>>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_state::<EditorScreen>()
        .add_message::<OpenMapRequest>()
        .add_message::<WindowCloseRequested>()
        .init_resource::<StartScreenActions>()
        .init_resource::<ErrorModal>()
        .init_resource::<UnsavedGuard>()
        .init_resource::<EditorOperation>()
        .init_resource::<MapPicker>()
        .add_systems(
            Update,
            (
                request_open,
                request_quit,
                finish_open
                    .after(MapSceneSystems::Readiness)
                    .run_if(in_state(EditorScreen::Loading)),
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                menu_bar.in_set(EditorUiSet::MenuBar),
                (map_picker, unsaved_guard, error_modal).in_set(EditorUiSet::Windows),
            ),
        );
}

/// Starts loading the last requested map, unless unsaved changes need
/// confirming first. The previous root is despawned with everything it owns.
#[allow(clippy::too_many_arguments)]
fn request_open(
    mut requests: MessageReader<OpenMapRequest>,
    mut commands: Commands,
    mut undo: ResMut<UndoStack>,
    mut guard: ResMut<UnsavedGuard>,
    mut errors: ResMut<ErrorModal>,
    mut picker: ResMut<MapPicker>,
    mut next: ResMut<NextState<EditorScreen>>,
    mut operation: ResMut<EditorOperation>,
    open: Option<Res<OpenMap>>,
    assets: Res<AssetServer>,
) {
    let Some(OpenMapRequest { path }) = requests.read().last().cloned() else {
        return;
    };
    if undo.is_dirty() {
        guard.0 = Some(GuardedAction::Open(path));
        return;
    }
    let Some(name) = map_name_from_path(&path) else {
        errors.show(
            "Cannot open map",
            vec![format!(
                "'{path}' is not a ro://maps/<name>/<name>.glb path"
            )],
        );
        return;
    };
    if let Some(open) = open {
        commands.entity(open.root).despawn();
    }
    undo.clear();
    commands.remove_resource::<MapLayout>();
    let root = commands
        .spawn((MapSceneRequest { path: path.clone() }, Transform::IDENTITY))
        .id();
    commands.insert_resource(OpeningGltf(assets.load(path.clone())));
    commands.insert_resource(OpenMap {
        path,
        name: name.clone(),
        root,
    });
    *operation = EditorOperation::Loading { name };
    picker.open = false;
    next.set(EditorScreen::Loading);
}

/// Window close requests (and File > Quit) exit, unless unsaved changes need
/// confirming first.
fn request_quit(
    mut closes: MessageReader<WindowCloseRequested>,
    undo: Res<UndoStack>,
    mut guard: ResMut<UnsavedGuard>,
    mut exit: MessageWriter<AppExit>,
) {
    if closes.read().last().is_none() {
        return;
    }
    if undo.is_dirty() {
        guard.0 = Some(GuardedAction::Quit);
        return;
    }
    exit.write(AppExit::Success);
}

/// Once the scene is ready and its `Gltf` loaded: layout, node ids, texture
/// list, lighting, water and camera framing. A failure shows the error and
/// returns to the start screen.
fn finish_open(world: &mut World) {
    let Some(open) = world.get_resource::<OpenMap>().cloned() else {
        return;
    };
    match world.get::<MapSceneStatus>(open.root) {
        Some(MapSceneStatus::Ready) => {}
        Some(MapSceneStatus::Failed(error)) => {
            let error = error.clone();
            return fail_open(world, &open, error);
        }
        _ => return,
    }
    match prepare_map(world, &open) {
        Ok(false) => {}
        Ok(true) => {
            world
                .resource_mut::<NextState<EditorScreen>>()
                .set(EditorScreen::Editing);
            let path = open.path.clone();
            let mut session = world.resource_mut::<EditorSession>();
            record_recent_map(&mut session, &path);
            *world.resource_mut::<EditorOperation>() = EditorOperation::Idle;
        }
        Err(error) => fail_open(world, &open, error),
    }
}

/// `Ok(false)` while the `Gltf` is still loading.
fn prepare_map(world: &mut World, open: &OpenMap) -> Result<bool, String> {
    let assets = world.resource::<AssetServer>().clone();
    let gltf = world.resource::<OpeningGltf>().0.clone();
    if let LoadState::Failed(error) = assets.load_state(&gltf) {
        return Err(format!("map '{}': {error}", open.path));
    }
    let Some(gltf) = world.resource::<Assets<Gltf>>().get(&gltf) else {
        return Ok(false);
    };
    let textures = texture_list_from_gltf(gltf, world.resource::<Assets<GltfMaterial>>())
        .map_err(|error| error.to_string())?;
    let layout = resolve_layout(world, open.root, &open.name).map_err(|error| error.to_string())?;
    let data = world
        .get::<LifMapData>(open.root)
        .cloned()
        .ok_or_else(|| SaveError::NotReady.to_string())?;
    let camera = world
        .query_filtered::<Entity, With<EditorCamera>>()
        .single(world)
        .map_err(|error| format!("editor camera: {error}"))?;

    assign_node_ids(world, &layout);
    world.entity_mut(open.root).insert(MapTextureList(textures));
    world.remove_resource::<OpeningGltf>();
    frame_map(world, camera, &data);
    let mut commands = world.commands();
    apply_map_lighting(&mut commands, &data.meta, camera);
    if let Some(water) = &data.water {
        begin_water_loading(
            &mut commands,
            &assets,
            open.root,
            water,
            data.water_tiles.clone(),
        );
    }
    world.insert_resource(layout);
    world.flush();
    Ok(true)
}

/// Every direct child of the map node except `terrain` gets an id.
fn assign_node_ids(world: &mut World, layout: &MapLayout) {
    let nodes: Vec<Entity> = world
        .get::<Children>(layout.map_node)
        .map(|children| children.to_vec())
        .unwrap_or_default();
    for node in nodes {
        if Some(node) == layout.terrain_node {
            continue;
        }
        let id = world.resource_mut::<EditorNodeIds>().allocate();
        world.entity_mut(node).insert(id);
    }
}

/// Frames the whole GAT area; RO maps span `+x` and `-z` from the origin.
fn frame_map(world: &mut World, camera: Entity, data: &LifMapData) {
    let width = data.altitude.width as f32 * RO_UNITS_PER_CELL;
    let height = data.altitude.height as f32 * RO_UNITS_PER_CELL;
    let (min, max) = (Vec3::new(0.0, 0.0, -height), Vec3::new(width, 0.0, 0.0));
    let mut entity = world.entity_mut(camera);
    if let Some(mut transform) = entity.get_mut::<Transform>() {
        frame_bounds(&mut transform, min, max);
    }
    if let Some(mut editor_camera) = entity.get_mut::<EditorCamera>() {
        editor_camera.focus = (min + max) * 0.5;
    }
}

fn fail_open(world: &mut World, open: &OpenMap, error: String) {
    world.entity_mut(open.root).despawn();
    world.remove_resource::<OpenMap>();
    world.remove_resource::<OpeningGltf>();
    world
        .resource_mut::<NextState<EditorScreen>>()
        .set(EditorScreen::Start);
    world
        .resource_mut::<ErrorModal>()
        .show(format!("Could not open {}", open.path), vec![error]);
    *world.resource_mut::<EditorOperation>() = EditorOperation::Idle;
}

/// Map names with a `maps/<n>/<n>.glb` in the source, sorted.
fn list_maps(source: &dyn AssetSource) -> Vec<String> {
    let mut maps: Vec<String> = source
        .list_files()
        .iter()
        .filter_map(|file| map_name_from_path(&format!("ro://{file}")))
        .collect();
    maps.sort();
    maps.dedup();
    maps
}

/// The File and Map menus: owns the top panel.
#[allow(clippy::too_many_arguments)]
fn menu_bar(
    mut contexts: EguiContexts,
    mut area: ResMut<EditorUiArea>,
    screen: Res<State<EditorScreen>>,
    layout: Option<Res<MapLayout>>,
    mut picker: ResMut<MapPicker>,
    mut actions: ResMut<StartScreenActions>,
    mut save_as: ResMut<SaveAsDialog>,
    mut settings: ResMut<MapSettingsWindow>,
    mut session: ResMut<EditorSession>,
    mut saves: MessageWriter<SaveRequest>,
    mut opens: MessageWriter<OpenMapRequest>,
    mut closes: MessageWriter<WindowCloseRequested>,
    windows: Query<Entity, With<PrimaryWindow>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let editing = *screen.get() == EditorScreen::Editing;
    area.show(ctx, "menu_bar", |ui| {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New map…").clicked() {
                        actions.new_map_requested = true;
                    }
                    if ui.button("Open…").clicked() {
                        picker.open = true;
                        picker.maps = None;
                    }
                    ui.menu_button("Open recent", |ui| {
                        if session.preferences.recent_maps.is_empty() {
                            ui.label("No recent maps");
                        }
                        for path in &session.preferences.recent_maps {
                            let label = map_name_from_path(path).unwrap_or_else(|| path.clone());
                            if ui.button(label).clicked() {
                                opens.write(OpenMapRequest { path: path.clone() });
                                ui.close();
                            }
                        }
                    });
                    if ui.add_enabled(editing, egui::Button::new("Save")).clicked() {
                        saves.write(SaveRequest { save_as: None });
                    }
                    if ui
                        .add_enabled(editing, egui::Button::new("Save as…"))
                        .clicked()
                        && let Some(layout) = &layout
                    {
                        save_as.open(&layout.name);
                    }
                    ui.separator();
                    let response = ui.checkbox(
                        &mut session.preferences.reopen_last_session,
                        "Reopen last map at startup",
                    );
                    if response.changed() {
                        persist_preferences(&session);
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        for window in &windows {
                            closes.write(WindowCloseRequested { window });
                        }
                    }
                });
                ui.menu_button("Map", |ui| {
                    if ui
                        .add_enabled(editing, egui::Button::new("Map settings…"))
                        .clicked()
                    {
                        settings.open = true;
                    }
                });
            });
        });
    });
    Ok(())
}

/// The start screen, and the Open picker once a map is open.
fn map_picker(
    mut contexts: EguiContexts,
    screen: Res<State<EditorScreen>>,
    mut picker: ResMut<MapPicker>,
    mut actions: ResMut<StartScreenActions>,
    source: Res<SharedCompositeAssetSource>,
    mut opens: MessageWriter<OpenMapRequest>,
) -> Result {
    let start = *screen.get() == EditorScreen::Start;
    if !start && !picker.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let MapPicker { open, filter, maps } = &mut *picker;
    let maps = maps
        .get_or_insert_with(|| list_maps(&*source.0.read().expect("ro:// source lock poisoned")));
    let mut refresh = false;
    let mut window = egui::Window::new(if start {
        "Lifthrasir Editor"
    } else {
        "Open map"
    })
    .collapsible(false)
    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
    if !start {
        window = window.open(open);
    }
    window.show(ctx, |ui| {
        if start {
            if ui.button("New map…").clicked() {
                actions.new_map_requested = true;
            }
            ui.separator();
        }
        ui.horizontal(|ui| {
            ui.label("Filter");
            ui.text_edit_singleline(filter);
            refresh = ui.button("Refresh").clicked();
        });
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                for name in maps.iter().filter(|name| name.contains(filter.as_str())) {
                    if ui.button(name).clicked() {
                        opens.write(OpenMapRequest {
                            path: format!("ro://maps/{name}/{name}.glb"),
                        });
                    }
                }
            });
    });
    if refresh {
        picker.maps = None;
    }
    Ok(())
}

/// Asks to discard unsaved changes before the guarded action runs.
fn unsaved_guard(
    mut contexts: EguiContexts,
    mut guard: ResMut<UnsavedGuard>,
    mut undo: ResMut<UndoStack>,
    mut actions: ResMut<StartScreenActions>,
    mut opens: MessageWriter<OpenMapRequest>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let Some(action) = guard.0.clone() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let modal = egui::Modal::new(egui::Id::new("unsaved_guard")).show(ctx, |ui| {
        ui.heading("Unsaved changes");
        ui.label("The open map has unsaved changes. Discard them?");
        ui.horizontal(|ui| {
            let discard = ui.button("Discard").clicked();
            let cancel = ui.button("Cancel").clicked();
            (discard, cancel)
        })
        .inner
    });
    let (discard, cancel) = modal.inner;
    if cancel || modal.should_close() {
        guard.0 = None;
    }
    if !discard {
        return Ok(());
    }
    guard.0 = None;
    undo.clear();
    match action {
        GuardedAction::Open(path) => {
            opens.write(OpenMapRequest { path });
        }
        GuardedAction::NewMap => actions.new_map_requested = true,
        GuardedAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
    Ok(())
}

fn error_modal(mut contexts: EguiContexts, mut errors: ResMut<ErrorModal>) -> Result {
    if errors.messages.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let modal = egui::Modal::new(egui::Id::new("error_modal")).show(ctx, |ui| {
        ui.heading(&errors.title);
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for message in &errors.messages {
                    ui.label(message);
                }
            });
        ui.button("OK").clicked()
    });
    if modal.inner || modal.should_close() {
        errors.messages.clear();
    }
    Ok(())
}
