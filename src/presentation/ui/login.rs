use super::{
    components::*, events::*, interactions::*, popup::ShowPopupEvent, theme::*,
};
use crate::{
    core::state::GameState,
    domain::authentication::events::*,
    infrastructure::networking::errors::NetworkError,
};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_lunex::prelude::*;

pub struct LoginPlugin;

impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((UiLunexPlugins, LunexInteractionsPlugin))
            .add_systems(OnEnter(GameState::Login), setup_login_ui)
            .add_systems(
                Update,
                (
                    handle_login_button_click,
                    handle_enter_key_login,
                    process_login_attempts,
                    handle_login_started,
                    handle_login_failure_ui,
                    handle_login_success_ui,
                )
                    .run_if(in_state(GameState::Login)),
            )
            .add_systems(OnExit(GameState::Login), cleanup_login_ui)
            .add_event::<LoginAttemptEvent>()
            .insert_resource(LoginFormData::default())
            .insert_resource(LoginUiState::default());
    }
}

fn setup_login_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
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

            // Login panel positioned at bottom center
            ui.spawn((
                Name::new("Login Panel"),
                UiLayout::window()
                    .pos(Rl((50.0, 80.0)))
                    .anchor(Anchor::Center)
                    .size(PANEL_SIZE_SMALL)
                    .pack(),
                UiColor::from(PANEL_BACKGROUND_LIGHT),
                Sprite::default(),
            ))
            .with_children(|ui| {
                // Panel content container
                ui.spawn((
                    Name::new("Panel Content"),
                    UiLayout::window()
                        .pos(Rl((50.0, 5.0)))
                        .anchor(Anchor::TopCenter)
                        .size(Rl((90.0, 90.0)))
                        .pack(),
                ))
                .with_children(|ui| {
                    // Username label
                    ui.spawn((
                        UiLayout::window()
                            .pos(Rl((10.0, 15.0)))
                            .anchor(Anchor::CenterLeft)
                            .pack(),
                        UiTextSize::from(Ab(FONT_SIZE_LABEL)),
                        Text2d::new("Username:"),
                        TextFont {
                            font_size: FONT_SIZE_LABEL,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));

                    // Username input
                    let username_input = ui.spawn((
                        Name::new("Username Input"),
                        UiLayout::window()
                            .pos(Rl((10.0, 25.0)))
                            .size((Rl(80.0), INPUT_HEIGHT))
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), INPUT_BACKGROUND_TRANSPARENT),
                            (UiHover::id(), Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE)),
                        ]),
                        UiHover::new().forward_speed(10.0).backward_speed(5.0),
                        Sprite::default(),
                        Pickable::default(),
                        LunexUsernameInput,
                        LunexInput,
                        LunexFocusedInput,
                    ))
                    .observe(focus_input)
                    .with_children(|ui| {
                        // Border effect on hover/focus
                        ui.spawn((
                            UiLayout::window().full().pack(),
                            UiColor::new(vec![
                                (UiBase::id(), Color::NONE),
                                (UiHover::id(), RUNIC_GLOW.with_alpha(0.3)),
                            ]),
                            UiHover::new().forward_speed(10.0).backward_speed(5.0),
                            Sprite::default(),
                            Pickable::IGNORE,
                        ));

                        // Text content for the input
                        ui.spawn((
                            UiLayout::window()
                                .pos((Rh(10.0), Rl(50.0)))
                                .anchor(Anchor::CenterLeft)
                                .pack(),
                            UiTextSize::from(Ab(FONT_SIZE_BODY)),
                            Text2d::new(""),
                            TextFont {
                                font_size: FONT_SIZE_BODY,
                                ..default()
                            },
                            TextColor(ASHEN_WHITE),
                            Pickable::IGNORE,
                        ));
                    })
                    .id();

                    // Password label
                    ui.spawn((
                        UiLayout::window()
                            .pos(Rl((10.0, 45.0)))
                            .anchor(Anchor::CenterLeft)
                            .pack(),
                        UiTextSize::from(Ab(FONT_SIZE_LABEL)),
                        Text2d::new("Password:"),
                        TextFont {
                            font_size: FONT_SIZE_LABEL,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));

                    // Password input
                    ui.spawn((
                        Name::new("Password Input"),
                        UiLayout::window()
                            .pos(Rl((10.0, 53.0)))
                            .size((Rl(80.0), INPUT_HEIGHT))
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), INPUT_BACKGROUND_TRANSPARENT),
                            (UiHover::id(), Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE)),
                        ]),
                        UiHover::new().forward_speed(10.0).backward_speed(5.0),
                        Sprite::default(),
                        Pickable::default(),
                        LunexPasswordInput,
                        LunexInput,
                    ))
                    .observe(focus_input)
                    .with_children(|ui| {
                        // Border effect on hover/focus
                        ui.spawn((
                            UiLayout::window().full().pack(),
                            UiColor::new(vec![
                                (UiBase::id(), Color::NONE),
                                (UiHover::id(), RUNIC_GLOW.with_alpha(0.3)),
                            ]),
                            UiHover::new().forward_speed(10.0).backward_speed(5.0),
                            Sprite::default(),
                            Pickable::IGNORE,
                        ));

                        // Text content for the input
                        ui.spawn((
                            UiLayout::window()
                                .pos((Rh(10.0), Rl(50.0)))
                                .anchor(Anchor::CenterLeft)
                                .pack(),
                            UiTextSize::from(Ab(FONT_SIZE_BODY)),
                            Text2d::new(""),
                            TextFont {
                                font_size: FONT_SIZE_BODY,
                                ..default()
                            },
                            TextColor(ASHEN_WHITE),
                            Pickable::IGNORE,
                        ));
                    });

                    // Remember me checkbox
                    ui.spawn((
                        Name::new("Checkbox Container"),
                        UiLayout::window()
                            .pos(Rl((10.0, 72.0)))
                            .size((200.0, CHECKBOX_SIZE))
                            .pack(),
                        LunexCheckbox { checked: false },
                    ))
                    .observe(toggle_checkbox)
                    .with_children(|ui| {
                        // Checkbox box
                        ui.spawn((
                            Name::new("Checkbox Box"),
                            UiLayout::window()
                                .pos((0.0, Rl(50.0)))
                                .anchor(Anchor::CenterLeft)
                                .size((CHECKBOX_SIZE, CHECKBOX_SIZE))
                                .pack(),
                            UiColor::new(vec![
                                (UiBase::id(), INPUT_BACKGROUND_TRANSPARENT),
                                (UiHover::id(), Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE)),
                            ]),
                            UiHover::new().forward_speed(10.0).backward_speed(5.0),
                            Sprite::default(),
                            OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                        ))
                        .with_children(|ui| {
                            // Checkmark (initially hidden)
                            ui.spawn((
                                Name::new("Checkmark"),
                                UiLayout::window()
                                    .pos(Rl(50.0))
                                    .anchor(Anchor::Center)
                                    .pack(),
                                UiTextSize::from(Ab(FONT_SIZE_BODY)),
                                Text2d::new(""),
                                TextFont {
                                    font_size: FONT_SIZE_BODY,
                                    ..default()
                                },
                                TextColor(RUNIC_GLOW),
                                Pickable::IGNORE,
                            ));
                        });

                        // Checkbox label
                        ui.spawn((
                            UiLayout::window()
                                .pos((CHECKBOX_SIZE + SPACING_SM, Rl(50.0)))
                                .anchor(Anchor::CenterLeft)
                                .pack(),
                            UiTextSize::from(Ab(FONT_SIZE_BODY)),
                            Text2d::new("Remember Me"),
                            TextFont {
                                font_size: FONT_SIZE_BODY,
                                ..default()
                            },
                            TextColor(TEXT_PRIMARY),
                            Pickable::IGNORE,
                        ));
                    });

                    // Login button
                    ui.spawn((
                        Name::new("Login Button"),
                        UiLayout::window()
                            .pos(Rl((50.0, 82.0)))
                            .size((120.0, BUTTON_HEIGHT))
                            .pack(),
                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                        LunexLoginButton,
                    ))
                    .observe(hover_set::<Pointer<Over>, true>)
                    .observe(hover_set::<Pointer<Out>, false>)
                    .observe(on_login_click)
                    .with_children(|ui| {
                        // Button background with states
                        ui.spawn((
                            UiLayout::new(vec![
                                (UiBase::id(), UiLayout::window().full()),
                                (UiHover::id(), UiLayout::window().full()),
                            ]),
                            UiHover::new().forward_speed(15.0).backward_speed(6.0),
                            UiColor::new(vec![
                                (UiBase::id(), BUTTON_NORMAL_TRANSPARENT),
                                (UiHover::id(), BUTTON_HOVER_TRANSPARENT),
                            ]),
                            Sprite::default(),
                            Pickable::IGNORE,
                        ))
                        .with_children(|ui| {
                            // Button text
                            ui.spawn((
                                UiLayout::window()
                                    .pos(Rl(50.0))
                                    .anchor(Anchor::Center)
                                    .pack(),
                                UiColor::new(vec![
                                    (UiBase::id(), TEXT_PRIMARY),
                                    (UiHover::id(), FORGE_SOOT),
                                ]),
                                UiHover::new().forward_speed(15.0).backward_speed(6.0),
                                UiTextSize::from(Ab(FONT_SIZE_BUTTON)),
                                Text2d::new("Login"),
                                TextFont {
                                    font_size: FONT_SIZE_BUTTON,
                                    ..default()
                                },
                                Pickable::IGNORE,
                            ));
                        });
                    });

                    // Status text
                    ui.spawn((
                        Name::new("Status Text"),
                        UiLayout::window()
                            .pos(Rl((50.0, 90.0)))
                            .anchor(Anchor::Center)
                            .pack(),
                        UiTextSize::from(Ab(FONT_SIZE_BODY)),
                        Text2d::new(""),
                        TextFont {
                            font_size: FONT_SIZE_BODY,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.4, 0.4)),
                        LunexStatusText,
                    ));
                });
            });
        });
}

