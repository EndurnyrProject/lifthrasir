use super::{components::*, events::*, interactions::*, theme::*, widgets::*};
use crate::{core::state::GameState, infrastructure::assets::HierarchicalAssetManager};
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
                    handle_input_focus,
                    handle_text_input,
                    update_input_display,
                    handle_enhanced_input_interactions,
                )
                    .run_if(in_state(GameState::Login)),
            )
            .add_systems(OnExit(GameState::Login), cleanup_login_ui)
            .add_event::<LoginAttemptEvent>()
            .insert_resource(LoginFormData::default());
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
) {
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

                // Trigger login attempt
                if !login_data.username.trim().is_empty() {
                    login_events.write(LoginAttemptEvent {
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

    // Handle Enter key for login
    if keys.just_pressed(KeyCode::Enter) && !login_data.username.trim().is_empty() {
        login_events.write(LoginAttemptEvent {
            username: login_data.username.clone(),
            password: login_data.password.clone(),
        });
    }
}

fn process_login_attempts(
    mut login_events: EventReader<LoginAttemptEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in login_events.read() {
        info!("Login attempt: {}", event.username);

        // For now, automatically succeed and move to in-game
        // In a real implementation, this would validate credentials with the server
        if !event.username.is_empty() {
            next_state.set(GameState::InGame);
        }
    }
}
