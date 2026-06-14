use std::collections::HashMap;

use bevy::prelude::*;
use bevy_extended_ui::html::{HtmlSource, HtmlSubmit};
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use bevy_extended_ui::styles::CssID;
use bevy_extended_ui::widgets::Paragraph;
use bevy_extended_ui_macros::html_fn;
use game_engine::core::state::GameState;
use game_engine::domain::authentication::events::LoginFailureEvent;
use game_engine::infrastructure::networking::errors::NetworkError;
use game_engine::presentation::ui::events::LoginAttemptEvent;
use secrecy::SecretString;

const LOGIN_UI: &str = "login";
/// `AssetServer` path, relative to `assets/` (NOT to `ExtendedUiConfiguration.assets_path`).
/// extended_ui resolves the `<link>` CSS hrefs inside the HTML relative to this file's own
/// location, so the stylesheets are referenced by bare name (`theme.css`) in `login.html`.
const LOGIN_HTML: &str = "ui/login.html";
/// `id` of the `<p>` element that surfaces login failures to the player.
const ERROR_ELEMENT_ID: &str = "login-error";

pub struct LoginScreenPlugin;

impl Plugin for LoginScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Login), show_login_screen);
        app.add_systems(OnExit(GameState::Login), hide_login_screen);
        app.add_systems(
            Update,
            surface_login_failure.run_if(in_state(GameState::Login)),
        );
    }
}

#[allow(deprecated)]
fn show_login_screen(mut registry: ResMut<UiRegistry>, asset_server: Res<AssetServer>) {
    let handle: Handle<HtmlAsset> = asset_server.load(LOGIN_HTML);
    registry.add_and_use(LOGIN_UI.into(), HtmlSource::from_handle(handle));
}

#[allow(deprecated)]
fn hide_login_screen(mut registry: ResMut<UiRegistry>) {
    registry.remove(LOGIN_UI);
}

/// Builds a `LoginAttemptEvent` from the form's submit `data` map. Pure seam so the
/// credential wiring can be unit-tested without a running extended_ui tree.
fn login_attempt_from_data(data: &HashMap<String, String>) -> LoginAttemptEvent {
    let username = data.get("username").cloned().unwrap_or_default();
    let password = data.get("password").cloned().unwrap_or_default();
    LoginAttemptEvent {
        username,
        password: SecretString::from(password),
    }
}

/// User-readable message for a login failure. Pure seam for unit testing.
fn login_error_text(error: &NetworkError) -> String {
    error.to_string()
}

#[html_fn("submit_login")]
fn submit_login(In(event): In<HtmlSubmit>, mut writer: MessageWriter<LoginAttemptEvent>) {
    writer.write(login_attempt_from_data(&event.data));
}

/// Surfaces the most recent login failure into the `#login-error` paragraph.
/// Success needs no handling here: the engine transitions to `ServerSelection`.
fn surface_login_failure(
    mut failures: MessageReader<LoginFailureEvent>,
    mut errors: Query<(&mut Paragraph, &CssID)>,
) {
    let Some(failure) = failures.read().last() else {
        return;
    };
    let text = login_error_text(&failure.error);
    for (mut paragraph, id) in &mut errors {
        if id.0 == ERROR_ELEMENT_ID {
            paragraph.text = text.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn login_attempt_carries_username_and_password() {
        let mut data = HashMap::new();
        data.insert("username".to_string(), "adventurer".to_string());
        data.insert("password".to_string(), "swordfish".to_string());

        let attempt = login_attempt_from_data(&data);

        assert_eq!(attempt.username, "adventurer");
        assert_eq!(attempt.password.expose_secret(), "swordfish");
    }

    #[test]
    fn login_attempt_defaults_missing_fields_to_empty() {
        let attempt = login_attempt_from_data(&HashMap::new());

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
