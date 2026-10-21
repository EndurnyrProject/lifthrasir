//! The Map settings window: ambient color and brightness, exposure, the
//! indoor flag and the no-shade tint. Each change is one [`MapDataEdit`]
//! whose hook re-applies them; a drag or text edit previews live and commits
//! once, when it ends.

use super::{
    layout::MapLayout, lighting::apply_map_lighting, open::EditorScreen,
    refresh::respawn_prop_visuals,
};
use crate::shell::{
    camera::EditorCamera,
    ui::EditorUiSet,
    undo::{MapDataEdit, commit},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_data::lif::{LifMap, no_shade_tint};
use lifthrasir_runtime::world::{LifMapData, LifPropRef, PropSceneContext, SceneOwner};
use lifthrasir_tooling::converters::map::document::{
    INDOOR_AMBIENT_BRIGHTNESS, INDOOR_EXPOSURE_EV100, OUTDOOR_AMBIENT_BRIGHTNESS,
    OUTDOOR_EXPOSURE_EV100,
};

/// The Map settings window's state.
#[derive(Resource, Default)]
pub struct MapSettingsWindow {
    pub open: bool,
    /// The tint follows ambient and sun color. `None` until detected from the
    /// open map: on when its tint equals the formula.
    auto_tint: Option<bool>,
    /// The map data when the drag or text edit in progress started.
    gesture: Option<LifMapData>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<MapSettingsWindow>()
        .add_systems(OnExit(EditorScreen::Editing), reset_window)
        .add_systems(
            EguiPrimaryContextPass,
            settings_window
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing)),
        );
}

/// The [`EditHook`](crate::shell::undo::EditHook) of map settings edits:
/// re-applies ambient light and exposure and, when the tint changed, moves it
/// onto the root and every prop and respawns the props' visuals.
pub fn refresh_map_settings(world: &mut World, root: Entity) {
    let Some(meta) = world.get::<LifMapData>(root).map(|data| data.meta.clone()) else {
        return;
    };
    apply_lighting(world, &meta);
    let context = PropSceneContext {
        no_shade_tint: meta.no_shade_tint,
    };
    if world.get::<PropSceneContext>(root) == Some(&context) {
        return;
    }
    world.entity_mut(root).insert(context);
    let props: Vec<Entity> = world
        .query_filtered::<(Entity, &SceneOwner), With<LifPropRef>>()
        .iter(world)
        .filter_map(|(prop, owner)| (owner.0 == root).then_some(prop))
        .collect();
    for prop in props {
        world.entity_mut(prop).insert(context);
        respawn_prop_visuals(world, prop);
    }
}

/// Sets the indoor flag and its ambient brightness and exposure presets.
/// Light values are left as they are.
pub fn set_indoor(meta: &mut LifMap, indoor: bool) {
    meta.indoor = indoor;
    (meta.ambient_brightness, meta.exposure_ev100) = if indoor {
        (INDOOR_AMBIENT_BRIGHTNESS, INDOOR_EXPOSURE_EV100)
    } else {
        (OUTDOOR_AMBIENT_BRIGHTNESS, OUTDOOR_EXPOSURE_EV100)
    };
}

/// Commits `meta` as the open map's settings, as one undoable step.
pub fn commit_settings(world: &mut World, meta: LifMap) {
    let Some(root) = world.get_resource::<MapLayout>().map(|layout| layout.root) else {
        return;
    };
    let Some(before) = world.get::<LifMapData>(root).cloned() else {
        return;
    };
    commit_change(world, root, before, meta);
}

fn commit_change(world: &mut World, root: Entity, before: LifMapData, meta: LifMap) {
    if before.meta == meta {
        return;
    }
    let after = LifMapData {
        meta,
        ..before.clone()
    };
    commit(
        world,
        Box::new(MapDataEdit::new(root, before, after).with_hook(refresh_map_settings)),
    );
}

fn apply_lighting(world: &mut World, meta: &LifMap) {
    let Ok(camera) = world
        .query_filtered::<Entity, With<EditorCamera>>()
        .single(world)
    else {
        return;
    };
    let mut commands = world.commands();
    apply_map_lighting(&mut commands, meta, camera);
    world.flush();
}

/// Writes `meta` live and re-applies the lighting, without recording it or
/// re-shading props; the gesture's commit does both.
fn preview(world: &mut World, root: Entity, meta: LifMap) {
    if let Some(mut data) = world.get_mut::<LifMapData>(root) {
        data.meta = meta.clone();
    }
    apply_lighting(world, &meta);
}

