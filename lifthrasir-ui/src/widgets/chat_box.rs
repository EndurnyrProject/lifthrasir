//! Chat box: appends incoming `ChatReceived` lines to the history paragraph and
//! sends the input on submit via the engine's `ChatSendRequested` event.
//!
//! Focus gating is already global (`focus::mirror_text_input_focus` flips
//! `UiFocus.text_input_active` whenever any `InputField` is focused), so typing in
//! chat stops WASD/hotkeys without extra wiring here.

use bevy::prelude::*;
use bevy_extended_ui::html::HtmlSubmit;
use bevy_extended_ui::styles::CssID;
use bevy_extended_ui::widgets::{InputField, Paragraph};
use bevy_extended_ui_macros::html_fn;
use game_engine::core::state::GameState;
use game_engine::domain::character::chat::ChatSendRequested;
use game_engine::infrastructure::networking::protocol::zone::ChatReceived;

const CHAT_HISTORY_ID: &str = "chat-history";
const CHAT_INPUT_ID: &str = "chat-input";
/// Oldest lines past this are dropped so the paragraph (and its layout) stays bounded.
const MAX_CHAT_LINES: usize = 100;

pub struct ChatBoxPlugin;

impl Plugin for ChatBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            append_incoming_chat.run_if(in_state(GameState::InGame)),
        );
    }
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

/// Appends each incoming chat line to the history paragraph. If the HUD hasn't
/// mounted yet the messages are left unread for a later frame (Bevy retains them
/// briefly), so chat that arrives during the mount window isn't dropped.
///
/// ponytail: no programmatic scroll-to-end — extended_ui's `Scrollbar` has no
/// confirmed scroll-to API. Add it if long histories need autoscroll.
fn append_incoming_chat(
    mut received: MessageReader<ChatReceived>,
    mut history: Query<(&mut Paragraph, &CssID)>,
) {
    let Some((mut paragraph, _)) = history
        .iter_mut()
        .find(|(_, id)| id.0 == CHAT_HISTORY_ID)
    else {
        return;
    };
    for event in received.read() {
        append_line(&mut paragraph.text, &event.message);
    }
}

#[html_fn("send_chat")]
fn send_chat(
    In(event): In<HtmlSubmit>,
    mut writer: MessageWriter<ChatSendRequested>,
    mut inputs: Query<(&mut InputField, &CssID)>,
) {
    let message = event.data.get("chat").cloned().unwrap_or_default();
    if message.trim().is_empty() {
        return;
    }
    writer.write(ChatSendRequested { message });
    for (mut input, id) in &mut inputs {
        if id.0 == CHAT_INPUT_ID {
            input.text.clear();
        }
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
        assert_eq!(*lines.last().unwrap(), format!("line{}", MAX_CHAT_LINES + 4));
    }
}
