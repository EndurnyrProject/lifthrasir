use super::{interactions::*, resources::*};
use crate::{
    core::state::GameState,
    infrastructure::{
        assets::{HierarchicalAssetManager, loading_states::AssetLoadingState},
        networking::{
            protocols::ro_login::{ServerInfo, ServerType},
            session::UserSession,
        },
    },
    presentation::ui::{events::*, shared::*},
};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_lunex::prelude::*;

pub fn setup_server_selection_ui_once(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    asset_manager: Option<Res<HierarchicalAssetManager>>,
    mut images: ResMut<Assets<Image>>,
    session: Res<UserSession>,
    mut state: ResMut<ServerSelectionState>,
) {
    if state.initialized {
        return;
    }
    state.initialized = true;

    setup_server_selection_ui(commands, asset_server, asset_manager, images, session);
}

pub fn setup_server_selection_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    _asset_manager: Option<Res<HierarchicalAssetManager>>,
    _images: ResMut<Assets<Image>>,
    session: Res<UserSession>,
) {
    // Spawn UI camera
    commands.spawn((
        Camera2d,
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
        RenderLayers::from_layers(&[0, 1]),
        ServerSelectionScreen,
    ));

    // Load background image
    let background_image = asset_server.load("data/login_screen.png");

    // Create UI root
    commands
        .spawn((
            Name::new("Server Selection UI Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
            ServerSelectionScreen,
        ))
        .with_children(|ui| {
            // Background image
            ui.spawn((
                Name::new("Background"),
                UiLayout::window().full().pack(),
                Sprite::from_image(background_image),
                Pickable::IGNORE,
            ));

            // Simple server list container
            ui.spawn((
                Name::new("Server List"),
                UiLayout::window()
                    .pos(Rl((50.0, 70.0)))
                    .anchor(Anchor::Center)
                    .size((600.0, 400.0))
                    .pack(),
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Add server items as simple text
                for (index, server) in session.server_list.iter().enumerate() {
                    let y_position = (index as f32 * 45.0) - 150.0; // 45px spacing between items

                    // Server name text button
                    ui.spawn((
                        Name::new(format!("Server {}", server.name)),
                        UiLayout::window()
                            .pos((Rl(50.0), y_position))
                            .anchor(Anchor::Center)
                            .size((500.0, 40.0))
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), Color::NONE),
                            (UiHover::id(), Color::NONE),
                        ]),
                        UiHover::new().forward_speed(10.0).backward_speed(5.0),
                        OnHoverSetCursor::new(if server.server_type == ServerType::Maintenance {
                            SystemCursorIcon::NotAllowed
                        } else {
                            SystemCursorIcon::Pointer
                        }),
                        Pickable::default(),
                        LunexServerCard {
                            server_index: index,
                            is_selected: false,
                        },
                    ))
                    .with_children(|ui| {
                        // Server name with glow container
                        ui.spawn((
                            UiLayout::window()
                                .pos(Rl(50.0))
                                .anchor(Anchor::Center)
                                .pack(),
                            Pickable::IGNORE,
                        ))
                        .with_children(|ui| {
                            // Shadow/glow layer (rendered behind)
                            ui.spawn((
                                UiLayout::window()
                                    .pos((1.0, 1.0))
                                    .anchor(Anchor::Center)
                                    .pack(),
                                UiTextSize::from(Ab(FONT_SIZE_SERVER)),
                                Text2d::new(&server.name),
                                TextFont {
                                    font_size: FONT_SIZE_SERVER,
                                    ..default()
                                },
                                TextColor(Color::NONE), // Initially invisible
                                LunexServerGlow { index },
                                Pickable::IGNORE,
                            ));

                            // Main text layer
                            ui.spawn((
                                UiLayout::window()
                                    .pos((0.0, 0.0))
                                    .anchor(Anchor::Center)
                                    .pack(),
                                UiTextSize::from(Ab(FONT_SIZE_SERVER)),
                                Text2d::new(&server.name),
                                TextFont {
                                    font_size: FONT_SIZE_SERVER,
                                    ..default()
                                },
                                TextColor(if server.server_type == ServerType::Maintenance {
                                    TEXT_SECONDARY
                                } else {
                                    TEXT_PRIMARY
                                }),
                                LunexServerText { index },
                                Pickable::IGNORE,
                            ));
                        });
                    })
                    .observe(on_server_item_click)
                    .observe(on_server_item_hover)
                    .observe(on_server_item_hover_exit);
                }
            });

            // Bottom buttons positioned directly on background
            ui.spawn((
                Name::new("Buttons Container"),
                UiLayout::window()
                    .pos(Rl((25.0, 85.0)))
                    .anchor(Anchor::CenterLeft)
                    .size((500.0, 50.0))
                    .pack(),
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Back to Login button
                ui.spawn((
                    Name::new("Back Button"),
                    UiLayout::window()
                        .pos((50.0, Rl(50.0)))
                        .anchor(Anchor::Center)
                        .size((150.0, BUTTON_HEIGHT))
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), BUTTON_NORMAL_TRANSPARENT),
                        (UiHover::id(), BUTTON_HOVER_TRANSPARENT),
                    ]),
                    UiHover::new().forward_speed(10.0).backward_speed(5.0),
                    Sprite::default(),
                    OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                    Pickable::default(),
                    BackToLoginButton,
                ))
                .with_children(|ui| {
                    ui.spawn((
                        UiLayout::window()
                            .pos(Rl(50.0))
                            .anchor(Anchor::Center)
                            .pack(),
                        UiTextSize::from(Ab(FONT_SIZE_BUTTON)),
                        Text2d::new("Back to Login"),
                        TextFont {
                            font_size: FONT_SIZE_BUTTON,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                        Pickable::IGNORE,
                    ));
                })
                .observe(on_back_button_click);

                // Connect button
                ui.spawn((
                    Name::new("Connect Button"),
                    UiLayout::window()
                        .pos((350.0, Rl(50.0)))
                        .anchor(Anchor::Center)
                        .size((150.0, BUTTON_HEIGHT))
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), BUTTON_NORMAL_TRANSPARENT.with_alpha(0.3)),
                        (UiHover::id(), BUTTON_HOVER_TRANSPARENT),
                    ]),
                    UiHover::new().forward_speed(10.0).backward_speed(5.0),
                    Sprite::default(),
                    OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                    Pickable::default(),
                    ConnectButton,
                    LunexConnectButton,
                ))
                .with_children(|ui| {
                    ui.spawn((
                        UiLayout::window()
                            .pos(Rl(50.0))
                            .anchor(Anchor::Center)
                            .pack(),
                        UiTextSize::from(Ab(FONT_SIZE_BUTTON)),
                        Text2d::new("Connect"),
                        TextFont {
                            font_size: FONT_SIZE_BUTTON,
                            ..default()
                        },
                        TextColor(TEXT_SECONDARY),
                        Pickable::IGNORE,
                    ));
                })
                .observe(on_connect_button_click);
            });
        });
}

