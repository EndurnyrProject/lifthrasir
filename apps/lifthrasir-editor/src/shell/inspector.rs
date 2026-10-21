//! The right panel: the selected node's name, transform and payload fields.
//!
//! Widgets edit a draft of one component. A drag or text edit writes the
//! draft live and commits one [`ComponentEdit`] when it ends; a one-frame
//! change commits right away.

use super::{
    node::{EditorNodeId, find_node},
    selection::{Selection, delete_selected},
    ui::{EditorUiArea, EditorUiSet},
    undo::{ComponentEdit, commit},
};
use crate::map::{open::EditorScreen, refresh::after_node_edit};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_assets::source::{AssetSource, SharedCompositeAssetSource};
use lifthrasir_runtime::world::{LifAudioEmitter, LifEffectEmitter, LifPropRef};

type Finish = Box<dyn FnOnce(&mut World) + Send + Sync>;

/// A drag or text edit in progress: commits the edit from its start value to
/// the node's live value.
struct ActiveGesture {
    node: EditorNodeId,
    changed: bool,
    finish: Finish,
}

/// The inspector's gesture in progress, if any.
#[derive(Resource, Default)]
pub struct Gesture(Option<ActiveGesture>);

impl Gesture {
    /// Turns one widget's response over a draft of `T` into live writes and
    /// commits.
    fn track<T: Component + Clone>(
        &mut self,
        commands: &mut Commands,
        target: (Entity, EditorNodeId),
        response: &egui::Response,
        before: &T,
        draft: &T,
    ) {
        let (entity, node) = target;
        if response.drag_started() || response.gained_focus() {
            self.finish(commands);
            self.0 = Some(ActiveGesture {
                node,
                changed: false,
                finish: finish_edit(node, before.clone()),
            });
        }
        if response.changed() {
            match &mut self.0 {
                Some(active) if active.node == node => {
                    active.changed = true;
                    commands.entity(entity).insert(draft.clone());
                }
                _ => {
                    let edit = ComponentEdit::new(node, before.clone(), draft.clone())
                        .with_hook(after_node_edit);
                    commands.queue(move |world: &mut World| commit(world, Box::new(edit)));
                }
            }
        }
        if response.drag_stopped() || response.lost_focus() {
            self.finish(commands);
        }
    }

    /// Commits the gesture in progress, if it changed anything.
    fn finish(&mut self, commands: &mut Commands) {
        let Some(active) = self.0.take() else {
            return;
        };
        if active.changed {
            commands.queue(active.finish);
        }
    }
}

/// Commits `before` to the node's live `T` (the gesture already wrote it).
fn finish_edit<T: Component + Clone>(node: EditorNodeId, before: T) -> Finish {
    Box::new(move |world: &mut World| {
        let Some(entity) = find_node(world, node) else {
            return;
        };
        let Some(after) = world.get::<T>(entity).cloned() else {
            return;
        };
        let edit = ComponentEdit::new(node, before, after).with_hook(after_node_edit);
        commit(world, Box::new(edit));
    })
}

/// Widgets over a draft of one component of the selected node.
struct Fields<'a, 'w, 's, T> {
    commands: &'a mut Commands<'w, 's>,
    gesture: &'a mut Gesture,
    target: (Entity, EditorNodeId),
    before: &'a T,
    draft: T,
}

impl<'a, 'w, 's, T: Component + Clone> Fields<'a, 'w, 's, T> {
    fn new(
        commands: &'a mut Commands<'w, 's>,
        gesture: &'a mut Gesture,
        target: (Entity, EditorNodeId),
        before: &'a T,
    ) -> Self {
        Self {
            commands,
            gesture,
            target,
            before,
            draft: before.clone(),
        }
    }

    fn add(
        &mut self,
        ui: &mut egui::Ui,
        widget: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
    ) {
        let response = widget(ui, &mut self.draft);
        self.gesture.track(
            self.commands,
            self.target,
            &response,
            self.before,
            &self.draft,
        );
    }

    /// A labelled row of widgets.
    fn row(&mut self, ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui, &mut Self)) {
        ui.horizontal(|ui| {
            ui.label(label);
            add(ui, self);
        });
    }
}

type Node<'a> = (
    Entity,
    &'a EditorNodeId,
    &'a Name,
    &'a Transform,
    Option<&'a LifPropRef>,
    Option<&'a LifAudioEmitter>,
    Option<&'a LifEffectEmitter>,
);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Gesture>().add_systems(
        EguiPrimaryContextPass,
        inspector
            .in_set(EditorUiSet::RightPanel)
            .run_if(in_state(EditorScreen::Editing)),
    );
}

fn inspector(
    mut contexts: EguiContexts,
    mut area: ResMut<EditorUiArea>,
    selection: Res<Selection>,
    mut gesture: ResMut<Gesture>,
    mut commands: Commands,
    nodes: Query<Node<'static>>,
    source: Res<SharedCompositeAssetSource>,
) -> Result {
    if gesture
        .0
        .as_ref()
        .is_some_and(|active| Some(active.node) != selection.0)
    {
        gesture.finish(&mut commands);
    }
    let node = selection
        .0
        .and_then(|id| nodes.iter().find(|node| *node.1 == id));
    let ctx = contexts.ctx_mut()?;
    area.show(ctx, "inspector", |ui| {
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                ui.heading("Inspector");
                ui.separator();
                let Some(node) = node else {
                    ui.label("Nothing selected");
                    return;
                };
                node_ui(ui, &mut commands, &mut gesture, &source, node);
            });
    });
    Ok(())
}

