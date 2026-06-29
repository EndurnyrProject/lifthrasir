//! Chat box: appends incoming `ChatHeard` lines to the history text and sends the
//! input on submit via the engine's `ChatSendRequested` event.
//!
//! Built as raw `bevy_ui` by [`spawn_chat_box`] (called from the HUD root). The input
//! is an `EditableText` field; focus gating is global
//! (`crate::focus::mirror_text_input_focus` flips `UiFocus.text_input_active` while
//! a text input holds focus), so typing in chat stops WASD/hotkeys without extra
//! wiring here.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use game_engine::core::state::GameState;
use game_engine::domain::character::chat::ChatSendRequested;
use game_engine::infrastructure::networking::zone_messages::ChatHeard;

use crate::theme;
use crate::widgets::placeholder::Placeholder;

/// Oldest lines past this are dropped so the text (and its layout) stays bounded.
const MAX_CHAT_LINES: usize = 100;
const CHAT_MAX_CHARS: usize = 255;

const TAB_ACTIVE_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const PILL_BG: Color = Color::srgba(0.184, 0.824, 0.478, 0.14);
const PILL_BORDER: Color = Color::srgba(0.184, 0.824, 0.478, 0.30);
const INPUT_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.34);

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
            (append_incoming_chat, chat_input_control).run_if(in_state(GameState::InGame)),
        );
    }
}

/// Builds the chat box under `parent`. The input is cleared on submit by
/// [`chat_input_control`].
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
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.043, 0.067, 0.059, 0.93)),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    spawn_tabs(commands, chat_box, font.clone());

    // Column pinned to the bottom + clipped overflow keeps the newest lines visible
    // (older ones scroll off the top) without a scrollbar widget. The top hairline
    // separates the log from the tab strip.
    let scroll = commands
        .spawn((
            Node {
                height: Val::Px(140.0),
                margin: UiRect::horizontal(Val::Px(6.0)),
                padding: UiRect::axes(Val::Px(13.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                border: UiRect::top(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor {
                top: theme::STROKE,
                right: Color::NONE,
                bottom: Color::NONE,
                left: Color::NONE,
            },
            Pickable::IGNORE,
            ChildOf(chat_box),
        ))
        .id();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: font.clone().into(),
            font_size: 12.5.into(),
            ..default()
        },
        TextColor(Color::srgb_u8(0xcd, 0xd8, 0xd0)),
        ChatHistory,
        Pickable::IGNORE,
        ChildOf(scroll),
    ));

    spawn_input(commands, chat_box, font);
}

/// Static channel tabs (visual only — switching is not wired up yet). The first
/// tab is the active one; "Party" carries a gold ping dot.
fn spawn_tabs(commands: &mut Commands, chat_box: Entity, font: Handle<Font>) {
    let tabs = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect {
                    top: Val::Px(7.0),
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    bottom: Val::Px(0.0),
                },
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(chat_box),
        ))
        .id();
    chat_tab(commands, tabs, "All", true, false, font.clone());
    chat_tab(commands, tabs, "Party", false, true, font.clone());
    chat_tab(commands, tabs, "Guild", false, false, font.clone());
    chat_tab(commands, tabs, "Trade", false, false, font);
}

fn chat_tab(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    active: bool,
    ping: bool,
    font: Handle<Font>,
) {
    let tab = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(13.0), Val::Px(6.0)),
                border_radius: BorderRadius {
                    top_left: Val::Px(8.0),
                    top_right: Val::Px(8.0),
                    bottom_left: Val::Px(0.0),
                    bottom_right: Val::Px(0.0),
                },
                ..default()
            },
            BackgroundColor(if active { TAB_ACTIVE_BG } else { Color::NONE }),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: font.into(),
            font_size: 11.5.into(),
            ..default()
        },
        TextColor(if active {
            theme::TEXT
        } else {
            theme::TEXT_FAINT
        }),
        Pickable::IGNORE,
        ChildOf(tab),
    ));
    if active {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(11.0),
                right: Val::Px(11.0),
                height: Val::Px(2.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            Pickable::IGNORE,
            ChildOf(tab),
        ));
    }
    if ping {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(3.0),
                right: Val::Px(4.0),
                width: Val::Px(5.0),
                height: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(theme::GOLD),
            Pickable::IGNORE,
            ChildOf(tab),
        ));
    }
}