pub fn cleanup_server_selection_ui(
    mut commands: Commands,
    query: Query<Entity, With<ServerSelectionScreen>>,
    mut state: ResMut<ServerSelectionState>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    state.initialized = false;
    state.selected_server_index = None;
}

// Observer for server item clicks
pub fn on_server_item_click(
    trigger: Trigger<Pointer<Click>>,
    mut commands: Commands,
    mut card_query: Query<&mut LunexServerCard>,
    mut text_query: Query<(&mut TextColor, &LunexServerText)>,
    mut state: ResMut<ServerSelectionState>,
    session: Res<UserSession>,
) {
    let target_entity = trigger.target();

    // First, get the server index from the clicked item
    let server_index = if let Ok(card) = card_query.get(target_entity) {
        // Check if server is under maintenance
        if let Some(server) = session.server_list.get(card.server_index) {
            if server.server_type == ServerType::Maintenance {
                warn!("Cannot select server {} - under maintenance", server.name);
                return;
            }
        }
        card.server_index
    } else {
        return;
    };

    // Deselect all items and select the clicked one
    for mut card in &mut card_query {
        card.is_selected = false;
    }

    // Now select the clicked item
    if let Ok(mut card) = card_query.get_mut(target_entity) {
        card.is_selected = true;
    }

    // Update selection components
    commands.entity(target_entity).insert(LunexSelectedServer);

    // Update text colors for all server items
    for (mut text_color, server_text) in &mut text_query {
        if server_text.index == server_index {
            text_color.0 = RUNIC_GLOW; // Selected color
        } else if let Some(server) = session.server_list.get(server_text.index) {
            text_color.0 = if server.server_type == ServerType::Maintenance {
                TEXT_SECONDARY
            } else {
                TEXT_PRIMARY
            };
        }
    }

    state.selected_server_index = Some(server_index);
    info!("Server {} selected", session.server_list[server_index].name);

    // Update connect button to be enabled
    update_connect_button_state(&mut commands, true);
}

