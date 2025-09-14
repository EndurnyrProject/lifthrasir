use super::{
    components::*, events::*, interactions::*, popup::ShowPopupEvent, theme::*, widgets::*,
};
use crate::{
    core::state::GameState,
    domain::authentication::{AuthenticationContext, events::*},
    infrastructure::assets::HierarchicalAssetManager,
    infrastructure::networking::errors::NetworkError,
};
use bevy::prelude::*;

pub struct LoginPlugin;

impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Login), setup_login_ui)
            .add_systems(
                Update,
                (
                    handle_login_interactions,
                    process_login_attempts,
                    handle_login_started,
                    handle_login_failure_ui,
                    handle_login_success_ui,
                    update_status_display,
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
    // Setup UI camera for login screen
    commands.spawn((Camera2d, LoginScreen));

    // Load background image
    let background_image = asset_server.load("data/login_screen.png");

    // Create main container with background image
    let mut root_entity = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::FlexEnd, // Align to bottom
            padding: UiRect::all(Val::Px(SPACING_XXL)), // Add bottom padding
            ..default()
        },
        LoginScreen,
    ));

    // Add background image
    root_entity.insert(ImageNode::new(background_image));

    root_entity.with_children(|parent| {
        // Small login panel with medium transparency for better visibility
        parent
            .spawn(ro_panel_preset(PANEL_SIZE_SMALL, PANEL_BACKGROUND_LIGHT))
            .with_children(|parent| {
                // Simple login form

                // Username section
                parent.spawn(ro_label("Username:"));
                parent
                    .spawn((ro_text_input(), UsernameInput))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new(""),
                            TextFont::from_font_size(FONT_SIZE_BODY),
                            TextColor(ASHEN_WHITE),
                            Node {
                                align_self: AlignSelf::Center,
                                margin: UiRect::left(Val::Px(2.0)),
                                ..default()
                            },
                        ));
                    });

                // Password section
                parent.spawn(ro_label("Password:"));
                parent
                    .spawn((ro_text_input(), PasswordInput))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new(""),
                            TextFont::from_font_size(FONT_SIZE_BODY),
                            TextColor(ASHEN_WHITE),
                            Node {
                                align_self: AlignSelf::Center,
                                margin: UiRect::left(Val::Px(2.0)),
                                ..default()
                            },
                        ));
                    });

                // Remember me checkbox using reusable components
                parent
                    .spawn((ro_checkbox_container(), RememberMeCheckbox))
                    .with_children(|parent| {
                        parent.spawn(ro_checkbox_box());
                        parent.spawn((
                            Text::new("Remember Me"),
                            TextFont::from_font_size(FONT_SIZE_BODY),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });

                // Login button using reusable component
                parent
                    .spawn((ro_button_with_width("Login", 120.0), LoginButton))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new("Login"),
                            TextFont::from_font_size(FONT_SIZE_BUTTON),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });

                // Status text area for connection status and errors
                parent
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(40.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(SPACING_MD)),
                            ..default()
                        },
                        StatusTextArea,
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new(""),
                            TextFont::from_font_size(FONT_SIZE_BODY),
                            TextColor(Color::srgb(1.0, 0.4, 0.4)), // Error color
                            StatusText,
                        ));
                    });
            });
    });
}

fn cleanup_login_ui(mut commands: Commands, query: Query<Entity, With<LoginScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn handle_login_interactions(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&mut BorderRadius>,
        ),
        (Changed<Interaction>, With<LoginButton>),
    >,
    mut login_events: EventWriter<LoginAttemptEvent>,
    login_data: Res<LoginFormData>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<LoginUiState>,
    time: Res<Time>,
) {
    // Tick the cooldown timer
    ui_state.login_cooldown.tick(time.delta());
    // Handle button interactions with enhanced transparency and radius effects
    for (interaction, mut bg_color, mut border_color, mut border_radius) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BUTTON_PRESSED_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
                // Slightly smaller radius when pressed for tactile feedback
                if let Some(ref mut radius) = border_radius {
                    **radius = BorderRadius::all(Val::Px(RADIUS_SM));
                }

                // Trigger login attempt with cooldown check
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
            Interaction::Hovered => {
                *bg_color = BUTTON_HOVER_TRANSPARENT.into();
                border_color.0 = RUNIC_GLOW;
                // Slightly larger radius on hover for visual feedback
                if let Some(ref mut radius) = border_radius {
                    **radius = BorderRadius::all(Val::Px(RADIUS_MD + 2.0));
                }
            }
            Interaction::None => {
                *bg_color = BUTTON_NORMAL_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
                // Return to default radius
                if let Some(ref mut radius) = border_radius {
                    **radius = BorderRadius::all(Val::Px(RADIUS_MD));
                }
            }
        }
    }

    // Handle Enter key for login with cooldown check
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

            // The domain layer will handle the actual login attempt
        }
    }
}

// Add new resource for UI state
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

// Add new systems for handling authentication events
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
) {
    for event in events.read() {
        ui_state.is_connecting = false;
        let error_message = format_login_error(&event.error);
        ui_state.error_message = Some(error_message.clone());

        // Set a 3-second cooldown after failed login attempts to prevent brute force
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

// Update status text based on UI state
fn update_status_display(
    mut status_text: Query<&mut Text, With<StatusText>>,
    ui_state: Res<LoginUiState>,
) {
    if let Ok(mut text) = status_text.get_single_mut() {
        if ui_state.is_connecting {
            text.0 = "Connecting to server...".to_string();
        } else if let Some(error) = &ui_state.error_message {
            text.0 = error.clone();
        } else {
            text.0 = "".to_string();
        }
    }
}