fn node_ui(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    gesture: &mut Gesture,
    source: &SharedCompositeAssetSource,
    (entity, id, name, transform, prop, sound, effect): Node<'_>,
) {
    let target = (entity, *id);
    let mut fields = Fields::new(commands, gesture, target, name);
    fields.row(ui, "Name", |ui, fields| {
        fields.add(ui, |ui, name| {
            let mut text = name.to_string();
            let response = ui.text_edit_singleline(&mut text);
            if response.changed() {
                *name = Name::new(text);
            }
            response
        });
    });
    transform_ui(ui, Fields::new(commands, gesture, target, transform));
    if let Some(prop) = prop {
        let missing = !source
            .0
            .read()
            .expect("ro:// source lock poisoned")
            .exists(prop.0.model.strip_prefix("ro://").unwrap_or(&prop.0.model));
        prop_ui(ui, Fields::new(commands, gesture, target, prop), missing);
    }
    if let Some(sound) = sound {
        sound_ui(ui, Fields::new(commands, gesture, target, sound));
    }
    if let Some(effect) = effect {
        effect_ui(ui, Fields::new(commands, gesture, target, effect));
    }
    ui.separator();
    if ui.button("Delete").clicked() {
        commands.queue(delete_selected);
    }
}

fn drag(value: &mut f32, speed: f32) -> egui::DragValue<'_> {
    egui::DragValue::new(value).speed(speed)
}

/// Translation, rotation (YXZ Euler degrees) and scale, in the scene's frame.
fn transform_ui(ui: &mut egui::Ui, mut fields: Fields<'_, '_, '_, Transform>) {
    ui.separator();
    fields.row(ui, "Position", |ui, fields| {
        for axis in 0..3 {
            fields.add(ui, |ui, t| ui.add(drag(&mut t.translation[axis], 0.1)));
        }
    });
    fields.row(ui, "Rotation", |ui, fields| {
        for axis in 0..3 {
            fields.add(ui, |ui, t| {
                let (y, x, z) = t.rotation.to_euler(EulerRot::YXZ);
                let mut degrees = [x, y, z].map(f32::to_degrees);
                let response = ui.add(drag(&mut degrees[axis], 0.5).suffix("°"));
                if response.changed() {
                    let [x, y, z] = degrees.map(f32::to_radians);
                    t.rotation = Quat::from_euler(EulerRot::YXZ, y, x, z);
                }
                response
            });
        }
    });
    fields.row(ui, "Scale", |ui, fields| {
        for axis in 0..3 {
            fields.add(ui, |ui, t| ui.add(drag(&mut t.scale[axis], 0.01)));
        }
    });
}

/// Model (red when missing from the source) and animation.
fn prop_ui(ui: &mut egui::Ui, mut fields: Fields<'_, '_, '_, LifPropRef>, missing: bool) {
    ui.separator();
    ui.strong("Prop");
    fields.row(ui, "Model", |ui, fields| {
        fields.add(ui, |ui, prop| {
            let mut edit = egui::TextEdit::singleline(&mut prop.0.model);
            if missing {
                edit = edit.text_color(egui::Color32::RED);
            }
            let response = ui.add(edit);
            if missing {
                return response.on_hover_text("missing from the source");
            }
            response
        });
    });
    fields.row(ui, "Anim type", |ui, fields| {
        fields.add(ui, |ui, prop| {
            ui.add(egui::DragValue::new(&mut prop.0.anim_type))
        });
    });
    fields.row(ui, "Anim speed", |ui, fields| {
        fields.add(ui, |ui, prop| ui.add(drag(&mut prop.0.anim_speed, 0.01)));
    });
}

fn sound_ui(ui: &mut egui::Ui, mut fields: Fields<'_, '_, '_, LifAudioEmitter>) {
    ui.separator();
    ui.strong("Sound");
    fields.row(ui, "File", |ui, fields| {
        fields.add(ui, |ui, sound| ui.text_edit_singleline(&mut sound.0.file));
    });
    fields.row(ui, "Volume", |ui, fields| {
        fields.add(ui, |ui, sound| ui.add(drag(&mut sound.0.volume, 0.01)));
    });
    fields.row(ui, "Range", |ui, fields| {
        fields.add(ui, |ui, sound| ui.add(drag(&mut sound.0.range, 0.5)));
    });
    fields.row(ui, "Cycle (s)", |ui, fields| {
        fields.add(ui, |ui, sound| ui.add(drag(&mut sound.0.cycle, 0.1)));
    });
}

fn effect_ui(ui: &mut egui::Ui, mut fields: Fields<'_, '_, '_, LifEffectEmitter>) {
    ui.separator();
    ui.strong("Effect");
    fields.row(ui, "Type", |ui, fields| {
        fields.add(ui, |ui, effect| {
            ui.add(egui::DragValue::new(&mut effect.0.effect_type))
        });
    });
    fields.row(ui, "Emit speed", |ui, fields| {
        fields.add(ui, |ui, effect| {
            ui.add(drag(&mut effect.0.emit_speed, 0.01))
        });
    });
    fields.row(ui, "Params", |ui, fields| {
        for index in 0..4 {
            fields.add(ui, |ui, effect| {
                ui.add(drag(&mut effect.0.params[index], 0.01))
            });
        }
    });
}