// Observer for back button click
pub fn on_back_button_click(
    _trigger: Trigger<Pointer<Click>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut back_events: EventWriter<BackToLoginEvent>,
) {
    info!("Going back to login screen");
    back_events.send(BackToLoginEvent);
    next_state.set(GameState::Login);
}

// Observer for connect button click
pub fn on_connect_button_click(
    _trigger: Trigger<Pointer<Click>>,
    state: Res<ServerSelectionState>,
    mut session: ResMut<UserSession>,
    mut server_events: EventWriter<ServerSelectedEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(index) = state.selected_server_index {
        if let Some(server) = session.server_list.get(index).cloned() {
            info!("Connecting to server: {}", server.name);

            // Update session with selected server
            session.selected_server = Some(server.clone());

            // Send server selected event
            server_events.send(ServerSelectedEvent { server: server });

            // Transition to character selection
            next_state.set(GameState::CharacterSelection);
        }
    }
}

pub fn handle_server_card_click(
    mut text_query: Query<(&mut TextColor, &LunexServerText)>,
    card_query: Query<&LunexServerCard>,
    session: Res<UserSession>,
) {
    // Find which server is selected
    let selected_index = card_query
        .iter()
        .find(|card| card.is_selected)
        .map(|card| card.server_index);

    // Update text colors based on selection and hover
    for (mut text_color, server_text) in &mut text_query {
        if Some(server_text.index) == selected_index {
            text_color.0 = RUNIC_GLOW; // Selected color
        } else if let Some(server) = session.server_list.get(server_text.index) {
            text_color.0 = if server.server_type == ServerType::Maintenance {
                TEXT_SECONDARY
            } else {
                TEXT_PRIMARY
            };
        }
    }
}

pub fn handle_connect_button(
    state: Res<ServerSelectionState>,
    mut button_query: Query<(&Children, &mut UiColor), With<LunexConnectButton>>,
    mut text_query: Query<&mut TextColor>,
) {
    if let Ok((children, mut color)) = button_query.get_single_mut() {
        let is_enabled = state.selected_server_index.is_some();

        if is_enabled {
            *color = UiColor::new(vec![
                (UiBase::id(), BUTTON_NORMAL_TRANSPARENT),
                (UiHover::id(), BUTTON_HOVER_TRANSPARENT),
            ]);

            // Update text color
            for child in children.iter() {
                if let Ok(mut text_color) = text_query.get_mut(child) {
                    text_color.0 = TEXT_PRIMARY;
                }
            }
        } else {
            *color = UiColor::new(vec![
                (UiBase::id(), BUTTON_NORMAL_TRANSPARENT.with_alpha(0.3)),
                (UiHover::id(), BUTTON_NORMAL_TRANSPARENT.with_alpha(0.3)),
            ]);

            // Update text color
            for child in children.iter() {
                if let Ok(mut text_color) = text_query.get_mut(child) {
                    text_color.0 = TEXT_SECONDARY;
                }
            }
        }
    }
}

pub fn handle_back_to_login(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut back_events: EventWriter<BackToLoginEvent>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        info!("ESC pressed, going back to login");
        back_events.send(BackToLoginEvent);
        next_state.set(GameState::Login);
    }
}

pub fn handle_keyboard_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ServerSelectionState>,
    session: Res<UserSession>,
    mut commands: Commands,
    card_query: Query<(Entity, &LunexServerCard)>,
) {
    if session.server_list.is_empty() {
        return;
    }

    let mut new_index = state.selected_server_index;

    if keys.just_pressed(KeyCode::ArrowUp) {
        new_index = match state.selected_server_index {
            Some(i) if i > 0 => Some(i - 1),
            Some(0) => Some(session.server_list.len() - 1),
            Some(_) => Some(0),
            None => Some(0),
        };
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        new_index = match state.selected_server_index {
            Some(i) if i < session.server_list.len() - 1 => Some(i + 1),
            Some(i) if i == session.server_list.len() - 1 => Some(0),
            Some(_) => Some(0),
            None => Some(0),
        };
    }

    if new_index != state.selected_server_index {
        state.selected_server_index = new_index;

        // Update visual selection
        for (entity, card) in card_query.iter() {
            if Some(card.server_index) == new_index {
                commands.entity(entity).insert(LunexSelectedServer);
            } else {
                commands.entity(entity).remove::<LunexSelectedServer>();
            }
        }

        // Update connect button
        update_connect_button_state(&mut commands, new_index.is_some());
    }

    // Handle Enter key for connection
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        if state.selected_server_index.is_some() {
            // Trigger connection (same as clicking connect button)
            // This will be handled by the connect button observer
        }
    }
}

