use super::resources::*;
use crate::core::state::CharacterScreenState;
use crate::domain::character::rendering::CharacterSpriteContainer;
use crate::domain::character::*;
use crate::infrastructure::networking::{CharServerClient, UserSession};
use crate::presentation::ui::screens::character_selection::creation::CharacterCreationResource;
use crate::presentation::ui::screens::character_selection::shared::CharacterScreenCamera;
use crate::presentation::ui::shared::theme::*;
use crate::presentation::ui::shared::widgets::textured_button;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_lunex::prelude::*;

pub fn setup_character_selection_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let assets = CharacterSelectionAssets::load(&asset_server);
    commands.insert_resource(assets);
}

#[derive(Component)]
pub struct EmptySlotText;

#[derive(Component)]
pub struct CharacterNameText;

#[derive(Component)]
pub struct CharacterLevelText;

#[derive(Component)]
pub struct CharacterSlotFrame {
    pub slot_index: u8,
}

pub fn setup_character_selection_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<CharacterScreenState>>,
) {
    info!("Setting up character selection screen");

    let _no_char_frame: Handle<Image> = asset_server.load(TEXTURE_NO_CHARACTER_FRAME);
    let _with_char_frame: Handle<Image> = asset_server.load(TEXTURE_WITH_CHARACTER_FRAME);

    // Spawn UI camera - persists across character screen states
    commands.spawn((
        Camera2d,
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
        RenderLayers::from_layers(&[0, 1]),
        CharacterScreenCamera,
    ));

    // Start in connecting state - the character list UI will be created when entering CharacterList state
    next_state.set(CharacterScreenState::Connecting);
}

fn create_character_grid(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    // Create 3x3 grid of character slots
    for row in 0..3 {
        for col in 0..3 {
            let slot_index = (row * 3 + col) as u8;
            create_character_slot(ui, asset_server, slot_index, row, col);
        }
    }
}

