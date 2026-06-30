//! Login screen.
//!
//! Built as raw `bevy_ui`. The username/password fields are a small hand-rolled
//! masked input ([`TextField`]) rather than the core `EditableText` widget, because
//! that widget has no password-masking mode — here the password renders as bullets.
//! Login owns both its fields, so focus is tracked on the field itself
//! ([`TextField::focused`]) with no cross-widget coordination; [`crate::focus`] reads
//! that flag to gate gameplay input while typing.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::authentication::events::LoginFailureEvent;
use game_engine::presentation::ui::events::LoginAttemptEvent;
use net_contract::dto::NetworkError;
use secrecy::SecretString;

use crate::theme;
use crate::widgets::settings_window::SettingsWindowRoot;

const LOGO_IMAGE: &str = "ro://logo.png";
const USERNAME_MAX: usize = 24;
const PASSWORD_MAX: usize = 32;

pub struct LoginScreenPlugin;

/// A hand-rolled single-line text field. `mask` renders the value as bullets (the
/// password). `focused` is the single source of truth for which field receives
/// keystrokes and is read by the focus mirror to gate gameplay input.
#[derive(Component)]
pub struct TextField {
    value: String,
    placeholder: String,
    pub focused: bool,
    mask: bool,
    max: usize,
}

/// Which credential a [`TextField`] holds. Drives Tab order and submit gathering.
#[derive(Component, Clone, Copy, PartialEq)]
enum LoginField {
    Username,
    Password,
}

/// The `<p>` that surfaces login failures to the player.
#[derive(Component)]
struct LoginError;

impl Plugin for LoginScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Login), show_login_screen);
        app.add_systems(
            Update,
            (
                handle_login_input,
                render_login_fields,
                surface_login_failure,
            )
                .run_if(in_state(GameState::Login)),
        );
    }
}

