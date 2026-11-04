use super::*;
use bevy::scene::ScenePlugin;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

fn prompt_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        StatesPlugin,
    ));
    app.init_state::<GameState>()
        .init_resource::<ZoneSessionGeneration>()
        .add_message::<SkillTextInputRequested>()
        .add_message::<SkillTextInputAnswered>()
        .add_message::<MapChangeRequested>()
        .add_message::<ZoneDisconnected>();
    crate::install_test_ui_primitives(&mut app);
    app.add_plugins(SkillTextInputPlugin);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();
    app
}

fn offer(app: &mut App, request_id: u64, skill_id: u32, max_utf8_bytes: u32) {
    app.world_mut().write_message(SkillTextInputRequested {
        request_id,
        skill_id,
        max_utf8_bytes,
    });
    app.update();
}

fn activate(app: &mut App, confirm: bool, text: &str) {
    let world = app.world_mut();
    let mut fields = world.query_filtered::<&mut EditableText, With<SkillTextInputField>>();
    fields.single_mut(world).unwrap().editor.set_text(text);
    let entity = world
        .query::<(Entity, &PromptAction)>()
        .iter(world)
        .find_map(|(entity, action)| (action.confirm == confirm).then_some(entity))
        .unwrap();
    world.trigger(Activate { entity });
}

fn answers(app: &App) -> Vec<(u64, Option<Vec<u8>>)> {
    app.world()
        .resource::<Messages<SkillTextInputAnswered>>()
        .iter_current_update_messages()
        .map(|answer| (answer.request_id, answer.raw_text.clone()))
        .collect()
}

#[test]
fn confirm_preserves_raw_utf8_and_sends_only_once() {
    let mut app = prompt_app();
    offer(&mut app, 1, 220, 6);
    activate(&mut app, true, " é ");
    activate(&mut app, true, " é ");
    app.update();
    assert_eq!(answers(&app), vec![(1, Some(" é ".as_bytes().to_vec()))]);
    assert!(!app.world().contains_resource::<ActiveSkillTextInput>());
}

#[test]
fn overlong_input_truncates_bytes_even_inside_utf8() {
    let mut app = prompt_app();
    offer(&mut app, 2, 125, 3);
    activate(&mut app, true, "éé");
    app.update();
    assert_eq!(answers(&app), vec![(2, Some(vec![0xc3, 0xa9, 0xc3]))]);
}

#[test]
fn empty_confirmation_is_distinct_from_cancellation() {
    let mut app = prompt_app();
    offer(&mut app, 1, 220, 79);
    activate(&mut app, true, "");
    app.update();
    assert_eq!(answers(&app), vec![(1, Some(vec![]))]);
    app.world_mut()
        .resource_mut::<Messages<SkillTextInputAnswered>>()
        .clear();
    offer(&mut app, 2, 125, 79);
    activate(&mut app, false, "ignored");
    app.update();
    assert_eq!(answers(&app), vec![(2, None)]);
}

#[test]
fn replacement_discards_already_queued_old_answer_and_resets_field() {
    let mut app = prompt_app();
    offer(&mut app, 1, 220, 79);
    activate(&mut app, true, "obsolete");
    offer(&mut app, 2, 125, 79);
    assert!(answers(&app).is_empty());
    let world = app.world_mut();
    assert_eq!(
        world
            .query_filtered::<&EditableText, With<SkillTextInputField>>()
            .single(world)
            .unwrap()
            .value()
            .to_string(),
        ""
    );
    activate(&mut app, true, "new");
    app.update();
    assert_eq!(answers(&app), vec![(2, Some(b"new".to_vec()))]);
}

#[test]
fn timeout_uses_unpaused_time_and_drops_queued_answer_at_deadline() {
    let mut app = prompt_app();
    offer(&mut app, 1, 220, 79);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(60)));
    activate(&mut app, true, "late");
    app.update();
    assert!(answers(&app).is_empty());
    assert!(!app.world().contains_resource::<ActiveSkillTextInput>());
}

#[test]
fn zone_changes_disconnect_and_exit_discard_queued_answers_and_focus() {
    for invalidation in 0..4 {
        let mut app = prompt_app();
        offer(&mut app, 1, 220, 79);
        activate(&mut app, true, "stale");
        match invalidation {
            0 => {
                app.world_mut().write_message(MapChangeRequested {
                    map_name: "prontera".into(),
                    x: 1,
                    y: 1,
                });
            }
            1 => {
                app.world_mut().write_message(ZoneDisconnected {
                    reason: "lost".into(),
                });
            }
            2 => {
                app.world_mut().resource_mut::<ZoneSessionGeneration>().0 += 1;
            }
            _ => {
                app.world_mut()
                    .resource_mut::<NextState<GameState>>()
                    .set(GameState::CharacterSelection);
            }
        }
        app.update();
        assert!(answers(&app).is_empty(), "invalidation {invalidation}");
        assert!(!app.world().contains_resource::<ActiveSkillTextInput>());
        assert!(app.world().resource::<InputFocus>().get().is_none());
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<(), With<SkillTextInputRoot>>()
                .iter(world)
                .count(),
            0
        );
    }
}

#[test]
fn unsupported_request_invalidates_old_prompt_without_sending() {
    let mut app = prompt_app();
    offer(&mut app, 1, 220, 79);
    activate(&mut app, true, "obsolete");
    offer(&mut app, 2, 221, 79);
    assert!(answers(&app).is_empty());
    assert!(!app.world().contains_resource::<ActiveSkillTextInput>());
}

#[test]
fn escape_sends_cancel_and_enter_confirms_focused_field() {
    use bevy::input::keyboard::{Key, KeyboardInput};
    for (key_code, logical_key, expected) in [
        (KeyCode::Escape, Key::Escape, None),
        (KeyCode::Enter, Key::Enter, Some(vec![])),
    ] {
        let mut app = prompt_app();
        offer(&mut app, 1, 125, 79);
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: bevy::input::ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(answers(&app), vec![(1, expected)]);
    }
}

#[test]
fn confirmation_includes_text_edits_committed_in_the_same_frame() {
    let mut app = prompt_app();
    app.add_plugins(bevy::text::TextPlugin);
    offer(&mut app, 1, 220, 79);
    activate(&mut app, true, "");
    let world = app.world_mut();
    world
        .query_filtered::<&mut EditableText, With<SkillTextInputField>>()
        .single_mut(world)
        .unwrap()
        .queue_edit(bevy::text::TextEdit::Insert("last character".into()));
    app.update();
    assert_eq!(answers(&app), vec![(1, Some(b"last character".to_vec()))]);
}