fn create_character_slot(
    ui: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    slot_index: u8,
    row: usize,
    col: usize,
) {
    // Calculate position
    let x_pos = 25.0 + (col as f32 * 25.0);
    let y_pos = 30.0 + (row as f32 * 20.0);

    let mut slot_entity = ui.spawn((
        Name::new(format!("CharacterSlot_{}", slot_index)),
        UiLayout::window()
            .pos(Rl((x_pos, y_pos)))
            .size((Rl(20.0), Rl(15.0)))
            .anchor(Anchor::Center)
            .pack(),
        CharacterSlot { index: slot_index },
        CharacterCard {
            slot: slot_index,
            character: None,
        },
        Pickable::default(),
    ));

    slot_entity.with_children(|slot| {
        // Character frame background (will be initialized with proper texture in setup system)
        slot.spawn((
            Name::new(format!("CharacterFrame_{}", slot_index)),
            UiLayout::window().full().pack(),
            UiColor::from(BACKGROUND_SECONDARY), // Temporary fallback until texture is loaded
            Sprite::default(),
            Pickable::IGNORE,
            CharacterSlotFrame { slot_index },
        ));

        // Empty slot text
        slot.spawn((
            UiLayout::window()
                .pos(Rl((50.0, 50.0)))
                .anchor(Anchor::Center)
                .pack(),
            UiTextSize::from(Ab(16.0)),
            Text2d::new("Empty Slot"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(TEXT_SECONDARY),
            Pickable::IGNORE,
            EmptySlotText,
        ));

        // Sprite container (above the name)
        slot.spawn((
            Name::new(format!("SpriteContainer_{}", slot_index)),
            UiLayout::window()
                .pos(Rl((50.0, 60.0)))
                .anchor(Anchor::Center)
                .size((Ab(64.0), Ab(64.0)))
                .pack(),
            Pickable::IGNORE,
            CharacterSpriteContainer::new(slot_index),
        ));

        // Character name text (initially hidden)
        slot.spawn((
            UiLayout::window()
                .pos(Rl((50.0, 75.0)))
                .anchor(Anchor::Center)
                .pack(),
            UiTextSize::from(Ab(18.0)),
            Text2d::new(""),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(TEXT_PRIMARY),
            Pickable::IGNORE,
            CharacterNameText,
            Visibility::Hidden,
        ));

        // Character level text (initially hidden)
        slot.spawn((
            UiLayout::window()
                .pos(Rl((50.0, 85.0)))
                .anchor(Anchor::Center)
                .pack(),
            UiTextSize::from(Ab(14.0)),
            Text2d::new(""),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(TEXT_SECONDARY),
            Pickable::IGNORE,
            CharacterLevelText,
            Visibility::Hidden,
        ));

        // Create button for empty slots
        let create_button = textured_button(
            slot,
            asset_server,
            "Create",
            format!("CreateButton_{}", slot_index),
            Rl((35.0, 75.0)),
            Some((80.0, 35.0)),
            None,
        );

        slot.commands()
            .entity(create_button)
            .insert(CreateCharacterButton { slot: slot_index })
            .observe(on_create_character_click);
    });
}

fn create_bottom_buttons(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    // Select button
    create_select_button(ui, asset_server);

    // Delete button
    create_delete_button(ui, asset_server);

    // Back button
    create_back_button(ui, asset_server);
}

fn create_select_button(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let select_button = textured_button(
        ui,
        asset_server,
        "Select",
        "SelectButton",
        Rl((30.0, 85.0)),
        Some((150.0, 50.0)),
        None,
    );

    ui.commands().entity(select_button).insert(EnterGameButton);
}

fn create_delete_button(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let delete_button = textured_button(
        ui,
        asset_server,
        "Delete",
        "DeleteButton",
        Rl((50.0, 85.0)),
        Some((150.0, 50.0)),
        None,
    );

    ui.commands()
        .entity(delete_button)
        .insert(DeleteCharacterButton { character_id: 0 });
}

fn create_back_button(ui: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let back_button = textured_button(
        ui,
        asset_server,
        "Back",
        "BackButton",
        Rl((70.0, 85.0)),
        Some((150.0, 50.0)),
        None,
    );

    ui.commands()
        .entity(back_button)
        .insert(BackToServerSelectionButton);
}

pub fn initialize_character_frame_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<Entity, (With<CharacterSlotFrame>, With<UiColor>)>,
) {
    let no_char_frame: Handle<Image> = asset_server.load(TEXTURE_NO_CHARACTER_FRAME);

    for entity in query.iter() {
        commands
            .entity(entity)
            .remove::<UiColor>()
            .insert(Sprite::from_image(no_char_frame.clone()));
    }
}

