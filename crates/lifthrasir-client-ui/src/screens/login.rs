//! Login screen.
//!
//! One BSN scene built from the RO-dark primitives ([`lifthrasir_ui::widgets::primitives`])
//! over the shared menu background. The username/password fields are a small hand-rolled
//! masked input ([`TextField`]) rather than the core `EditableText` widget, because
//! that widget has no password-masking mode — here the password renders as bullets.
//! Login owns both its fields, so focus is tracked on the field itself
//! ([`TextField::focused`]) with no cross-widget coordination; [`crate::focus`] reads
//! that flag to gate gameplay input while typing.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::authentication::events::LoginFailureEvent;
use lifthrasir_client::presentation::ui::events::LoginAttemptEvent;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, WINDOW_SHADOW, button, checkbox, divider, divider_diamond, icon_button, input_frame,
    title_text, window_title, window_titled,
};
use net_contract::dto::NetworkError;
use secrecy::SecretString;

use crate::widgets::settings_window::SettingsWindowRoot;

const USERNAME_MAX: usize = 24;
const PASSWORD_MAX: usize = 32;

/// Design window width, excluding the baked shadow.
const WINDOW_WIDTH: f32 = 360.0;

pub struct LoginScreenPlugin;

/// A hand-rolled single-line text field. `mask` renders the value as bullets (the
/// password). `focused` is the single source of truth for which field receives
/// keystrokes and is read by the focus mirror to gate gameplay input.
#[derive(Component, Default, Clone)]
pub struct TextField {
    value: String,
    placeholder: String,
    pub focused: bool,
    mask: bool,
    max: usize,
}

/// Which credential a [`TextField`] holds. Drives Tab order and submit gathering.
#[derive(Component, Clone, Copy, PartialEq, Default, Debug)]
enum LoginField {
    #[default]
    Username,
    Password,
}

/// The text line that surfaces login failures to the player.
#[derive(Component, Default, Clone)]
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

fn show_login_screen(mut commands: Commands) {
    commands
        .spawn_scene(login_screen())
        .insert(DespawnOnExit(GameState::Login));
}

fn login_screen() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [
            Node {
                width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
            }
            Children [
                @title_text("LIFTHRASIR", 48.0)
                TextShadow { offset: Vec2::splat(3.0), color: {Color::BLACK.with_alpha(0.85)} }
                --
                @chrome_text("The tree endures. So do you.".into(), 14.0, ro_dark::ACCENT)
                TextShadow { offset: Vec2::splat(1.0), color: {Color::BLACK.with_alpha(0.9)} }
                Node { margin: {UiRect::bottom(px(20))} }
                --
                @panel()
            ]
            --
            @settings_gear()
        ]
    }
}

fn panel() -> impl Scene {
    bsn! {
        @window_titled()
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: px(14),
        }
        Children [
            @window_title("Login")
            --
            @field(LoginField::Username, "Username", USERNAME_MAX, false, true)
            --
            @field(LoginField::Password, "Password", PASSWORD_MAX, true, false)
            --
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            Children [
                // NOTE: display only, nothing persists credentials yet; wire it when that lands.
                @checkbox("Remember me")
                --
                @chrome_text("Forgot password?".into(), 12.0, ro_dark::FROST)
            ]
            --
            LoginError
            @chrome_text(String::new(), 12.0, ro_dark::DANGER_GLOW)
            --
            @button(ButtonArt::PRIMARY, "Log In")
            on(submit_button)
            --
            @divider(bsn! { @chrome_text("or continue with".into(), 11.0, ro_dark::MUTED) })
            --
            Node { column_gap: px(12) }
            Children [
                @icon_button(ButtonArt::NORMAL, "google")
                --
                @icon_button(ButtonArt::NORMAL, "apple")
            ]
            --
            @divider(divider_diamond())
            --
            @chrome_text("New here?  Create account".into(), 12.0, ro_dark::MUTED)
            Node { align_self: AlignSelf::Center }
        ]
    }
}

/// A labelled credential field. Focus is shown with the frame's `Outline`, which
/// [`render_login_fields`] colours; that also runs on spawn since `Changed` fires on
/// insertion.
fn field(
    kind: LoginField,
    label: &'static str,
    max: usize,
    mask: bool,
    focused: bool,
) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::MUTED)
            --
            TextField { value: String::new(), placeholder: String::new(), focused: focused, mask: mask, max: max }
            kind
            Pickable
            @input_frame()
            on(focus_field)
            Children [ @chrome_text(String::new(), 13.0, ro_dark::BODY) ]
        ]
    }
}

fn settings_gear() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            right: px(16),
            width: px(36),
            height: px(36),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(ro_dark::WINDOW_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        Pickable
        on(open_settings)
        Children [ @glyph_icon("gear", 18.0, ro_dark::BRONZE_HIGHLIGHT) ]
    }
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

fn focus_field(click: On<PointerClick>, mut fields: Query<(Entity, &mut TextField)>) {
    for (entity, mut field) in &mut fields {
        field.focused = entity == click.entity;
    }
}

fn submit_button(
    _: On<Activate>,
    fields: Query<(&TextField, &LoginField)>,
    mut writer: MessageWriter<LoginAttemptEvent>,
) {
    let (username, password) = credentials(fields.iter());
    writer.write(login_attempt(&username, &password));
}

fn open_settings(
    _: On<PointerClick>,
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

/// Reflects each field's value (masked or not), placeholder, and focus artwork.
fn render_login_fields(
    mut fields: Query<(&TextField, &Children, &mut Outline), Changed<TextField>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (field, children, mut outline) in &mut fields {
        let (shown, color) = if field.value.is_empty() {
            (field.placeholder.clone(), ro_dark::MUTED)
        } else if field.mask {
            (
                "\u{2022}".repeat(field.value.chars().count()),
                ro_dark::BODY,
            )
        } else {
            (field.value.clone(), ro_dark::BODY)
        };
        outline.color = if field.focused {
            ro_dark::FROST_GLOW
        } else {
            Color::NONE
        };
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

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut().commands().spawn_scene(login_screen());
        app.update();
        app
    }

    #[test]
    fn login_scene_spawns_two_fields_with_username_focused() {
        let mut app = scene_app();

        let world = app.world_mut();
        let mut fields: Vec<(bool, LoginField)> = world
            .query::<(&TextField, &LoginField)>()
            .iter(world)
            .map(|(field, kind)| (field.focused, *kind))
            .collect();
        fields.sort_by_key(|(focused, _)| !*focused);
        assert_eq!(
            fields,
            vec![(true, LoginField::Username), (false, LoginField::Password)]
        );
    }
}
