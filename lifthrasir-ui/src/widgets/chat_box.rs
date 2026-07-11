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
use net_contract::events::ChatHeard;

use crate::rich_text::spawn_colored_text;
use crate::theme;
use crate::widgets::party::slash::{parse_party_slash, PartySlashSubmitted};
use crate::widgets::placeholder::Placeholder;

/// Oldest lines past this are dropped so the history (and its layout) stays bounded.
const MAX_CHAT_LINES: usize = 100;
const CHAT_MAX_CHARS: usize = 255;
const CHAT_FONT_SIZE: f32 = 12.5;
const CHAT_DEFAULT_COLOR: Color = Color::srgb_u8(0xcd, 0xd8, 0xd0);

const TAB_ACTIVE_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const PILL_BG: Color = Color::srgba(0.184, 0.824, 0.478, 0.14);
const PILL_BORDER: Color = Color::srgba(0.184, 0.824, 0.478, 0.30);
const INPUT_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.34);

/// The scroll container that parents the per-line [`ChatLine`] entities. Sibling
/// widgets (e.g. the announcement echo) query it via `With<ChatHistory>`.
#[derive(Component)]
pub(crate) struct ChatHistory;

/// One rendered chat line (a `spawn_colored_text` root) under the [`ChatHistory`]
/// container. Tagged so the line cap can count and despawn oldest lines.
#[derive(Component)]
struct ChatLine;

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
    commands.spawn((
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
        ChatHistory,
        Pickable::IGNORE,
        ChildOf(chat_box),
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

/// Appends `text` as one `ChatLine` colored-text line under `container`, then
/// despawns the oldest `ChatLine` children past [`MAX_CHAT_LINES`]. Inline
/// `^RRGGBB` codes in `text` become colored runs over `default_color`. Shared by
/// normal chat and the announcement echo, hence `pub(crate)`.
pub(crate) fn append_colored_line(
    commands: &mut Commands,
    container: Entity,
    text: &str,
    default_color: Color,
    font: Handle<Font>,
) {
    let line = spawn_colored_text(
        commands,
        container,
        text,
        font,
        CHAT_FONT_SIZE,
        default_color,
    );
    commands.entity(line).insert(ChatLine);

    commands.queue(move |world: &mut World| {
        let Some(children) = world.get::<Children>(container) else {
            return;
        };
        let lines: Vec<Entity> = children
            .iter()
            .filter(|entity| world.get::<ChatLine>(*entity).is_some())
            .collect();
        if lines.len() <= MAX_CHAT_LINES {
            return;
        }
        let drop = lines.len() - MAX_CHAT_LINES;
        for entity in lines.into_iter().take(drop) {
            world.entity_mut(entity).despawn();
        }
    });
}

fn append_incoming_chat(
    mut received: MessageReader<ChatHeard>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if received.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);
    for event in received.read() {
        append_colored_line(
            &mut commands,
            container,
            &event.message,
            CHAT_DEFAULT_COLOR,
            font.clone(),
        );
    }
}

/// RO-style chat control. The core `EditableText` widget has no submit event, so we
/// drive everything off the keyboard:
///
/// - Unfocused + Enter opens the chat input (gating gameplay input while typing).
/// - Focused + Escape releases it without sending.
/// - Focused + Enter submits: a non-empty message is sent and the field cleared and
///   unfocused; an empty submit (e.g. the Enter that opened the chat) leaves it focused.
///   A recognized party slash command (`parse_party_slash`) is queued as
///   `PartySlashSubmitted` instead of a normal chat message.
///
/// The input has `Pickable::IGNORE`, so Enter is the only way it gains focus — clicking
/// can't strand the cursor in the field.
fn chat_input_control(
    keys: Res<ButtonInput<KeyCode>>,
    mut chat_input: Query<(Entity, &mut EditableText), With<ChatInput>>,
    mut writer: MessageWriter<ChatSendRequested>,
    mut slash_writer: MessageWriter<PartySlashSubmitted>,
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
            match parse_party_slash(message) {
                Some(slash) => {
                    slash_writer.write(PartySlashSubmitted(slash));
                }
                None => {
                    writer.write(ChatSendRequested {
                        message: message.to_string(),
                    });
                }
            }
            field.clear();
            input_focus.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    fn append_test_line(app: &mut App, container: Entity, text: &str) {
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, app.world());
            append_colored_line(
                &mut commands,
                container,
                text,
                CHAT_DEFAULT_COLOR,
                Handle::default(),
            );
        }
        queue.apply(app.world_mut());
    }

    #[test]
    fn colored_line_splits_into_runs() {
        let mut app = App::new();
        let container = app.world_mut().spawn_empty().id();
        append_test_line(&mut app, container, "Use ^ff0000fire^000000 here");

        let line = app
            .world()
            .get::<Children>(container)
            .and_then(|c| c.iter().next())
            .expect("one chat line");
        let text = app.world().get::<Text>(line).expect("line has text");
        assert_eq!(text.0, "Use ");
        let spans = app
            .world()
            .get::<Children>(line)
            .expect("line has run spans");
        assert_eq!(spans.iter().count(), 2);
    }

    fn chat_control_app(initial_text: &str) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<InputFocus>();
        app.add_message::<ChatSendRequested>();
        app.add_message::<PartySlashSubmitted>();
        app.add_systems(Update, chat_input_control);
        let chat = app
            .world_mut()
            .spawn((ChatInput, EditableText::new(initial_text)))
            .id();
        (app, chat)
    }

    fn chat_messages(app: &App) -> Vec<ChatSendRequested> {
        let messages = app.world().resource::<Messages<ChatSendRequested>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn slash_messages(app: &App) -> Vec<PartySlashSubmitted> {
        let messages = app.world().resource::<Messages<PartySlashSubmitted>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn enter_focuses_chat_and_escape_releases_it() {
        let (mut app, chat) = chat_control_app("");

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
    fn enter_with_normal_text_sends_chat_message() {
        let (mut app, chat) = chat_control_app("hello world");
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(chat, FocusCause::Navigated);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let sent = chat_messages(&app);
        assert_eq!(sent.len(), 1, "normal chat still sends one message");
        assert_eq!(sent[0].message, "hello world");
        assert!(slash_messages(&app).is_empty(), "no slash message written");
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            None,
            "field unfocused after submit"
        );
    }

    #[test]
    fn enter_with_party_slash_writes_slash_not_chat() {
        let (mut app, chat) = chat_control_app("/pcreate Wolfpack");
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(chat, FocusCause::Navigated);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let submitted = slash_messages(&app);
        assert_eq!(submitted.len(), 1, "one slash command queued");
        assert_eq!(
            submitted[0].0,
            crate::widgets::party::PartySlash::Create("Wolfpack".to_string())
        );
        assert!(
            chat_messages(&app).is_empty(),
            "a recognized slash never sends normal chat"
        );
    }

    #[test]
    fn append_colored_line_caps_oldest_children() {
        let mut app = App::new();
        let container = app.world_mut().spawn_empty().id();
        for i in 0..(MAX_CHAT_LINES + 5) {
            append_test_line(&mut app, container, &format!("line{i}"));
        }

        let lines: Vec<Entity> = {
            let world = app.world();
            let children = world
                .get::<Children>(container)
                .expect("container children");
            children
                .iter()
                .filter(|entity| world.get::<ChatLine>(*entity).is_some())
                .collect()
        };
        assert_eq!(lines.len(), MAX_CHAT_LINES);

        let oldest = app.world().get::<Text>(lines[0]).expect("oldest line text");
        assert_eq!(oldest.0, "line5");
        let newest = app
            .world()
            .get::<Text>(*lines.last().unwrap())
            .expect("newest line text");
        assert_eq!(newest.0, format!("line{}", MAX_CHAT_LINES + 4));
    }
}
