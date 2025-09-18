use super::resources::*;
use crate::core::state::CharacterScreenState;
use crate::domain::assets::patterns::hair_palette_path;
use crate::domain::character::catalog::HeadStyleCatalog;
use crate::domain::character::*;
use crate::domain::entities::sprite_factory::RoSpriteFactory;
use crate::presentation::ui::shared::theme::*;
use crate::presentation::ui::shared::widgets::{
    ScrollablePanel, scrollable_panel, textured_button,
};
use bevy::prelude::*;
use bevy_lunex::prelude::*;

pub fn setup_character_creation_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    current_state: Res<State<CharacterScreenState>>,
) {
    info!(
        "Setting up character creation UI - Current state: {:?}",
        current_state.get()
    );

    // Create creation UI root
    commands
        .spawn((
            Name::new("Character Creation UI Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
            CharacterCreationUiRoot,
        ))
        .with_children(|ui| {
            // Background overlay
            ui.spawn((
                Name::new("Background"),
                UiLayout::window().full().pack(),
                UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.95)),
                Sprite::default(),
                Pickable::IGNORE,
            ));

            // Title
            ui.spawn((
                Name::new("Title"),
                UiLayout::window()
                    .pos(Rl((50.0, 8.0)))
                    .anchor(Anchor::TopCenter)
                    .pack(),
                UiTextSize::from(Ab(36.0)),
                Text2d::new("Create New Character"),
                TextFont {
                    font_size: 36.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));

            // Main content panel
            ui.spawn((
                Name::new("Main Panel"),
                UiLayout::window()
                    .pos(Rl((50.0, 50.0)))
                    .anchor(Anchor::Center)
                    .size((Rl(85.0), Rl(80.0)))
                    .pack(),
                UiColor::from(BACKGROUND_SECONDARY.with_alpha(0.8)),
                Sprite::default(),
                Pickable::IGNORE,
            ))
            .with_children(|main_panel| {
                // Left section - Form area (60% width)
                main_panel
                    .spawn((
                        Name::new("Form Section"),
                        UiLayout::window()
                            .pos(Rl((30.0, 50.0)))
                            .anchor(Anchor::Center)
                            .size((Rl(60.0), Rl(90.0)))
                            .pack(),
                        CharacterCreationFormSection,
                        Pickable::IGNORE,
                    ))
                    .with_children(|form_section| {
                        // Character name input area
                        create_name_input_area(form_section, &asset_server);

                        // Gender selection area
                        create_gender_selection_area(form_section, &asset_server);

                        // Hair customization areas (Phase 3)
                        // Note: These will be updated with available options via systems
                        create_hair_style_selection_area(form_section, &asset_server);
                        create_hair_color_selection_area(form_section, &asset_server);

                        // Action buttons area
                        create_action_buttons_area(form_section, &asset_server);
                    });

                // Right section - Preview area (40% width)
                create_preview_section(main_panel, &asset_server);
            });

            // Bottom navigation
            create_bottom_navigation(ui, &asset_server);
        });
}

