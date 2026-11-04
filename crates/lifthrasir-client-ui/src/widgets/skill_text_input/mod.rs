//! Server-requested text entry for Graffiti and Talkie Box.

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use net_contract::commands::SkillTextInputAnswered;
use net_contract::events::{MapChangeRequested, SkillTextInputRequested, ZoneDisconnected};
use net_contract::state::ZoneSessionGeneration;
use std::time::Duration;

mod scene;
#[cfg(test)]
mod tests;

pub struct SkillTextInputPlugin;

#[derive(Resource)]
pub(crate) struct ActiveSkillTextInput {
    request_id: u64,
    skill_id: u32,
    max_utf8_bytes: u32,
    deadline: Duration,
    generation: ZoneSessionGeneration,
}

#[derive(Component, Default, Clone)]
struct SkillTextInputRoot;

#[derive(Component, Default, Clone)]
struct SkillTextInputField;

#[derive(Component, Default, Clone, Copy)]
struct PromptAction {
    request_id: u64,
    confirm: bool,
}

#[derive(Message)]
struct QueuedAnswer {
    request_id: u64,
    confirm: bool,
}

impl Plugin for SkillTextInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<QueuedAnswer>();
        app.add_observer(clear_input_focus);
        app.add_systems(
            PostUpdate,
            (receive_prompt, answer_prompt, sync_window)
                .chain()
                .after(bevy::text::EditableTextSystems),
        );
        app.add_systems(Update, keyboard_answer.run_if(in_state(GameState::InGame)));
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveSkillTextInput>();
        });
    }
}

#[derive(SystemParam)]
struct PromptLifecycle<'w, 's> {
    disconnected: MessageReader<'w, 's, ZoneDisconnected>,
    map_changes: MessageReader<'w, 's, MapChangeRequested>,
    generation: Res<'w, ZoneSessionGeneration>,
    state: Res<'w, State<GameState>>,
    next_state: Res<'w, NextState<GameState>>,
    time: Res<'w, Time<Real>>,
}

fn receive_prompt(
    mut requests: MessageReader<SkillTextInputRequested>,
    mut lifecycle: PromptLifecycle,
    active: Option<Res<ActiveSkillTextInput>>,
    mut commands: Commands,
) {
    let disconnected = lifecycle.disconnected.read().count() > 0;
    let map_changed = lifecycle.map_changes.read().count() > 0;
    let request = requests.read().last();
    if disconnected
        || map_changed
        || *lifecycle.state.get() != GameState::InGame
        || matches!(&*lifecycle.next_state, NextState::Pending(state) if *state != GameState::InGame)
    {
        commands.remove_resource::<ActiveSkillTextInput>();
        return;
    }
    if let Some(request) = request {
        if !matches!(request.skill_id, 220 | 125) {
            warn!(
                skill_id = request.skill_id,
                "Ignoring unsupported skill text input request"
            );
            commands.remove_resource::<ActiveSkillTextInput>();
            return;
        }
        commands.insert_resource(ActiveSkillTextInput {
            request_id: request.request_id,
            skill_id: request.skill_id,
            max_utf8_bytes: request.max_utf8_bytes,
            deadline: lifecycle.time.elapsed() + Duration::from_secs(60),
            generation: *lifecycle.generation,
        });
    } else if active.is_some_and(|active| {
        active.generation != *lifecycle.generation || lifecycle.time.elapsed() >= active.deadline
    }) {
        commands.remove_resource::<ActiveSkillTextInput>();
    }
}

fn on_action(
    activate: On<Activate>,
    actions: Query<&PromptAction>,
    mut queued: MessageWriter<QueuedAnswer>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    queued.write(QueuedAnswer {
        request_id: action.request_id,
        confirm: action.confirm,
    });
}

fn keyboard_answer(
    keys: Res<ButtonInput<KeyCode>>,
    active: Option<Res<ActiveSkillTextInput>>,
    fields: Query<Entity, With<SkillTextInputField>>,
    focus: Res<InputFocus>,
    mut queued: MessageWriter<QueuedAnswer>,
) {
    let Some(active) = active else { return };
    let confirm = if keys.just_pressed(KeyCode::Escape) {
        false
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let Ok(entity) = fields.single() else {
            return;
        };
        if focus.get() != Some(entity) {
            return;
        }
        true
    } else {
        return;
    };
    queued.write(QueuedAnswer {
        request_id: active.request_id,
        confirm,
    });
}

fn answer_prompt(
    mut queued: MessageReader<QueuedAnswer>,
    active: Option<Res<ActiveSkillTextInput>>,
    fields: Query<&EditableText, With<SkillTextInputField>>,
    mut answered: MessageWriter<SkillTextInputAnswered>,
    mut commands: Commands,
) {
    let mut pending = active.as_deref();
    for answer in queued.read() {
        let Some(active) = pending else { continue };
        if answer.request_id != active.request_id {
            continue;
        }
        let raw_text = if answer.confirm {
            let Ok(field) = fields.single() else { continue };
            let mut bytes = field.value().to_string().into_bytes();
            bytes.truncate(active.max_utf8_bytes as usize);
            Some(bytes)
        } else {
            None
        };
        answered.write(SkillTextInputAnswered {
            request_id: active.request_id,
            raw_text,
        });
        commands.remove_resource::<ActiveSkillTextInput>();
        pending = None;
    }
}

fn clear_input_focus(removed: On<Remove<SkillTextInputField>>, mut focus: ResMut<InputFocus>) {
    if focus.get() == Some(removed.entity) {
        focus.clear();
    }
}

fn sync_window(
    active: Option<Res<ActiveSkillTextInput>>,
    roots: Query<Entity, With<SkillTextInputRoot>>,
    mut commands: Commands,
) {
    if active.as_ref().is_some_and(|active| !active.is_changed()) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    if let Some(active) = active {
        commands
            .spawn_scene(scene::window(
                active.request_id,
                active.skill_id,
                active.max_utf8_bytes,
            ))
            .insert(DespawnOnExit(GameState::InGame));
    }
}
