use super::{components::*, theme::*};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::Ime;
use secrecy::{ExposeSecret, SecretString};

/// Enhanced interaction system for buttons with dynamic transparency and border radius effects
pub fn handle_enhanced_button_interactions(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&mut BorderRadius>,
        ),
        (Changed<Interaction>, With<RoButton>),
    >,
) {
    for (interaction, mut bg_color, mut border_color, mut border_radius) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BUTTON_PRESSED_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
                // Slightly smaller radius when pressed for tactile feedback
                if let Some(ref mut radius) = border_radius {
                    **radius = BorderRadius::all(Val::Px(RADIUS_SM));
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
}

/// Enhanced interaction system for input fields with focus effects
pub fn handle_enhanced_input_interactions(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<RoInput>),
    >,
) {
    for (interaction, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                // Input field is selected/focused
                *bg_color = INPUT_BACKGROUND_TRANSPARENT.into();
                border_color.0 = INPUT_BORDER_FOCUS;
            }
            Interaction::Hovered => {
                // Input field is being hovered
                *bg_color = Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE).into();
                border_color.0 = RUNIC_GLOW;
            }
            Interaction::None => {
                // Input field is in normal state
                *bg_color = INPUT_BACKGROUND_TRANSPARENT.into();
                border_color.0 = INPUT_BORDER;
            }
        }
    }
}

/// Enhanced interaction system for panels with subtle hover effects
pub fn handle_enhanced_panel_interactions(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<RoPanel>),
    >,
) {
    for (interaction, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                // Panel is being interacted with (if it's interactive)
                *bg_color = Color::srgba(0.176, 0.188, 0.220, TRANSPARENCY_SUBTLE).into();
                border_color.0 = RUNIC_GLOW;
            }
            Interaction::Hovered => {
                // Panel is being hovered (subtle effect)
                *bg_color = Color::srgba(0.176, 0.188, 0.220, TRANSPARENCY_MODERATE + 0.05).into();
                border_color.0 = POLISHED_STEEL;
            }
            Interaction::None => {
                // Panel is in normal state
                *bg_color = BACKGROUND_SECONDARY_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
            }
        }
    }
}

/// Focus management for text input fields
pub fn handle_input_focus(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focused_query: Query<Entity, With<FocusedInput>>,
    input_query: Query<
        (&Interaction, Entity),
        (
            Changed<Interaction>,
            Or<(With<UsernameInput>, With<PasswordInput>)>,
        ),
    >,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        // Remove focus from all inputs first
        for entity in focused_query.iter() {
            commands.entity(entity).remove::<FocusedInput>();
        }

        // Add focus to clicked input
        for (interaction, entity) in input_query.iter() {
            if *interaction == Interaction::Pressed {
                commands.entity(entity).insert(FocusedInput);
            }
        }
    }
}

/// Text input handling for focused input fields with IME support
pub fn handle_text_input(
    mut keyboard_events: EventReader<KeyboardInput>,
    mut ime_events: EventReader<Ime>,
    mut login_data: ResMut<LoginFormData>,
    focused_query: Query<Entity, With<FocusedInput>>,
    username_query: Query<Entity, With<UsernameInput>>,
    password_query: Query<Entity, With<PasswordInput>>,
) {
    // Check if any input is focused
    if focused_query.is_empty() {
        return;
    }

    let Ok(focused_entity) = focused_query.single() else {
        return;
    };

    // Determine which field is focused
    let is_username = username_query.contains(focused_entity);
    let is_password = password_query.contains(focused_entity);

    if !is_username && !is_password {
        return;
    }

    // Handle IME events first (better for international text input)
    for event in ime_events.read() {
        match event {
            Ime::Commit { value, .. } => {
                // Committed text from IME with length validation
                if is_username {
                    if login_data.username.len() + value.len() <= MAX_USERNAME_LENGTH {
                        login_data.username.push_str(value);
                    }
                } else if is_password {
                    let current_password = login_data.password.expose_secret();
                    if current_password.len() + value.len() <= MAX_PASSWORD_LENGTH {
                        let mut new_password = current_password.to_string();
                        new_password.push_str(value);
                        login_data.password = SecretString::from(new_password);
                    }
                }
            }
            _ => {}
        }
    }

    // Handle keyboard events
    for event in keyboard_events.read() {
        // Only trigger changes when the key is first pressed
        if !event.state.is_pressed() {
            continue;
        }

        match (&event.logical_key, &event.text) {
            (Key::Backspace, _) => {
                if is_username {
                    login_data.username.pop();
                } else if is_password {
                    let current_password = login_data.password.expose_secret();
                    if !current_password.is_empty() {
                        let mut new_password = current_password.to_string();
                        new_password.pop();
                        login_data.password = SecretString::from(new_password);
                    }
                }
            }
            (_, Some(text)) => {
                // Use event.text for better character handling (includes IME fallback)
                if text.chars().all(|c| !c.is_control()) {
                    if is_username {
                        if login_data.username.len() + text.len() <= MAX_USERNAME_LENGTH {
                            login_data.username.push_str(text);
                        }
                    } else if is_password {
                        let current_password = login_data.password.expose_secret();
                        if current_password.len() + text.len() <= MAX_PASSWORD_LENGTH {
                            let mut new_password = current_password.to_string();
                            new_password.push_str(text);
                            login_data.password = SecretString::from(new_password);
                        }
                    }
                }
            }
            _ => continue,
        }
    }
}

/// Update displayed text in input fields
pub fn update_input_display(
    login_data: Res<LoginFormData>,
    username_query: Query<Entity, With<UsernameInput>>,
    password_query: Query<Entity, With<PasswordInput>>,
    children_query: Query<&Children>,
    mut text_query: Query<&mut Text>,
) {
    // Always update to ensure text is visible - this is more reliable than checking is_changed
    // Update username display
    if let Ok(entity) = username_query.single() {
        if let Ok(children) = children_query.get(entity) {
            for child in children {
                if let Ok(mut text) = text_query.get_mut(*child) {
                    // Use correct Bevy text API - Text.0 is the string content
                    let current_text = &text.0;
                    let expected_text = &login_data.username;
                    // Only update if different to avoid unnecessary work
                    if current_text != expected_text {
                        text.0 = login_data.username.clone();
                    }
                    break;
                }
            }
        }
    }

    // Update password display (show asterisks)
    if let Ok(entity) = password_query.single() {
        if let Ok(children) = children_query.get(entity) {
            for child in children {
                if let Ok(mut text) = text_query.get_mut(*child) {
                    // Use correct Bevy text API - Text.0 is the string content
                    let expected_text = "*".repeat(login_data.password.expose_secret().len());
                    let current_text = &text.0;
                    // Only update if different to avoid unnecessary work
                    if current_text != &expected_text {
                        text.0 = expected_text;
                    }
                    break;
                }
            }
        }
    }
}

/// Plugin to register all enhanced interaction systems
pub struct EnhancedInteractionsPlugin;

impl Plugin for EnhancedInteractionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_enhanced_button_interactions,
                handle_enhanced_input_interactions,
                handle_enhanced_panel_interactions,
                handle_input_focus,
                handle_text_input,
                update_input_display,
            ),
        );
    }
}