fn create_name_input_area(form_section: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    form_section
        .spawn((
            Name::new("Name Input Area"),
            UiLayout::window()
                .pos(Rl((50.0, 15.0)))
                .anchor(Anchor::Center)
                .size((Rl(90.0), Rl(15.0)))
                .pack(),
            UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.3)),
            Sprite::default(),
            Pickable::IGNORE,
        ))
        .with_children(|name_area| {
            // Name input label
            name_area.spawn((
                Name::new("Name Label"),
                UiLayout::window()
                    .pos(Rl((10.0, 30.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(18.0)),
                Text2d::new("Character Name:"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
                Pickable::IGNORE,
            ));

            // Placeholder for name input field
            name_area
                .spawn((
                    Name::new("Name Input Field"),
                    UiLayout::window()
                        .pos(Rl((50.0, 70.0)))
                        .anchor(Anchor::Center)
                        .size((Rl(80.0), Rl(40.0)))
                        .pack(),
                    UiColor::from(INPUT_BACKGROUND),
                    Sprite::default(),
                    Pickable::default(),
                    CharacterNameInput::default(),
                ))
                .with_children(|input_field| {
                    // Input text display
                    input_field.spawn((
                        Name::new("Input Text"),
                        UiLayout::window()
                            .pos(Rl((10.0, 50.0)))
                            .anchor(Anchor::CenterLeft)
                            .pack(),
                        UiTextSize::from(Ab(16.0)),
                        Text2d::new(""),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn create_gender_selection_area(
    form_section: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    form_section
        .spawn((
            Name::new("Gender Selection Area"),
            UiLayout::window()
                .pos(Rl((50.0, 35.0)))
                .anchor(Anchor::Center)
                .size((Rl(90.0), Rl(15.0)))
                .pack(),
            UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.3)),
            Sprite::default(),
            GenderSelectionContainer,
            Pickable::IGNORE,
        ))
        .with_children(|gender_area| {
            // Gender selection label
            gender_area.spawn((
                Name::new("Gender Label"),
                UiLayout::window()
                    .pos(Rl((10.0, 30.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(18.0)),
                Text2d::new("Gender:"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
                Pickable::IGNORE,
            ));

            // Male button
            let male_button = textured_button(
                gender_area,
                asset_server,
                "Male",
                "MaleGenderButton",
                Rl((20.0, 15.0)),
                Some((100.0, 35.0)),
                None,
            );

            gender_area
                .commands()
                .entity(male_button)
                .insert(GenderToggleButton::male())
                .observe(on_gender_button_click);

            // Female button
            let female_button = textured_button(
                gender_area,
                asset_server,
                "Female",
                "FemaleGenderButton",
                Rl((35.0, 15.0)),
                Some((100.0, 35.0)),
                None,
            );

            gender_area
                .commands()
                .entity(female_button)
                .insert(GenderToggleButton::female())
                .observe(on_gender_button_click);
        });
}

fn create_hair_style_selection_area(
    form_section: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    form_section
        .spawn((
            Name::new("Hair Style Selection Area"),
            UiLayout::window()
                .pos(Rl((50.0, 50.0)))
                .anchor(Anchor::Center)
                .size((Rl(90.0), Rl(15.0)))
                .pack(),
            UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.3)),
            Sprite::default(),
            HairStyleSelectionContainer,
            Pickable::IGNORE,
        ))
        .with_children(|style_area| {
            // Hair style label
            style_area.spawn((
                Name::new("Hair Style Label"),
                UiLayout::window()
                    .pos(Rl((10.0, 30.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(18.0)),
                Text2d::new("Hair Style:"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
                Pickable::IGNORE,
            ));

            // Hair style buttons container (will be populated by update system)
            style_area.spawn((
                Name::new("Hair Style Buttons Container"),
                UiLayout::window()
                    .pos(Rl((50.0, 70.0)))
                    .anchor(Anchor::Center)
                    .size((Rl(90.0), Rl(50.0)))
                    .pack(),
                HairStyleGrid,
                Pickable::IGNORE,
            ));
        });
}

fn create_hair_color_selection_area(
    form_section: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    form_section
        .spawn((
            Name::new("Hair Color Selection Area"),
            UiLayout::window()
                .pos(Rl((50.0, 70.0)))
                .anchor(Anchor::Center)
                .size((Rl(90.0), Rl(15.0)))
                .pack(),
            UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.3)),
            Sprite::default(),
            HairColorSelectionContainer,
            Pickable::IGNORE,
        ))
        .with_children(|color_area| {
            // Hair color label
            color_area.spawn((
                Name::new("Hair Color Label"),
                UiLayout::window()
                    .pos(Rl((10.0, 30.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Ab(18.0)),
                Text2d::new("Hair Color:"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
                Pickable::IGNORE,
            ));

            // Hair color buttons container (will be populated by update system)
            color_area.spawn((
                Name::new("Hair Color Buttons Container"),
                UiLayout::window()
                    .pos(Rl((50.0, 70.0)))
                    .anchor(Anchor::Center)
                    .size((Rl(90.0), Rl(50.0)))
                    .pack(),
                HairColorGrid,
                Pickable::IGNORE,
            ));
        });
}

fn create_action_buttons_area(form_section: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    form_section
        .spawn((
            Name::new("Action Buttons Area"),
            UiLayout::window()
                .pos(Rl((50.0, 85.0)))
                .anchor(Anchor::Center)
                .size((Rl(90.0), Rl(12.0)))
                .pack(),
            Pickable::IGNORE,
        ))
        .with_children(|buttons_area| {
            let create_button = textured_button(
                buttons_area,
                asset_server,
                "Create",
                "CreateCharacterButton",
                Rl((70.0, 50.0)),
                Some((120.0, 40.0)),
                None,
            );

            buttons_area
                .commands()
                .entity(create_button)
                .insert(CreateCharacterSubmitButton)
                .observe(on_character_creation_submit_click);
        });
}

fn create_preview_section(main_panel: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    main_panel
        .spawn((
            Name::new("Preview Section"),
            UiLayout::window()
                .pos(Rl((75.0, 50.0)))
                .anchor(Anchor::Center)
                .size((Rl(35.0), Rl(90.0)))
                .pack(),
            UiColor::from(BACKGROUND_PRIMARY.with_alpha(0.3)),
            Sprite::default(),
            CharacterCreationPreviewSection,
            Pickable::IGNORE,
        ))
        .with_children(|preview_section| {
            // Preview title
            preview_section.spawn((
                Name::new("Preview Title"),
                UiLayout::window()
                    .pos(Rl((50.0, 8.0)))
                    .anchor(Anchor::Center)
                    .pack(),
                UiTextSize::from(Ab(18.0)),
                Text2d::new("Hair Styles"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
                Pickable::IGNORE,
            ));

            // Create scrollable panel for hair previews
            let (_panel_entity, content_entity) = scrollable_panel(
                preview_section,
                "Hair Preview Scrollable",
                Rl((15.0, 10.0)),
                500.0,
                350.0,
            );

            // Add marker to the content entity so we can find it
            preview_section.commands().entity(content_entity).insert((
                CharacterPreviewContainer,
                UiColor::from(BACKGROUND_SECONDARY.with_alpha(0.5)),
                Sprite::default(),
            ));
        });
}

fn create_bottom_navigation(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    ui.spawn((
        Name::new("Navigation Section"),
        UiLayout::window()
            .pos(Rl((50.0, 92.0)))
            .anchor(Anchor::Center)
            .size((Rl(85.0), Rl(8.0)))
            .pack(),
        Pickable::IGNORE,
    ))
    .with_children(|nav_section| {
        // Back button
        let back_button = textured_button(
            nav_section,
            &asset_server,
            "Back",
            "BackButton",
            Rl((20.0, 50.0)),
            Some((120.0, 40.0)),
            None,
        );

        nav_section
            .commands()
            .entity(back_button)
            .insert(CharacterCreationBackButton)
            .observe(on_character_creation_back_click);

        // Validation error display area
        nav_section.spawn((
            Name::new("Error Display"),
            UiLayout::window()
                .pos(Rl((50.0, 50.0)))
                .anchor(Anchor::Center)
                .pack(),
            UiTextSize::from(Ab(14.0)),
            Text2d::new(""),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(ERROR_COLOR),
            ValidationErrorDisplay,
            Pickable::IGNORE,
            Visibility::Hidden,
        ));
    });
}

pub fn cleanup_character_creation_ui(
    mut commands: Commands,
    query: Query<Entity, With<CharacterCreationUiRoot>>,
) {
    info!(
        "Cleaning up character creation UI - Entities to clean: {}",
        query.iter().count()
    );

    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Spawns all hair style previews in the preview container
pub fn spawn_all_hair_previews(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    catalog: Option<Res<HeadStyleCatalog>>,
    preview_container_query: Query<Entity, With<CharacterPreviewContainer>>,
    mut scroll_panel_query: Query<&mut ScrollablePanel>,
    mut spawned: Local<bool>,
) {
    // Only spawn once
    if *spawned {
        return;
    }

    // Wait for catalog to be loaded
    let Some(catalog) = catalog else {
        return;
    };

    // Wait for preview container to exist
    let Ok(container_entity) = preview_container_query.get_single() else {
        return;
    };

    // Wait for styles to be initialized
    if creation_resource.available_hair_styles.is_empty() {
        return;
    }

    info!(
        "Spawning {} hair style previews",
        creation_resource.available_hair_styles.len()
    );

    let gender = creation_resource.form.sex;
    let styles_per_row = 4;
    let sprite_size = 50.0; // 20% of original (100 -> 50)
    let spacing = 60.0; // Tighter spacing for smaller sprites

    let total_styles = creation_resource.available_hair_styles.len();
    let total_rows = (total_styles + styles_per_row - 1) / styles_per_row;

    // Spawn all available hair styles
    for (index, &style_id) in creation_resource.available_hair_styles.iter().enumerate() {
        if let Some(entry) = catalog.get(gender, style_id) {
            let row = index / styles_per_row;
            let col = index % styles_per_row;

            let x = col as f32 * spacing + 10.0; // Start offset + spacing
            let y = row as f32 * spacing + 10.0;

            // Spawn sprite
            let sprite_entity = RoSpriteFactory::spawn_from_paths(
                &mut commands,
                &asset_server,
                entry.sprite_path.clone(),
                entry.act_path.clone(),
                None, // No palette for preview
                Vec3::new(0.0, 0.0, 1.0),
                0, // Idle action
            );

            // Add UiLayout for proper positioning with smaller size
            commands.entity(sprite_entity).insert(
                UiLayout::window()
                    .pos(Ab((x, y)))
                    .anchor(Anchor::TopLeft)
                    .size((Ab(sprite_size), Ab(sprite_size)))
                    .pack(),
            );

            // Parent to container
            commands.entity(container_entity).add_child(sprite_entity);
        }
    }

    // Update ScrollablePanel content height
    // The hierarchy is: ScrollablePanel -> Clip Container -> ScrollContent (CharacterPreviewContainer)
    // So we need to go up two levels to find the ScrollablePanel
    let content_height = total_rows as f32 * spacing + 20.0; // Total height needed

    // Find the ScrollablePanel by traversing up the parent hierarchy
    for mut panel in scroll_panel_query.iter_mut() {
        panel.content_height = content_height;
        info!(
            "Set scroll panel content height to {} for {} rows",
            content_height, total_rows
        );
    }

    *spawned = true;
    info!("Finished spawning all hair previews");
}

// Observer for character creation submit button clicks
pub fn on_character_creation_submit_click(
    trigger: Trigger<Pointer<Click>>,
    submit_buttons: Query<&CreateCharacterSubmitButton>,
    creation_resource: Res<CharacterCreationResource>,
    mut create_events: EventWriter<CreateCharacterRequestEvent>,
    current_state: Res<State<CharacterScreenState>>,
) {
    let entity = trigger.target();

    // Only process clicks when in the correct state
    if *current_state.get() != CharacterScreenState::CharacterCreation {
        return;
    }

    if let Ok(_) = submit_buttons.get(entity) {
        // Validate and submit creation form
        match creation_resource.form.validate() {
            Ok(()) => {
                create_events.write(CreateCharacterRequestEvent {
                    form: creation_resource.form.clone(),
                });
                info!(
                    "Submitting character creation for '{}'",
                    creation_resource.form.name
                );
            }
            Err(e) => {
                error!("Character creation validation failed: {:?}", e);
                // TODO: Show error message to user
            }
        }
    }
}

pub fn on_character_creation_back_click(
    trigger: Trigger<Pointer<Click>>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    mut close_events: EventWriter<CloseCharacterCreationEvent>,
    mut char_state: ResMut<NextState<CharacterScreenState>>,
    current_state: Res<State<CharacterScreenState>>,
) {
    info!(
        "on_character_creation_back_click observer triggered for entity {:?} - Current state: {:?}",
        trigger.target(),
        current_state.get()
    );
    creation_resource.reset();
    close_events.write(CloseCharacterCreationEvent);
    info!("Setting state to CharacterList from back button observer");
    char_state.set(CharacterScreenState::CharacterList);
}

// Observer for gender button clicks
pub fn on_gender_button_click(
    trigger: Trigger<Pointer<Click>>,
    mut all_gender_buttons: Query<&mut GenderToggleButton>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    current_state: Res<State<CharacterScreenState>>,
) {
    let entity = trigger.target();

    // Only process clicks when in the correct state
    if *current_state.get() != CharacterScreenState::CharacterCreation {
        return;
    }

    // Get the clicked button's gender first (before borrowing mutably)
    let selected_gender = if let Ok(clicked_button) = all_gender_buttons.get(entity) {
        clicked_button.gender
    } else {
        return;
    };

    info!("Gender button clicked: {:?}", selected_gender);

    // Update the creation resource
    creation_resource.form.sex = selected_gender;

    // Update button selection states
    for mut button in all_gender_buttons.iter_mut() {
        button.is_selected = button.gender == selected_gender;
    }
}

// ==== CHARACTER NAME INPUT SYSTEMS ====

/// Automatically focus the character name input when character creation starts
pub fn auto_focus_character_name_input(
    mut input_query: Query<&mut CharacterNameInput>,
    creation_resource: Res<CharacterCreationResource>,
    mut has_focused: Local<bool>,
) {
    // Auto-focus when character creation becomes active
    if creation_resource.is_active && !*has_focused {
        if let Ok(mut input) = input_query.get_single_mut() {
            input.is_focused = true;
            *has_focused = true;
        }
    }

    // Reset focus flag when character creation is not active
    if !creation_resource.is_active {
        *has_focused = false;
    }
}

/// Handles keyboard input for character name entry
pub fn handle_character_name_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut CharacterNameInput>,
    mut creation_resource: ResMut<CharacterCreationResource>,
) {
    // Only process input if we have an active input field
    let mut input_component = match query.get_single_mut() {
        Ok(component) => component,
        Err(_) => return,
    };

    // Only process input when focused
    if !input_component.is_focused {
        return;
    }

    let mut text_changed = false;
    let mut current_text = input_component.current_text.clone();
    let mut cursor_pos = input_component.cursor_position;

    // Handle backspace
    if keyboard_input.just_pressed(KeyCode::Backspace) || keyboard_input.pressed(KeyCode::Backspace)
    {
        if cursor_pos > 0 && !current_text.is_empty() {
            current_text.remove(cursor_pos - 1);
            cursor_pos -= 1;
            text_changed = true;
        }
    }

    // Handle delete
    if keyboard_input.just_pressed(KeyCode::Delete) {
        if cursor_pos < current_text.len() {
            current_text.remove(cursor_pos);
            text_changed = true;
        }
    }

    // Handle cursor movement
    if keyboard_input.just_pressed(KeyCode::ArrowLeft) {
        cursor_pos = cursor_pos.saturating_sub(1);
    }
    if keyboard_input.just_pressed(KeyCode::ArrowRight) {
        cursor_pos = (cursor_pos + 1).min(current_text.len());
    }
    if keyboard_input.just_pressed(KeyCode::Home) {
        cursor_pos = 0;
    }
    if keyboard_input.just_pressed(KeyCode::End) {
        cursor_pos = current_text.len();
    }

    // Handle character input (alphanumeric + underscore only)
    for key in keyboard_input.get_just_pressed() {
        if current_text.len() >= input_component.max_length {
            break;
        }

        let character = match key {
            // Letters
            KeyCode::KeyA => Some('a'),
            KeyCode::KeyB => Some('b'),
            KeyCode::KeyC => Some('c'),
            KeyCode::KeyD => Some('d'),
            KeyCode::KeyE => Some('e'),
            KeyCode::KeyF => Some('f'),
            KeyCode::KeyG => Some('g'),
            KeyCode::KeyH => Some('h'),
            KeyCode::KeyI => Some('i'),
            KeyCode::KeyJ => Some('j'),
            KeyCode::KeyK => Some('k'),
            KeyCode::KeyL => Some('l'),
            KeyCode::KeyM => Some('m'),
            KeyCode::KeyN => Some('n'),
            KeyCode::KeyO => Some('o'),
            KeyCode::KeyP => Some('p'),
            KeyCode::KeyQ => Some('q'),
            KeyCode::KeyR => Some('r'),
            KeyCode::KeyS => Some('s'),
            KeyCode::KeyT => Some('t'),
            KeyCode::KeyU => Some('u'),
            KeyCode::KeyV => Some('v'),
            KeyCode::KeyW => Some('w'),
            KeyCode::KeyX => Some('x'),
            KeyCode::KeyY => Some('y'),
            KeyCode::KeyZ => Some('z'),
            // Numbers
            KeyCode::Digit0 => Some('0'),
            KeyCode::Digit1 => Some('1'),
            KeyCode::Digit2 => Some('2'),
            KeyCode::Digit3 => Some('3'),
            KeyCode::Digit4 => Some('4'),
            KeyCode::Digit5 => Some('5'),
            KeyCode::Digit6 => Some('6'),
            KeyCode::Digit7 => Some('7'),
            KeyCode::Digit8 => Some('8'),
            KeyCode::Digit9 => Some('9'),
            // Underscore (typically Shift + Minus, but let's handle both common cases)
            KeyCode::Minus
                if keyboard_input.pressed(KeyCode::ShiftLeft)
                    || keyboard_input.pressed(KeyCode::ShiftRight) =>
            {
                Some('_')
            }
            _ => None,
        };

        if let Some(mut ch) = character {
            // Apply shift modifier for uppercase letters
            if (keyboard_input.pressed(KeyCode::ShiftLeft)
                || keyboard_input.pressed(KeyCode::ShiftRight))
                && ch.is_ascii_lowercase()
            {
                ch = ch.to_ascii_uppercase();
            }

            current_text.insert(cursor_pos, ch);
            cursor_pos += 1;
            text_changed = true;
        }
    }

    // Update the component if anything changed
    if text_changed || cursor_pos != input_component.cursor_position {
        input_component.current_text = current_text.clone();
        input_component.cursor_position = cursor_pos;

        // Sync with creation resource
        creation_resource.form.name = current_text.clone();
    }
}

/// Handles focus state for the input field based on mouse interactions
pub fn handle_input_field_focus(
    mouse_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut input_query: Query<&mut CharacterNameInput>,
    input_text_query: Query<Entity, (With<Text2d>, With<Name>)>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    // Convert screen position to world position
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    // Check if click is within input field bounds
    let mut clicked_input = false;

    // For now, we'll use a simple bounds check. In a more sophisticated implementation,
    // we'd check the actual UI element bounds
    for input_text_entity in input_text_query.iter() {
        // Simple bounds check - this should be replaced with proper UI bounds checking
        // For now, we'll just focus the input when clicking anywhere on the UI
        clicked_input = true;
        break;
    }

    // Update focus state
    for mut input in input_query.iter_mut() {
        input.is_focused = clicked_input;
    }
}

/// Validates character name input and updates validation errors
pub fn validate_character_name_input(
    input_query: Query<&CharacterNameInput, Changed<CharacterNameInput>>,
    mut creation_resource: ResMut<CharacterCreationResource>,
) {
    // Only validate when the input component has changed
    let Ok(input) = input_query.get_single() else {
        return;
    };

    // Clear previous validation errors related to the name
    creation_resource
        .validation_errors
        .retain(|error| !error.contains("name") && !error.contains("Name"));

    // Validate the form using the existing validation logic
    if let Err(error) = creation_resource.form.validate() {
        let error_message = match error {
            crate::domain::character::models::CharacterCreationError::NameEmpty => {
                "Character name cannot be empty".to_string()
            }
            crate::domain::character::models::CharacterCreationError::NameTooShort => {
                "Character name must be at least 4 characters".to_string()
            }
            crate::domain::character::models::CharacterCreationError::NameTooLong => {
                "Character name cannot exceed 23 characters".to_string()
            }
            crate::domain::character::models::CharacterCreationError::NameInvalidCharacters => {
                "Character name can only contain letters, numbers, and underscores".to_string()
            }
            crate::domain::character::models::CharacterCreationError::NameForbidden => {
                "Character name contains forbidden words".to_string()
            }
            _ => return, // Don't add non-name related errors here
        };

        creation_resource.validation_errors.push(error_message);
    }
}

/// Updates the displayed text in the character name input field
pub fn update_character_name_display(
    creation_resource: Res<CharacterCreationResource>,
    input_query: Query<(&CharacterNameInput, &Children)>,
    mut text_query: Query<&mut Text2d>,
) {
    // Find the input field and update its child Text2d entity
    if let Ok((input, children)) = input_query.get_single() {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                // Update text based on form data, not component data
                text.0 = creation_resource.form.name.clone();
                break;
            }
        }
    }
}

/// Updates visual feedback for the input field (cursor blinking, placeholder text)
pub fn update_input_visual_feedback(
    input_query: Query<(&CharacterNameInput, &Children)>,
    mut text_query: Query<&mut Text2d>,
    creation_resource: Res<CharacterCreationResource>,
    time: Res<Time>,
    mut last_blink_time: Local<f32>,
) {
    let Ok((input, children)) = input_query.get_single() else {
        return;
    };

    // Handle cursor blinking (0.5 second intervals)
    let current_time = time.elapsed_secs();
    let should_show_cursor = if input.is_focused {
        (current_time - *last_blink_time) % 1.0 < 0.5
    } else {
        false
    };

    // Update the blink timer
    if current_time - *last_blink_time >= 1.0 {
        *last_blink_time = current_time;
    }

    // Find and update the child Text2d entity
    for child in children.iter() {
        if let Ok(mut text) = text_query.get_mut(child) {
            // Create display text with or without cursor
            let display_text = if input.is_focused && should_show_cursor {
                let mut display = creation_resource.form.name.clone();
                if input.cursor_position <= display.len() {
                    display.insert(input.cursor_position, '|');
                }
                display
            } else if input.is_focused {
                // Focused but cursor hidden during blink
                creation_resource.form.name.clone()
            } else {
                // Not focused, show placeholder if empty
                if creation_resource.form.name.is_empty() {
                    "Enter character name...".to_string()
                } else {
                    creation_resource.form.name.clone()
                }
            };

            text.0 = display_text;
            break;
        }
    }
}

/// Updates input field border colors based on focus and validation state
pub fn update_input_border_feedback(
    mut input_query: Query<(&CharacterNameInput, &mut UiColor)>,
    creation_resource: Res<CharacterCreationResource>,
) {
    let Ok((input, mut ui_color)) = input_query.get_single_mut() else {
        return;
    };

    // Determine the appropriate border color
    let border_color = if !creation_resource.validation_errors.is_empty() {
        // Has validation errors - show error color
        ERROR_COLOR
    } else if input.is_focused {
        // Focused and valid - show focus color
        INPUT_BORDER_FOCUS
    } else {
        // Default state
        INPUT_BORDER
    };

    // Update the input field's background color to show focus/error state
    *ui_color = UiColor::from(border_color);
}

/// Updates visual highlighting for gender selection buttons
pub fn update_gender_button_highlight(
    mut gender_buttons: Query<(&GenderToggleButton, &Children), Changed<GenderToggleButton>>,
    mut text_query: Query<&mut TextColor>,
) {
    for (button, children) in gender_buttons.iter_mut() {
        // Update text color based on selection state
        let text_color = if button.is_selected {
            RUNIC_GLOW // Highlight selected with accent color
        } else {
            TEXT_PRIMARY // Normal text color for unselected
        };

        // Find and update the text child entity
        for child in children.iter() {
            if let Ok(mut text_color_component) = text_query.get_mut(child) {
                text_color_component.0 = text_color;
                break; // Only update the first text child
            }
        }
    }
}

/// Initializes the default gender selection when character creation starts
pub fn initialize_gender_selection(
    mut gender_buttons: Query<&mut GenderToggleButton>,
    creation_resource: Res<CharacterCreationResource>,
    mut initialized: Local<bool>,
) {
    // Only initialize once when character creation becomes active
    if !creation_resource.is_active {
        *initialized = false;
        return;
    }

    if *initialized {
        return;
    }

    // Set the default selection based on the form's default gender (Male)
    for mut button in gender_buttons.iter_mut() {
        button.is_selected = button.gender == creation_resource.form.sex;
    }

    *initialized = true;
}

/// Initializes available hair styles and colors when character creation starts or gender changes
pub fn initialize_hair_options(
    mut creation_resource: ResMut<CharacterCreationResource>,
    catalog: Option<Res<HeadStyleCatalog>>,
    current_state: Res<State<CharacterScreenState>>,
    mut last_gender: Local<Option<Gender>>,
) {
    // Only process when in character creation state
    if *current_state.get() != CharacterScreenState::CharacterCreation {
        return;
    }

    // Wait for catalog to be loaded
    let Some(catalog) = catalog else {
        debug!("HeadStyleCatalog not loaded yet, waiting...");
        return;
    };

    // Only run when creation becomes active or gender changes
    let current_gender = creation_resource.form.sex;
    let should_initialize = creation_resource.is_active
        && (last_gender.is_none() || *last_gender != Some(current_gender));

    if !should_initialize {
        return;
    }

    info!("Initializing hair options for gender: {:?}", current_gender);

    // Get all styles for current gender
    let all_styles = catalog.get_all(current_gender);
    creation_resource.available_hair_styles = all_styles.iter().map(|entry| entry.id).collect();

    info!(
        "Found {} hair styles for {:?}",
        creation_resource.available_hair_styles.len(),
        current_gender
    );

    // Validate current hair_style selection, use first available if invalid
    if !creation_resource
        .available_hair_styles
        .contains(&creation_resource.form.hair_style)
    {
        if let Some(&first_style) = creation_resource.available_hair_styles.first() {
            info!(
                "Current hair style {} not available, defaulting to {}",
                creation_resource.form.hair_style, first_style
            );
            creation_resource.form.hair_style = first_style;
        }
    }

    // Query available colors for selected style
    if let Some(colors) = catalog.get_colors(current_gender, creation_resource.form.hair_style) {
        creation_resource.available_hair_colors = colors.clone();
        info!(
            "Found {} hair colors for style {}",
            creation_resource.available_hair_colors.len(),
            creation_resource.form.hair_style
        );

        // Validate current color selection
        if !creation_resource
            .available_hair_colors
            .contains(&creation_resource.form.hair_color)
        {
            if let Some(&first_color) = creation_resource.available_hair_colors.first() {
                info!(
                    "Current hair color {} not available, defaulting to {}",
                    creation_resource.form.hair_color, first_color
                );
                creation_resource.form.hair_color = first_color;
            }
        }
    } else {
        warn!(
            "No colors found for style {} gender {:?}",
            creation_resource.form.hair_style, current_gender
        );
        creation_resource.available_hair_colors.clear();
    }

    *last_gender = Some(current_gender);
}

/// Populates hair style buttons when available styles change
pub fn populate_hair_style_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    creation_resource: Res<CharacterCreationResource>,
    style_grid_query: Query<(Entity, &Children), With<HairStyleGrid>>,
    existing_buttons: Query<Entity, With<HairStyleButton>>,
) {
    // Only run if we have styles available
    if creation_resource.available_hair_styles.is_empty() {
        return;
    }

    // Find the grid container
    let Ok((grid_entity, children)) = style_grid_query.get_single() else {
        return;
    };

    // Check if buttons already populated (count matches)
    let current_button_count = existing_buttons.iter().count();
    if current_button_count == creation_resource.available_hair_styles.len() {
        return; // Already populated
    }

    // Clear existing buttons
    for child in children.iter() {
        if existing_buttons.contains(child) {
            commands.entity(child).despawn_recursive();
        }
    }

    // Spawn new buttons
    commands.entity(grid_entity).with_children(|ui| {
        let button_width = 80.0;
        let button_height = 35.0;
        let spacing = 10.0;

        for (index, &style_id) in creation_resource.available_hair_styles.iter().enumerate() {
            let y_pos = (index as f32) * (button_height + spacing);
            let is_selected = style_id == creation_resource.form.hair_style;

            let button = textured_button(
                ui,
                &asset_server,
                format!("Style {}", style_id),
                format!("HairStyleButton_{}", style_id),
                (10.0, y_pos), // Absolute positioning
                Some((button_width, button_height)),
                None,
            );

            ui.commands()
                .entity(button)
                .insert(HairStyleButton {
                    style_id,
                    is_selected,
                })
                .observe(on_hair_style_click);
        }
    });

    info!(
        "Populated {} hair style buttons",
        creation_resource.available_hair_styles.len()
    );
}

/// Observer for hair style button clicks
pub fn on_hair_style_click(
    trigger: Trigger<Pointer<Click>>,
    mut all_style_buttons: Query<&mut HairStyleButton>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    catalog: Res<HeadStyleCatalog>,
    current_state: Res<State<CharacterScreenState>>,
) {
    let entity = trigger.target();

    // Guard: only in CharacterCreation state
    if *current_state.get() != CharacterScreenState::CharacterCreation {
        return;
    }

    // Get clicked button's style_id
    let selected_style_id = if let Ok(clicked_button) = all_style_buttons.get(entity) {
        clicked_button.style_id
    } else {
        return;
    };

    info!("Hair style button clicked: {}", selected_style_id);

    // Update the creation resource
    creation_resource.form.hair_style = selected_style_id;

    // Update button selection states
    for mut button in all_style_buttons.iter_mut() {
        button.is_selected = button.style_id == selected_style_id;
    }

    // Query catalog for available colors of selected style
    let current_gender = creation_resource.form.sex;
    if let Some(colors) = catalog.get_colors(current_gender, selected_style_id) {
        creation_resource.available_hair_colors = colors.clone();
        info!(
            "Loaded {} colors for style {}",
            colors.len(),
            selected_style_id
        );

        // Reset to first available color
        if let Some(&first_color) = colors.first() {
            creation_resource.form.hair_color = first_color;
            info!("Reset hair color to {}", first_color);
        }
    } else {
        warn!(
            "No colors found for style {} gender {:?}",
            selected_style_id, current_gender
        );
        creation_resource.available_hair_colors.clear();
    }
}

/// Updates visual highlighting for hair style selection buttons
pub fn update_hair_style_button_highlight(
    mut style_buttons: Query<(&HairStyleButton, &Children), Changed<HairStyleButton>>,
    mut text_query: Query<&mut TextColor>,
) {
    for (button, children) in style_buttons.iter_mut() {
        // Determine text color based on selection state
        let text_color = if button.is_selected {
            RUNIC_GLOW // Highlight selected with accent color
        } else {
            TEXT_PRIMARY // Normal text color for unselected
        };

        // Find and update the text child entity
        for child in children.iter() {
            if let Ok(mut text_color_component) = text_query.get_mut(child) {
                text_color_component.0 = text_color;
                break; // Only update the first text child
            }
        }
    }
}

/// Populates hair color buttons when available colors change
pub fn populate_hair_color_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    creation_resource: Res<CharacterCreationResource>,
    color_grid_query: Query<(Entity, &Children), With<HairColorGrid>>,
    existing_buttons: Query<Entity, With<HairColorButton>>,
) {
    if creation_resource.available_hair_colors.is_empty() {
        return;
    }

    let Ok((grid_entity, children)) = color_grid_query.get_single() else {
        return;
    };

    // Check if already populated
    let current_button_count = existing_buttons.iter().count();
    if current_button_count == creation_resource.available_hair_colors.len() {
        return;
    }

    // Clear existing buttons
    for child in children.iter() {
        if existing_buttons.contains(child) {
            commands.entity(child).despawn_recursive();
        }
    }

    // Spawn new buttons in grid layout (8 per row)
    commands.entity(grid_entity).with_children(|ui| {
        let button_size = 40.0;
        let spacing = 5.0;
        let buttons_per_row = 8;

        for (index, &color_id) in creation_resource.available_hair_colors.iter().enumerate() {
            let row = index / buttons_per_row;
            let col = index % buttons_per_row;

            let x_pos = 10.0 + (col as f32) * (button_size + spacing);
            let y_pos = (row as f32) * (button_size + spacing);
            let is_selected = color_id == creation_resource.form.hair_color;

            let button = textured_button(
                ui,
                &asset_server,
                format!("{}", color_id),
                format!("HairColorButton_{}", color_id),
                (x_pos, y_pos),
                Some((button_size, button_size)),
                None,
            );

            ui.commands()
                .entity(button)
                .insert(HairColorButton {
                    color_id,
                    is_selected,
                })
                .observe(on_hair_color_click);
        }
    });

    info!(
        "Populated {} hair color buttons",
        creation_resource.available_hair_colors.len()
    );
}

/// Observer for hair color button clicks
pub fn on_hair_color_click(
    trigger: Trigger<Pointer<Click>>,
    mut all_color_buttons: Query<&mut HairColorButton>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    current_state: Res<State<CharacterScreenState>>,
) {
    let entity = trigger.target();

    if *current_state.get() != CharacterScreenState::CharacterCreation {
        return;
    }

    let selected_color_id = if let Ok(clicked_button) = all_color_buttons.get(entity) {
        clicked_button.color_id
    } else {
        return;
    };

    info!("Hair color button clicked: {}", selected_color_id);

    creation_resource.form.hair_color = selected_color_id;

    for mut button in all_color_buttons.iter_mut() {
        button.is_selected = button.color_id == selected_color_id;
    }
}

/// Updates visual highlighting for hair color selection buttons
pub fn update_hair_color_button_highlight(
    mut color_buttons: Query<(&HairColorButton, &Children), Changed<HairColorButton>>,
    mut text_query: Query<&mut TextColor>,
) {
    for (button, children) in color_buttons.iter_mut() {
        let text_color = if button.is_selected {
            RUNIC_GLOW
        } else {
            TEXT_PRIMARY
        };

        for child in children.iter() {
            if let Ok(mut text_color_component) = text_query.get_mut(child) {
                text_color_component.0 = text_color;
                break;
            }
        }
    }
}