fn cleanup_login_ui(mut commands: Commands, query: Query<Entity, With<LoginScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

/// Observer for login button clicks
fn on_login_click(
    _trigger: Trigger<Pointer<Click>>,
    mut login_events: EventWriter<LoginAttemptEvent>,
    login_data: Res<LoginFormData>,
    ui_state: Res<LoginUiState>,
) {
    if !login_data.username.trim().is_empty()
        && ui_state.login_cooldown.finished()
        && !ui_state.is_connecting
    {
        login_events.send(LoginAttemptEvent {
            username: login_data.username.clone(),
            password: login_data.password.clone(),
        });
    }
}

/// System to handle login button clicks (alternative to observer)
fn handle_login_button_click(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<LunexLoginButton>)>,
    mut login_events: EventWriter<LoginAttemptEvent>,
    login_data: Res<LoginFormData>,
    ui_state: Res<LoginUiState>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            if !login_data.username.trim().is_empty()
                && ui_state.login_cooldown.finished()
                && !ui_state.is_connecting
            {
                login_events.send(LoginAttemptEvent {
                    username: login_data.username.clone(),
                    password: login_data.password.clone(),
                });
            }
        }
    }
}

/// System to handle Enter key for login
fn handle_enter_key_login(
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
        login_events.send(LoginAttemptEvent {
            username: login_data.username.clone(),
            password: login_data.password.clone(),
        });
    }
}

