use super::{interactions::*, resources::LoginUiState};
use crate::presentation::ui::{events::*, popup::ShowPopupEvent, shared::*};
use crate::{
    domain::authentication::events::*,
    infrastructure::networking::errors::NetworkError,
};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_lunex::prelude::*;

pub fn setup_login_ui_once(
    commands: Commands,
    asset_server: Res<AssetServer>,
    images: ResMut<Assets<Image>>,
    mut ui_state: ResMut<LoginUiState>,
) {
    if ui_state.initialized {
        return;
    }
    ui_state.initialized = true;

    setup_login_ui(commands, asset_server, images);
}

fn setup_login_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    images: ResMut<Assets<Image>>,
) {
    // Spawn UI camera with proper source configuration
    commands.spawn((
        Camera2d,
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
        RenderLayers::from_layers(&[0, 1]),
        LoginScreen,
    ));

    // Load assets
    let background_image = asset_server.load("data/login_screen.png");

    // Create UI root
    commands
        .spawn((
            Name::new("Login UI Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
            LoginScreen,
        ))
        .with_children(|ui| {
            // Background image
            ui.spawn((
                Name::new("Background"),
                UiLayout::window().full().pack(),
                Sprite::from_image(background_image),
                Pickable::IGNORE,
            ));

            // Login form elements positioned directly on background
            // Username label
            ui.spawn((
                UiLayout::window()
                    .pos(Rl((36.0, 50.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(FONT_SIZE_LABEL)),
                Text2d::new("Username"),
                TextFont {
                    font_size: FONT_SIZE_LABEL,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));

            // Username input
            let username_entity = text_input(
                ui,
                "Username Input",
                Rl((36.0, 52.0)),
                380.0, // Approximate width for 50% of screen
                InputType::Username,
            );

            ui.commands().entity(username_entity).observe(focus_input);

            // Password label
            ui.spawn((
                UiLayout::window()
                    .pos(Rl((36.0, 57.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(FONT_SIZE_LABEL)),
                Text2d::new("Password"),
                TextFont {
                    font_size: FONT_SIZE_LABEL,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));

            // Password input
            let password_entity = text_input(
                ui,
                "Password Input",
                Rl((36.0, 59.0)),
                380.0, // Approximate width for 50% of screen
                InputType::Password,
            );
            ui.commands().entity(password_entity).observe(focus_input);

            // Remember me checkbox
            let checkbox_entity = checkbox(ui, "Remember Me", Rl((36.0, 64.0)));
            ui.commands()
                .entity(checkbox_entity)
                .observe(toggle_checkbox);

            // Login button using textured button
            let login_button_entity = textured_button(
                ui,
                &asset_server,
                "Login",
                "Login Button",
                Rl((43.0, 70.0)),
                Some((190.0, 60.0)),
                Some(ButtonType::Login),
            );
            ui.commands()
                .entity(login_button_entity)
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(on_login_click);

            // Status text
            status_text(ui, Rl((50.0, 92.0)));
        });
}

pub fn cleanup_login_ui(
    mut commands: Commands,
    query: Query<Entity, With<LoginScreen>>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    ui_state.initialized = false;
}

/// Observer for login button clicks
fn on_login_click(
    _trigger: Trigger<Pointer<Click>>,
    mut login_events: EventWriter<LoginAttemptEvent>,
    login_data: Res<LoginFormData>,
    ui_state: Res<LoginUiState>,
) {
    let username_valid = !login_data.username.trim().is_empty();
    let cooldown_ready = ui_state.login_cooldown.finished();
    let not_connecting = !ui_state.is_connecting;

    if username_valid && cooldown_ready && not_connecting {
        login_events.write(LoginAttemptEvent {
            username: login_data.username.clone(),
            password: login_data.password.clone(),
        });
    }
}

/// System to handle Enter key for login
pub fn handle_enter_key_login(
    keys: Res<ButtonInput<KeyCode>>,
    mut login_events: EventWriter<LoginAttemptEvent>,
    login_data: Res<LoginFormData>,
    ui_state: Res<LoginUiState>,
) {
    if keys.just_pressed(KeyCode::Enter)
        && !login_data.username.trim().is_empty()
        && ui_state.login_cooldown.finished()
        && !ui_state.is_connecting
    {
        login_events.write(LoginAttemptEvent {
            username: login_data.username.clone(),
            password: login_data.password.clone(),
        });
    }
}

pub fn process_login_attempts(
    mut login_events: EventReader<LoginAttemptEvent>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for event in login_events.read() {
        if !event.username.trim().is_empty() {
            // Update UI state - stay in Login state to keep UI visible
            ui_state.is_connecting = true;
            ui_state.error_message = None;
            ui_state.last_username = event.username.clone();

            // Don't transition states - let success/failure handlers manage transitions
        }
    }
}

pub fn handle_login_started(
    mut events: EventReader<LoginAttemptStartedEvent>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for _event in events.read() {
        ui_state.is_connecting = true;
        ui_state.error_message = None;
    }
}

pub fn handle_login_failure_ui(
    mut events: EventReader<LoginFailureEvent>,
    mut ui_state: ResMut<LoginUiState>,
    mut popup_events: EventWriter<ShowPopupEvent>,
    _time: Res<Time>,
) {
    for event in events.read() {
        ui_state.is_connecting = false;
        let error_message = format_login_error(&event.error);
        ui_state.error_message = Some(error_message.clone());

        // Set a 3-second cooldown after failed login attempts
        ui_state.login_cooldown = Timer::from_seconds(3.0, TimerMode::Once);

        // Show error popup
        popup_events.write(ShowPopupEvent::error(error_message));
    }
}

pub fn handle_login_success_ui(
    mut events: EventReader<LoginSuccessEvent>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for _event in events.read() {
        ui_state.is_connecting = false;
        ui_state.error_message = None;
    }
}

fn format_login_error(error: &NetworkError) -> String {
    match error {
        NetworkError::ConnectionFailed(_) => {
            "Cannot connect to server. Please check your connection.".to_string()
        }
        NetworkError::LoginRefused { code } => match code {
            0 => "Invalid username or password.".to_string(),
            1 => "Server is currently under maintenance.".to_string(),
            _ => format!("Login refused by server (code: {}).", code),
        },
        NetworkError::Timeout => "Connection timed out. Please try again.".to_string(),
        NetworkError::AuthenticationFailed { reason } => reason.clone(),
        _ => "An unexpected error occurred. Please try again.".to_string(),
    }
}
