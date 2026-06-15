//! Chat box: appends incoming `ChatReceived` lines to the history text and sends the
//! input on submit via the engine's `ChatSendRequested` event.
//!
//! Built as raw `bevy_ui` by [`spawn_chat_box`] (called from the HUD root). The input
//! is a `bevy_ui_text_input` field; focus gating is global
//! (`crate::focus::mirror_text_input_focus` flips `UiFocus.text_input_active` while
//! a text input holds focus), so typing in chat stops WASD/hotkeys without extra
//! wiring here.

use bevy::prelude::*;
use bevy_ui_text_input::{SubmitText, TextInputMode, TextInputNode, TextInputPrompt};
use game_engine::core::state::GameState;
use game_engine::domain::character::chat::ChatSendRequested;
use game_engine::infrastructure::networking::protocol::zone::ChatReceived;

use crate::theme;

/// Oldest lines past this are dropped so the text (and its layout) stays bounded.
const MAX_CHAT_LINES: usize = 100;
const CHAT_MAX_CHARS: usize = 255;

/// The chat history text element.
#[derive(Component)]
struct ChatHistory;

/// The chat input field. Used to filter [`SubmitText`] to this input.
#[derive(Component)]
struct ChatInput;

pub struct ChatBoxPlugin;

impl Plugin for ChatBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (append_incoming_chat, send_chat).run_if(in_state(GameState::InGame)),
        );
    }
}

/// Builds the chat box under `parent`. The input clears on submit (`clear_on_submit`),
/// so no manual clear is needed.
pub fn spawn_chat_box(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font = asset_server.load(theme::FONT_BODY);

    let chat_box = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(392.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.043, 0.067, 0.059, 0.93)),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    // Column pinned to the bottom + clipped overflow keeps the newest lines visible
    // (older ones scroll off the top) without a scrollbar widget.
    let scroll = commands
        .spawn((
            Node {
                height: Val::Px(140.0),
                padding: UiRect::axes(Val::Px(13.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(chat_box),
        ))
        .id();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: font.clone(),
            font_size: 12.5,
            ..default()
        },
        TextColor(Color::srgb_u8(0xcd, 0xd8, 0xd0)),
        ChatHistory,
        Pickable::IGNORE,
        ChildOf(scroll),
    ));

    let form = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(chat_box),
        ))
        .id();
    commands.spawn((
        TextInputNode {
            mode: TextInputMode::SingleLine,
            max_chars: Some(CHAT_MAX_CHARS),
            clear_on_submit: true,
            ..default()
        },
        TextInputPrompt::new("Enter to chat"),
        TextFont {
            font,
            font_size: 13.0,
            ..default()
        },
        TextColor(theme::TEXT),
        ChatInput,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(40.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(9.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.34)),
        BorderColor::all(theme::STROKE),
        ChildOf(form),
    ));
}

/// Appends `line` to the newline-joined history, capping at `MAX_CHAT_LINES`.
fn append_line(history: &mut String, line: &str) {
    if !history.is_empty() {
        history.push('\n');
    }
    history.push_str(line);

    let line_count = history.matches('\n').count() + 1;
    if line_count > MAX_CHAT_LINES {
        let drop = line_count - MAX_CHAT_LINES;
        if let Some((idx, _)) = history.match_indices('\n').nth(drop - 1) {
            history.replace_range(..=idx, "");
        }
    }
}

fn append_incoming_chat(
    mut received: MessageReader<ChatReceived>,
    mut history: Query<&mut Text, With<ChatHistory>>,
) {
    let Ok(mut text) = history.single_mut() else {
        return;
    };
    for event in received.read() {
        append_line(&mut text.0, &event.message);
    }
}

fn send_chat(
    mut submits: MessageReader<SubmitText>,
    inputs: Query<(), With<ChatInput>>,
    mut writer: MessageWriter<ChatSendRequested>,
) {
    for event in submits.read() {
        if !inputs.contains(event.entity) {
            continue;
        }
        let message = event.text.trim();
        if message.is_empty() {
            continue;
        }
        writer.write(ChatSendRequested {
            message: message.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_line_joins_with_newline() {
        let mut history = String::new();
        append_line(&mut history, "a");
        append_line(&mut history, "b");
        assert_eq!(history, "a\nb");
    }

    #[test]
    fn append_line_caps_oldest_lines() {
        let mut history = String::new();
        for i in 0..(MAX_CHAT_LINES + 5) {
            append_line(&mut history, &format!("line{i}"));
        }
        let lines: Vec<&str> = history.split('\n').collect();
        assert_eq!(lines.len(), MAX_CHAT_LINES);
        assert_eq!(lines[0], "line5");
        assert_eq!(
            *lines.last().unwrap(),
            format!("line{}", MAX_CHAT_LINES + 4)
        );
    }
}