/// The input bar: channel pill + borderless text field + send button, all inside a
/// single rounded container. Enter still submits (the send button is decorative).
fn spawn_input(commands: &mut Commands, chat_box: Entity, font: Handle<Font>) {
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                height: Val::Px(40.0),
                column_gap: Val::Px(9.0),
                margin: UiRect {
                    top: Val::Px(4.0),
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                },
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(9.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                },
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(INPUT_BG),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(chat_box),
        ))
        .id();

    let pill = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(PILL_BG),
            BorderColor::all(PILL_BORDER),
            Pickable::IGNORE,
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Text::new("All"),
        TextFont {
            font: font.clone().into(),
            font_size: 10.5.into(),
            ..default()
        },
        TextColor(theme::EMERALD_BRI),
        Pickable::IGNORE,
        ChildOf(pill),
    ));

    let input = commands
        .spawn((
            EditableText {
                max_characters: Some(CHAT_MAX_CHARS),
                ..default()
            },
            // Focus is driven by Enter/Escape (see `chat_input_control`), not by
            // clicking — `Pickable::IGNORE` stops a pointer press from focusing the
            // field, so a click can't trap the cursor with no way out.
            Pickable::IGNORE,
            TextFont {
                font: font.clone().into(),
                font_size: 13.0.into(),
                ..default()
            },
            TextColor(theme::TEXT),
            ChatInput,
            Node {
                flex_grow: 1.0,
                // ~one line tall (default line height = 1.2 * font) so the bar's
                // center alignment vertically centers the text instead of pinning it
                // to the top of a taller box.
                height: Val::Px(16.0),
                ..default()
            },
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Text::new("Press Enter to chat…"),
        TextFont {
            font: font.clone().into(),
            font_size: 13.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Pickable::IGNORE,
        Placeholder(input),
        ChildOf(input),
    ));

    let send = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(TAB_ACTIVE_BG),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Text::new("\u{21B5}"),
        TextFont {
            font: font.into(),
            font_size: 13.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_DIM),
        Pickable::IGNORE,
        ChildOf(send),
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
    mut received: MessageReader<ChatHeard>,
    mut history: Query<&mut Text, With<ChatHistory>>,
) {
    let Ok(mut text) = history.single_mut() else {
        return;
    };
    for event in received.read() {
        append_line(&mut text.0, &event.message);
    }
}

/// RO-style chat control. The core `EditableText` widget has no submit event, so we
/// drive everything off the keyboard:
///
/// - Unfocused + Enter opens the chat input (gating gameplay input while typing).
/// - Focused + Escape releases it without sending.
/// - Focused + Enter submits: a non-empty message is sent and the field cleared and
///   unfocused; an empty submit (e.g. the Enter that opened the chat) leaves it focused.
///
/// The input has `Pickable::IGNORE`, so Enter is the only way it gains focus — clicking
/// can't strand the cursor in the field.
fn chat_input_control(
    keys: Res<ButtonInput<KeyCode>>,
    mut chat_input: Query<(Entity, &mut EditableText), With<ChatInput>>,
    mut writer: MessageWriter<ChatSendRequested>,
    mut input_focus: ResMut<InputFocus>,
) {
    let Ok((entity, mut field)) = chat_input.single_mut() else {
        return;
    };
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);

    if input_focus.get() != Some(entity) {
        if enter {
            input_focus.set(entity, FocusCause::Navigated);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        input_focus.clear();
        return;
    }

    if enter {
        let value = field.value().to_string();
        let message = value.trim();
        if !message.is_empty() {
            writer.write(ChatSendRequested {
                message: message.to_string(),
            });
            field.clear();
            input_focus.clear();
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
    fn enter_focuses_chat_and_escape_releases_it() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<InputFocus>();
        app.add_message::<ChatSendRequested>();
        app.add_systems(Update, chat_input_control);
        let chat = app.world_mut().spawn((ChatInput, EditableText::default())).id();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(chat));

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
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