fn process_login_attempts(
    mut login_events: EventReader<LoginAttemptEvent>,
    mut ui_state: ResMut<LoginUiState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in login_events.read() {
        if !event.username.trim().is_empty() {
            info!("Login attempt initiated for: {}", event.username);

            // Update UI state
            ui_state.is_connecting = true;
            ui_state.error_message = None;
            ui_state.last_username = event.username.clone();

            // Transition to connecting state
            next_state.set(GameState::Connecting);
        }
    }
}

fn handle_login_started(
    mut events: EventReader<LoginAttemptStartedEvent>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for event in events.read() {
        ui_state.is_connecting = true;
        ui_state.error_message = None;
        info!("UI: Login started for {}", event.username);
    }
}

fn handle_login_failure_ui(
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
        popup_events.send(ShowPopupEvent::error(error_message));

        warn!("UI: Login failed for {}", event.username);
    }
}

fn handle_login_success_ui(
    mut events: EventReader<LoginSuccessEvent>,
    mut ui_state: ResMut<LoginUiState>,
) {
    for event in events.read() {
        ui_state.is_connecting = false;
        ui_state.error_message = None;
        info!("UI: Login successful for {}", event.session.username);
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

// LoginUiState resource for managing UI state
#[derive(Resource)]
pub struct LoginUiState {
    pub is_connecting: bool,
    pub error_message: Option<String>,
    pub last_username: String,
    pub login_cooldown: Timer,
}

impl Default for LoginUiState {
    fn default() -> Self {
        Self {
            is_connecting: false,
            error_message: None,
            last_username: String::new(),
            login_cooldown: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}