pub fn update_connect_button_state(commands: &mut Commands, enabled: bool) {
    // This is a helper function that would need access to the connect button entity
    // In a real implementation, you might track the button entity or query for it
    _ = (commands, enabled); // Suppress unused warning for now
}

// Observer for server item hover
pub fn on_server_item_hover(
    trigger: Trigger<Pointer<Over>>,
    mut commands: Commands,
    card_query: Query<&LunexServerCard>,
    session: Res<UserSession>,
) {
    let target_entity = trigger.target();

    // Check if this is a maintenance server
    if let Ok(card) = card_query.get(target_entity) {
        if let Some(server) = session.server_list.get(card.server_index) {
            if server.server_type == ServerType::Maintenance {
                return; // Don't apply hover effects to maintenance servers
            }
        }

        // Add hover component
        commands.entity(target_entity).insert(LunexServerHovered);
    }
}

// Observer for server item hover exit
pub fn on_server_item_hover_exit(trigger: Trigger<Pointer<Out>>, mut commands: Commands) {
    let target_entity = trigger.target();
    commands
        .entity(target_entity)
        .remove::<LunexServerHovered>();
}

// System to handle hover effects on server text
pub fn handle_server_hover_effects(
    server_card_query: Query<(
        &LunexServerCard,
        Option<&LunexServerHovered>,
        Option<&LunexSelectedServer>,
        &Children,
    )>,
    mut text_query: Query<(&mut TextFont, &mut TextColor, &LunexServerText), With<LunexServerText>>,
    mut glow_query: Query<
        (&mut TextFont, &mut TextColor, &LunexServerGlow),
        Without<LunexServerText>,
    >,
    children_query: Query<&Children>,
    session: Res<UserSession>,
) {
    for (card, hovered, selected, card_children) in &server_card_query {
        let is_hovered = hovered.is_some();
        let is_selected = selected.is_some();

        // Get the server info
        let server = match session.server_list.get(card.server_index) {
            Some(s) => s,
            None => continue,
        };

        // Apply effects to text children (need to traverse nested structure)
        for card_child in card_children.iter() {
            // Get the container's children (glow and main text)
            if let Ok(container_children) = children_query.get(card_child) {
                for text_entity in container_children.iter() {
                    // Update main text
                    if let Ok((mut font, mut color, server_text)) = text_query.get_mut(text_entity)
                    {
                        if server_text.index != card.server_index {
                            continue;
                        }

                        // Update font size
                        if is_hovered || is_selected {
                            font.font_size = FONT_SIZE_SERVER_HOVER;
                        } else {
                            font.font_size = FONT_SIZE_SERVER;
                        }

                        // Update color
                        if is_selected {
                            color.0 = RUNIC_GLOW;
                        } else if is_hovered {
                            color.0 = RUNIC_GLOW.with_alpha(0.95);
                        } else if server.server_type == ServerType::Maintenance {
                            color.0 = TEXT_SECONDARY;
                        } else {
                            color.0 = TEXT_PRIMARY;
                        }
                    }

                    // Update glow layer
                    if let Ok((mut glow_font, mut glow_color, server_glow)) =
                        glow_query.get_mut(text_entity)
                    {
                        if server_glow.index != card.server_index {
                            continue;
                        }

                        // Update glow font size
                        if is_hovered || is_selected {
                            glow_font.font_size = FONT_SIZE_SERVER_HOVER;
                        } else {
                            glow_font.font_size = FONT_SIZE_SERVER;
                        }

                        // Update glow color (shadow effect)
                        if is_selected {
                            glow_color.0 = RUNIC_GLOW.with_alpha(0.4);
                        } else if is_hovered {
                            glow_color.0 = RUNIC_GLOW.with_alpha(0.3);
                        } else {
                            glow_color.0 = Color::NONE;
                        }
                    }
                }
            }
        }
    }
}