pub fn cleanup_character_selection_screen(
    mut commands: Commands,
    query: Query<Entity, With<CharacterSelectionScreen>>,
    camera_query: Query<Entity, With<CharacterScreenCamera>>,
) {
    info!("Cleaning up character selection screen - Found {} entities", query.iter().count());
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    // Also cleanup the camera when leaving CharacterSelection game state
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn connect_to_character_server(
    mut commands: Commands,
    session: Res<UserSession>,
    mut next_state: ResMut<NextState<CharacterScreenState>>,
    current_char_state: Res<State<CharacterScreenState>>,
    existing_client: Option<Res<CharServerClient>>,
) {
    info!("connect_to_character_server called - Current state: {:?}, Client exists: {}",
        current_char_state.get(), existing_client.is_some());

    // If we already have a client, don't reconnect
    if existing_client.is_some() {
        info!("Character server client already exists, skipping connection");
        // Only set to CharacterList if we're currently in Connecting state
        if current_char_state.get() == &CharacterScreenState::Connecting {
            info!("connect_to_character_server: Setting state to CharacterList (client exists, state is Connecting)");
            next_state.set(CharacterScreenState::CharacterList);
        } else {
            info!("connect_to_character_server: NOT setting state (client exists, current state: {:?})", current_char_state.get());
        }
        return;
    }

    info!("Connecting to character server");

    // Get selected server info
    if let Some(server) = &session.selected_server {
        // Create character server client
        let mut client = CharServerClient::new(session.clone());

        // Parse IP address
        let ip_bytes = server.ip.to_be_bytes();
        let server_ip = format!(
            "{}.{}.{}.{}",
            ip_bytes[3], ip_bytes[2], ip_bytes[1], ip_bytes[0]
        );

        // Connect to character server
        if let Err(e) = client.connect(&server_ip, server.port) {
            error!("Failed to connect to character server: {:?}", e);
            // TODO: Show error popup and return to server selection
        } else {
            // Insert client as resource
            commands.insert_resource(client);
            // Only set to CharacterList if we're currently in Connecting state
            // This prevents overriding other states like CharacterCreation
            if current_char_state.get() == &CharacterScreenState::Connecting {
                info!("connect_to_character_server: Setting state to CharacterList (new client connected, state is Connecting)");
                next_state.set(CharacterScreenState::CharacterList);
            } else {
                info!("connect_to_character_server: NOT setting state (new client connected, current state: {:?})", current_char_state.get());
            }
        }
    } else {
        error!("No server selected");
        // TODO: Return to server selection
    }
}

pub fn setup_character_list_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut list_events: EventWriter<RequestCharacterListEvent>,
) {
    info!("Setting up character list UI");

    // Create main UI root for character list
    commands
        .spawn((
            Name::new("Character List UI Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
            CharacterListUiRoot,
        ))
        .with_children(|ui| {
            // Background
            ui.spawn((
                Name::new("Background"),
                UiLayout::window().full().pack(),
                UiColor::from(BACKGROUND_PRIMARY),
                Sprite::default(),
                Pickable::IGNORE,
            ));

            // Title text
            ui.spawn((
                Name::new("Title"),
                UiLayout::window()
                    .pos(Rl((50.0, 10.0)))
                    .anchor(Anchor::TopCenter)
                    .pack(),
                UiTextSize::from(Ab(48.0)),
                Text2d::new("Character Selection"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));

            // Character grid container
            create_character_grid(ui, &asset_server);

            // Bottom buttons
            create_bottom_buttons(ui, &asset_server);
        });

    // Request character list data
    list_events.write(RequestCharacterListEvent);
}

pub fn cleanup_character_list_ui(
    mut commands: Commands,
    ui_roots: Query<Entity, With<CharacterListUiRoot>>,
) {
    info!("Cleaning up character list UI");

    for entity in ui_roots.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn update_character_list_ui(
    mut events: EventReader<CharacterListReceivedEvent>,
    assets: Res<CharacterSelectionAssets>,
    mut commands: Commands,
    mut list_resource: ResMut<CharacterListResource>,
    mut character_cards: Query<(Entity, &CharacterSlot, &mut CharacterCard, &Children)>,
    mut empty_slot_query: Query<
        &mut Visibility,
        (
            With<EmptySlotText>,
            Without<CharacterNameText>,
            Without<CharacterLevelText>,
            Without<CreateCharacterButton>,
        ),
    >,
    mut char_name_query: Query<
        (&mut Text2d, &mut Visibility),
        (
            With<CharacterNameText>,
            Without<EmptySlotText>,
            Without<CharacterLevelText>,
        ),
    >,
    mut char_level_query: Query<
        (&mut Text2d, &mut Visibility),
        (
            With<CharacterLevelText>,
            Without<EmptySlotText>,
            Without<CharacterNameText>,
        ),
    >,
    mut create_btn_query: Query<
        &mut Visibility,
        (
            With<CreateCharacterButton>,
            Without<EmptySlotText>,
            Without<CharacterNameText>,
            Without<CharacterLevelText>,
        ),
    >,
    frame_query: Query<Entity, With<CharacterSlotFrame>>,
) {
    for event in events.read() {
        info!(
            "Updating character list with {} slots",
            event.characters.len()
        );

        // Update resource
        list_resource.characters = event.characters.clone();
        list_resource.max_slots = event.max_slots;
        list_resource.available_slots = event.available_slots;

        // Use cached texture handles
        let no_char_frame = &assets.no_char_frame;
        let with_char_frame = &assets.with_char_frame;

        // Update character cards and their UI
        for (slot_entity, slot, mut card, children) in character_cards.iter_mut() {
            if let Some(character_data_option) = event.characters.get(slot.index as usize) {
                card.character = character_data_option.clone();

                // Update character slot height based on character presence
                let slot_height = if character_data_option.is_some() {
                    Rl(30.0) // Double height for slots with characters
                } else {
                    Rl(15.0) // Normal height for empty slots
                };

                // Update the slot's UiLayout with new height
                commands.entity(slot_entity).insert(
                    UiLayout::window()
                        .pos(Rl((
                            25.0 + ((slot.index % 3) as f32 * 25.0),
                            30.0 + ((slot.index / 3) as f32 * 20.0),
                        )))
                        .size((Rl(20.0), slot_height))
                        .anchor(Anchor::Center)
                        .pack(),
                );

                // Iterate through children to find and update UI elements
                for child in children.iter() {
                    // Update character frame texture based on character presence
                    if frame_query.get(child).is_ok() {
                        if character_data_option.is_some() {
                            // Has character - use frame_with_char.png
                            commands
                                .entity(child)
                                .insert(Sprite::from_image(with_char_frame.clone()));
                        } else {
                            // No character - use no_char_frame.png
                            commands
                                .entity(child)
                                .insert(Sprite::from_image(no_char_frame.clone()));
                        }
                    }
                    // Update UI based on whether a character exists
                    if let Some(character) = character_data_option {
                        debug!("Character in slot {}: {}", slot.index, character.name);

                        // Hide empty slot text
                        if let Ok(mut visibility) = empty_slot_query.get_mut(child) {
                            *visibility = Visibility::Hidden;
                        }

                        // Show and update character name
                        if let Ok((mut text, mut visibility)) = char_name_query.get_mut(child) {
                            text.0 = character.name.clone();
                            *visibility = Visibility::Visible;
                        }

                        // Show and update character level
                        if let Ok((mut text, mut visibility)) = char_level_query.get_mut(child) {
                            text.0 = format!(
                                "Lv. {} / J.Lv. {}",
                                character.base_level, character.job_level
                            );
                            *visibility = Visibility::Visible;
                        }

                        // Hide create button
                        if let Ok(mut visibility) = create_btn_query.get_mut(child) {
                            *visibility = Visibility::Hidden;
                        }
                    } else {
                        // No character in this slot - show empty slot UI

                        // Update frame texture for empty slot (redundant but ensures consistency)
                        if frame_query.get(child).is_ok() {
                            commands
                                .entity(child)
                                .insert(Sprite::from_image(no_char_frame.clone()));
                        }

                        // Show empty slot text
                        if let Ok(mut visibility) = empty_slot_query.get_mut(child) {
                            *visibility = Visibility::Visible;
                        }

                        // Hide character name
                        if let Ok((mut text, mut visibility)) = char_name_query.get_mut(child) {
                            text.0 = String::new();
                            *visibility = Visibility::Hidden;
                        }

                        // Hide character level
                        if let Ok((mut text, mut visibility)) = char_level_query.get_mut(child) {
                            text.0 = String::new();
                            *visibility = Visibility::Hidden;
                        }

                        // Show create button
                        if let Ok(mut visibility) = create_btn_query.get_mut(child) {
                            *visibility = Visibility::Visible;
                        }
                    }
                }
            }
        }
    }
}

pub fn update_character_details_panel(selection: Res<CharacterSelectionResource>) {
    if let Some(character) = &selection.selected_character {
        // Update character details display
        debug!("Showing details for character: {}", character.name);
    }
}

// Observer for create character button clicks - bridges to creation
pub fn on_create_character_click(
    trigger: Trigger<Pointer<Click>>,
    create_buttons: Query<&CreateCharacterButton>,
    mut creation_events: EventWriter<OpenCharacterCreationEvent>,
    mut creation_resource: ResMut<CharacterCreationResource>,
    current_state: Res<State<CharacterScreenState>>,
) {
    let entity = trigger.target();

    // Only process clicks when in the correct state
    if *current_state.get() != CharacterScreenState::CharacterList {
        return;
    }

    if let Ok(button) = create_buttons.get(entity) {
        info!("Create button clicked for slot {}", button.slot);
        creation_resource.start_creation(button.slot);
        creation_events.write(OpenCharacterCreationEvent { slot: button.slot });
    }
}