fn show_login_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load(theme::FONT_BODY);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            DespawnOnExit(GameState::Login),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(432.0),
                padding: UiRect::all(Val::Px(40.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        ImageNode::new(asset_server.load(LOGO_IMAGE)),
        Node {
            width: Val::Px(280.0),
            height: Val::Px(152.0),
            margin: UiRect::bottom(Val::Px(18.0)),
            align_self: AlignSelf::Center,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(22.0)),
            ..default()
        },
        BackgroundColor(theme::GOLD_FAINT),
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    spawn_field_label(&mut commands, panel, "USERNAME", font.clone());
    spawn_field(
        &mut commands,
        panel,
        &asset_server,
        LoginField::Username,
        "user",
        "Enter your name",
        false,
        USERNAME_MAX,
        true,
        font.clone(),
    );

    spawn_field_label(&mut commands, panel, "PASSWORD", font.clone());
    spawn_field(
        &mut commands,
        panel,
        &asset_server,
        LoginField::Password,
        "lock",
        "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
        true,
        PASSWORD_MAX,
        false,
        font.clone(),
    );

    commands.spawn((
        Text::new(""),
        TextFont {
            font: font.clone().into(),
            font_size: 13.0.into(),
            ..default()
        },
        TextColor(theme::BAD),
        Node {
            min_height: Val::Px(18.0),
            margin: UiRect::bottom(Val::Px(12.0)),
            ..default()
        },
        LoginError,
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    let button = commands
        .spawn((
            Pickable::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new("Enter Realm"),
        TextFont {
            font: font.clone().into(),
            font_size: 15.0.into(),
            ..default()
        },
        TextColor(theme::EMERALD_INK),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    commands.entity(button).observe(submit_button);

    commands.spawn((
        Text::new("New to the realm? Create account"),
        TextFont {
            font: font.into(),
            font_size: 12.5.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Node {
            margin: UiRect::top(Val::Px(16.0)),
            align_self: AlignSelf::Center,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    let gear = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                width: Val::Px(36.0),
                height: Val::Px(36.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::default(),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        theme::icon(&asset_server, "gear", 18.0, theme::GOLD),
        ChildOf(gear),
    ));
    commands.entity(gear).observe(open_settings);
}

fn spawn_field_label(commands: &mut Commands, parent: Entity, text: &str, font: Handle<Font>) {
    commands.spawn((
        Text::new(text),
        TextFont {
            font: font.into(),
            font_size: 11.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_DIM),
        Node {
            margin: UiRect::bottom(Val::Px(7.0)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(parent),
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_field(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    kind: LoginField,
    icon: &str,
    placeholder: &str,
    mask: bool,
    max: usize,
    focused: bool,
    font: Handle<Font>,
) {
    let field = commands
        .spawn((
            TextField {
                value: String::new(),
                placeholder: placeholder.to_string(),
                focused,
                mask,
                max,
            },
            kind,
            Pickable::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                margin: UiRect::bottom(Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, icon, 17.0, theme::TEXT_FAINT),
        ChildOf(field),
    ));
    commands.spawn((
        Text::new(placeholder),
        TextFont {
            font: font.into(),
            font_size: 15.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Pickable::IGNORE,
        ChildOf(field),
    ));
    commands.entity(field).observe(
        move |_: On<Pointer<Click>>, mut fields: Query<(&mut TextField, &LoginField)>| {
            for (mut field, field_kind) in &mut fields {
                field.focused = *field_kind == kind;
            }
        },
    );
}

/// Reads `(username, password)` out of the field set, regardless of iteration order.
fn credentials<'a>(
    fields: impl Iterator<Item = (&'a TextField, &'a LoginField)>,
) -> (String, String) {
    let mut username = String::new();
    let mut password = String::new();
    for (field, kind) in fields {
        match kind {
            LoginField::Username => username = field.value.clone(),
            LoginField::Password => password = field.value.clone(),
        }
    }
    (username, password)
}

/// Pure seam: builds a `LoginAttemptEvent` so the credential wiring is unit-testable.
fn login_attempt(username: &str, password: &str) -> LoginAttemptEvent {
    LoginAttemptEvent {
        username: username.to_string(),
        password: SecretString::from(password.to_string()),
    }
}

/// User-readable message for a login failure. Pure seam for unit testing.
fn login_error_text(error: &NetworkError) -> String {
    error.to_string()
}

fn submit_button(
    _click: On<Pointer<Click>>,
    fields: Query<(&TextField, &LoginField)>,
    mut writer: MessageWriter<LoginAttemptEvent>,
) {
    let (username, password) = credentials(fields.iter());
    writer.write(login_attempt(&username, &password));
}

fn open_settings(
    _: On<Pointer<Click>>,
    mut window: Query<&mut Visibility, With<SettingsWindowRoot>>,
) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Visible;
    }
}

/// Routes keystrokes to the focused field: type/backspace edit it, Tab moves focus,
/// Enter submits the credentials.
fn handle_login_input(
    mut keys: MessageReader<KeyboardInput>,
    mut fields: Query<(&mut TextField, &LoginField)>,
    mut writer: MessageWriter<LoginAttemptEvent>,
) {
    for event in keys.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Tab => {
                let current = fields.iter().find(|(f, _)| f.focused).map(|(_, k)| *k);
                let next = match current {
                    Some(LoginField::Username) => LoginField::Password,
                    _ => LoginField::Username,
                };
                for (mut field, kind) in &mut fields {
                    field.focused = *kind == next;
                }
            }
            Key::Enter => {
                let (username, password) = credentials(fields.iter());
                writer.write(login_attempt(&username, &password));
            }
            Key::Backspace => {
                for (mut field, _) in &mut fields {
                    if field.focused {
                        field.value.pop();
                    }
                }
            }
            Key::Space => {
                for (mut field, _) in &mut fields {
                    if field.focused && field.value.chars().count() < field.max {
                        field.value.push(' ');
                    }
                }
            }
            Key::Character(input) => {
                for (mut field, _) in &mut fields {
                    if !field.focused {
                        continue;
                    }
                    for ch in input.chars() {
                        if !ch.is_control() && field.value.chars().count() < field.max {
                            field.value.push(ch);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Reflects each field's value (masked or not), placeholder, and focus border.
fn render_login_fields(
    mut fields: Query<(&TextField, &Children, &mut BorderColor), Changed<TextField>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (field, children, mut border) in &mut fields {
        let (shown, color) = if field.value.is_empty() {
            (field.placeholder.clone(), theme::TEXT_FAINT)
        } else if field.mask {
            ("\u{2022}".repeat(field.value.chars().count()), theme::TEXT)
        } else {
            (field.value.clone(), theme::TEXT)
        };
        *border = BorderColor::all(if field.focused {
            theme::EMERALD
        } else {
            theme::STROKE
        });
        for child in children.iter() {
            if let Ok((mut text, mut text_color)) = texts.get_mut(child) {
                *text = Text::new(shown.clone());
                *text_color = TextColor(color);
            }
        }
    }
}

/// Surfaces the most recent login failure into the error line. Success needs no
/// handling here: the engine transitions to `ServerSelection`.
fn surface_login_failure(
    mut failures: MessageReader<LoginFailureEvent>,
    mut errors: Query<&mut Text, With<LoginError>>,
) {
    let Some(failure) = failures.read().last() else {
        return;
    };
    let text = login_error_text(&failure.error);
    for mut error in &mut errors {
        *error = Text::new(text.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn field(kind: LoginField, value: &str) -> (TextField, LoginField) {
        (
            TextField {
                value: value.to_string(),
                placeholder: String::new(),
                focused: false,
                mask: matches!(kind, LoginField::Password),
                max: 32,
            },
            kind,
        )
    }

    #[test]
    fn credentials_reads_both_fields_in_any_order() {
        let password = field(LoginField::Password, "swordfish");
        let username = field(LoginField::Username, "adventurer");
        let (user, pass) =
            credentials([(&password.0, &password.1), (&username.0, &username.1)].into_iter());
        assert_eq!(user, "adventurer");
        assert_eq!(pass, "swordfish");
    }

    #[test]
    fn login_attempt_carries_username_and_password() {
        let attempt = login_attempt("adventurer", "swordfish");
        assert_eq!(attempt.username, "adventurer");
        assert_eq!(attempt.password.expose_secret(), "swordfish");
    }

    #[test]
    fn login_attempt_defaults_empty_fields() {
        let attempt = login_attempt("", "");
        assert_eq!(attempt.username, "");
        assert_eq!(attempt.password.expose_secret(), "");
    }

    #[test]
    fn error_text_renders_authentication_failure() {
        let error = NetworkError::AuthenticationFailed {
            reason: "bad credentials".to_string(),
        };

        assert_eq!(
            login_error_text(&error),
            "Authentication failed: bad credentials"
        );
    }

    #[test]
    fn error_text_renders_login_refused() {
        let error = NetworkError::LoginRefused { code: 1 };

        assert_eq!(
            login_error_text(&error),
            "Server refused login with code: 1"
        );
    }
}
