use super::{components::*, login::LoginUiState};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::Ime;
use bevy_lunex::prelude::*;
use secrecy::{ExposeSecret, SecretString};

/// Component to mark Lunex input fields
#[derive(Component)]
pub struct LunexInput;

/// Component to mark username input field
#[derive(Component)]
pub struct LunexUsernameInput;

/// Component to mark password input field
#[derive(Component)]
pub struct LunexPasswordInput;

/// Component to mark the login button
#[derive(Component)]
pub struct LunexLoginButton;

/// Component to mark checkbox
#[derive(Component)]
pub struct LunexCheckbox {
    pub checked: bool,
}

/// Component to mark status text
#[derive(Component)]
pub struct LunexStatusText;

/// Component to track focused input
#[derive(Component)]
pub struct LunexFocusedInput;

// Use the hover_set function from bevy_lunex directly

/// System to handle text input for focused Lunex input fields
pub fn handle_text_input(
    mut keyboard_events: EventReader<KeyboardInput>,
    mut ime_events: EventReader<Ime>,
    mut login_data: ResMut<LoginFormData>,
    focused_query: Query<Entity, With<LunexFocusedInput>>,
    username_query: Query<Entity, With<LunexUsernameInput>>,
    password_query: Query<Entity, With<LunexPasswordInput>>,
) {
    // Check if any input is focused
    let Ok(focused_entity) = focused_query.get_single() else {
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
            (Key::Tab, _) => {
                // Tab navigation between fields will be handled by focus system
            }
            (_, Some(text)) => {
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

/// System to update the displayed text in Lunex input fields
pub fn update_input_display(
    login_data: Res<LoginFormData>,
    username_query: Query<&Children, With<LunexUsernameInput>>,
    password_query: Query<&Children, With<LunexPasswordInput>>,
    mut text_query: Query<&mut Text2d>,
) {
    // Update username display
    if let Ok(children) = username_query.get_single() {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                if text.0 != login_data.username {
                    text.0 = login_data.username.clone();
                }
                break;
            }
        }
    }

    // Update password display (show asterisks)
    if let Ok(children) = password_query.get_single() {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                let password_display = "*".repeat(login_data.password.expose_secret().len());
                if text.0 != password_display {
                    text.0 = password_display;
                }
                break;
            }
        }
    }
}

/// System to update status text
pub fn update_status_text(
    ui_state: Res<LoginUiState>,
    mut query: Query<&mut Text2d, With<LunexStatusText>>,
) {
    if let Ok(mut text) = query.get_single_mut() {
        if ui_state.is_connecting {
            text.0 = "Connecting to server...".to_string();
        } else if let Some(error) = &ui_state.error_message {
            text.0 = error.clone();
        } else {
            text.0 = "".to_string();
        }
    }
}

/// Observer for handling checkbox clicks
pub fn toggle_checkbox(
    trigger: Trigger<Pointer<Click>>,
    mut query: Query<(&mut LunexCheckbox, &Children)>,
    mut text_query: Query<&mut Text2d>,
    mut login_data: ResMut<LoginFormData>,
) {
    if let Ok((mut checkbox, children)) = query.get_mut(trigger.target()) {
        checkbox.checked = !checkbox.checked;
        let checked = checkbox.checked;
        login_data.remember_me = checked;

        // Copy children to avoid borrowing issues
        let children_vec: Vec<Entity> = children.to_vec();

        // Drop the mutable borrow by exiting the if let scope
        // Update checkmark display
        for child in children_vec.iter() {
            // Find the checkbox box child
            if let Ok((_, box_children)) = query.get(*child) {
                for checkmark_child in box_children.iter() {
                    if let Ok(mut text) = text_query.get_mut(checkmark_child) {
                        text.0 = if checked { "✓" } else { "" }.to_string();
                        break;
                    }
                }
            }
        }
    }
}

/// Observer for handling input field focus
pub fn focus_input(
    trigger: Trigger<Pointer<Click>>,
    mut commands: Commands,
    focused_query: Query<Entity, With<LunexFocusedInput>>,
) {
    // Remove focus from all inputs
    for entity in focused_query.iter() {
        commands.entity(entity).remove::<LunexFocusedInput>();
    }

    // Add focus to clicked input
    commands.entity(trigger.target()).insert(LunexFocusedInput);
}

/// System to handle Tab navigation between input fields
pub fn handle_tab_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    focused_query: Query<Entity, With<LunexFocusedInput>>,
    username_query: Query<Entity, With<LunexUsernameInput>>,
    password_query: Query<Entity, With<LunexPasswordInput>>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }

    // Get currently focused entity if any
    let current_focus = focused_query.get_single().ok();

    // Remove current focus
    if let Some(entity) = current_focus {
        commands.entity(entity).remove::<LunexFocusedInput>();
    }

    // Determine next focus
    match current_focus {
        Some(entity) if username_query.contains(entity) => {
            // Move focus to password field
            if let Ok(password_entity) = password_query.get_single() {
                commands.entity(password_entity).insert(LunexFocusedInput);
            }
        }
        Some(entity) if password_query.contains(entity) => {
            // Move focus back to username field
            if let Ok(username_entity) = username_query.get_single() {
                commands.entity(username_entity).insert(LunexFocusedInput);
            }
        }
        _ => {
            // No focus or unknown focus, start with username field
            if let Ok(username_entity) = username_query.get_single() {
                commands.entity(username_entity).insert(LunexFocusedInput);
            }
        }
    }
}

/// Plugin to register Lunex interaction systems
pub struct LunexInteractionsPlugin;

impl Plugin for LunexInteractionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_text_input,
                update_input_display,
                update_status_text,
                handle_tab_navigation,
            ),
        );
    }
}