fn reset_window(mut window: ResMut<MapSettingsWindow>) {
    window.auto_tint = None;
    window.gesture = None;
}

/// Commits the gesture in progress from its start to the live map data.
fn finish_gesture(window: &mut MapSettingsWindow, commands: &mut Commands, root: Entity) {
    let Some(before) = window.gesture.take() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let Some(meta) = world.get::<LifMapData>(root).map(|data| data.meta.clone()) else {
            return;
        };
        commit_change(world, root, before, meta);
    });
}

fn settings_window(
    mut contexts: EguiContexts,
    mut window: ResMut<MapSettingsWindow>,
    layout: Option<Res<MapLayout>>,
    data: Query<&LifMapData>,
    suns: Query<(&DirectionalLight, &SceneOwner)>,
    mut commands: Commands,
) -> Result {
    let Some(root) = layout.map(|layout| layout.root) else {
        return Ok(());
    };
    let Ok(data) = data.get(root) else {
        return Ok(());
    };
    let window = &mut *window;
    if !window.open {
        finish_gesture(window, &mut commands, root);
        return Ok(());
    }
    let sun = suns
        .iter()
        .find_map(|(light, owner)| (owner.0 == root).then(|| srgb(light.color)));
    let mut auto = *window.auto_tint.get_or_insert_with(|| {
        sun.is_some_and(|sun| {
            no_shade_tint(data.meta.ambient_color, sun) == data.meta.no_shade_tint
        })
    });
    let mut meta = data.meta.clone();
    let mut responses = Vec::new();
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Map settings")
        .open(&mut window.open)
        .show(ctx, |ui| {
            settings_ui(ui, &mut meta, &mut auto, sun.is_some(), &mut responses);
        });
    window.auto_tint = Some(auto);
    // Only on a change made here: a sun color drag must not commit from here.
    let touched = responses.iter().any(egui::Response::changed);
    if let (true, true, Some(sun)) = (touched, auto, sun) {
        meta.no_shade_tint = no_shade_tint(meta.ambient_color, sun);
    }

    let started = responses
        .iter()
        .any(|r| r.drag_started() || r.gained_focus());
    let stopped = responses.iter().any(|r| r.drag_stopped() || r.lost_focus());
    if started && window.gesture.is_none() {
        window.gesture = Some(data.clone());
    }
    if meta != data.meta {
        let before = data.clone();
        match window.gesture {
            Some(_) => commands.queue(move |world: &mut World| preview(world, root, meta)),
            None => {
                commands.queue(move |world: &mut World| commit_change(world, root, before, meta))
            }
        }
    }
    if stopped || !window.open {
        finish_gesture(window, &mut commands, root);
    }
    Ok(())
}

fn settings_ui(
    ui: &mut egui::Ui,
    meta: &mut LifMap,
    auto: &mut bool,
    has_sun: bool,
    responses: &mut Vec<egui::Response>,
) {
    egui::Grid::new("map_settings")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Ambient color");
            ui.horizontal(|ui| {
                for channel in &mut meta.ambient_color {
                    responses.push(ui.add(egui::DragValue::new(channel).speed(0.005)));
                }
            });
            ui.end_row();
            ui.label("Ambient brightness");
            responses.push(ui.add(egui::DragValue::new(&mut meta.ambient_brightness).speed(10.0)));
            ui.end_row();
            ui.label("Exposure EV100");
            responses.push(ui.add(egui::DragValue::new(&mut meta.exposure_ev100).speed(0.05)));
            ui.end_row();
            ui.label("Indoor");
            let mut indoor = meta.indoor;
            let response = ui
                .checkbox(&mut indoor, "")
                .on_hover_text("also sets the indoor/outdoor ambient brightness and exposure");
            if response.changed() {
                set_indoor(meta, indoor);
            }
            responses.push(response);
            ui.end_row();
            ui.label("No-shade tint");
            ui.horizontal(|ui| {
                responses.push(
                    ui.add_enabled(has_sun, egui::Checkbox::new(auto, "Auto"))
                        .on_hover_text("from the ambient and sun colors")
                        .on_disabled_hover_text("the map has no sun"),
                );
                ui.add_enabled_ui(!*auto, |ui| {
                    for channel in &mut meta.no_shade_tint {
                        responses.push(ui.add(egui::DragValue::new(channel).speed(0.005)));
                    }
                });
            });
            ui.end_row();
        });
}

fn srgb(color: Color) -> [f32; 3] {
    let srgba = color.to_srgba();
    [srgba.red, srgba.green, srgba.blue]
}
