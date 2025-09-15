use super::{events::*, theme::*, widgets::*};
use crate::{
    core::state::GameState,
    infrastructure::networking::{
        protocols::ro_login::{ServerInfo, ServerType},
        session::UserSession,
    },
};
use bevy::prelude::*;

pub struct ServerSelectionPlugin;

impl Plugin for ServerSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::ServerSelection),
            setup_server_selection_ui,
        )
        .add_systems(
            Update,
            (
                handle_server_selection,
                handle_back_to_login,
                update_server_hover_effects,
            )
                .run_if(in_state(GameState::ServerSelection)),
        )
        .add_systems(
            OnExit(GameState::ServerSelection),
            cleanup_server_selection_ui,
        )
        .add_event::<ServerSelectedEvent>()
        .add_event::<BackToLoginEvent>();
    }
}

#[derive(Component)]
struct ServerSelectionScreen;

#[derive(Component)]
struct ServerListItem {
    server: ServerInfo,
}

#[derive(Component)]
struct ServerListContainer;

#[derive(Component)]
struct BackToLoginButton;

fn setup_server_selection_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    session: Res<UserSession>,
) {
    // Setup UI camera for server selection screen
    commands.spawn((Camera2d, ServerSelectionScreen));

    // Load background image (same as login)
    let background_image = asset_server.load("data/login_screen.png");

    // Create main container with background image
    let mut root_entity = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(SPACING_XXL)),
            ..default()
        },
        ServerSelectionScreen,
    ));

    // Add background image
    root_entity.insert(ImageNode::new(background_image));

    root_entity.with_children(|parent| {
        // Server selection panel - compact size similar to login panel
        parent
            .spawn(ro_panel_custom(400.0, 450.0, PANEL_BACKGROUND_LIGHT))
            .with_children(|parent| {
                parent
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(320.0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::scroll_y(),
                            padding: UiRect::all(Val::Px(SPACING_XS)),
                            margin: UiRect::vertical(Val::Px(SPACING_SM)),
                            ..default()
                        },
                        BackgroundColor(BACKGROUND_PRIMARY_TRANSPARENT),
                        BorderRadius::all(Val::Px(RADIUS_SM)),
                        ServerListContainer,
                    ))
                    .with_children(|parent| {
                        // Add server items
                        for server in &session.server_list {
                            spawn_server_item(parent, server.clone());
                        }
                    });

                // Back to Login button
                parent
                    .spawn((
                        ro_button_with_width("Back to Login", 150.0),
                        BackToLoginButton,
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new("Back to Login"),
                            TextFont::from_font_size(FONT_SIZE_BUTTON),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });
            });
    });
}

fn spawn_server_item(parent: &mut ChildSpawnerCommands, server: ServerInfo) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(SPACING_SM)),
                margin: UiRect::bottom(Val::Px(SPACING_XS)),
                border: UiRect::all(Val::Px(BORDER_WIDTH)),
                ..default()
            },
            BackgroundColor(BUTTON_NORMAL_TRANSPARENT),
            BorderColor(BORDER_COLOR),
            BorderRadius::all(Val::Px(RADIUS_MD)),
            ServerListItem {
                server: server.clone(),
            },
        ))
        .with_children(|parent| {
            // Left side - Server info
            parent
                .spawn((Node {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Start,
                    ..default()
                },))
                .with_children(|parent| {
                    // Server name with status indicators
                    parent
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(SPACING_XS)),
                            ..default()
                        },))
                        .with_children(|parent| {
                            // Server name
                            parent.spawn((
                                Text::new(server.name.clone()),
                                TextFont::from_font_size(FONT_SIZE_BODY),
                                TextColor(TEXT_PRIMARY),
                            ));

                            // New server indicator
                            if server.new_server > 0 {
                                parent.spawn((
                                    Text::new(" [NEW]"),
                                    TextFont::from_font_size(FONT_SIZE_BODY),
                                    TextColor(SUCCESS_COLOR),
                                    Node {
                                        margin: UiRect::left(Val::Px(SPACING_SM)),
                                        ..default()
                                    },
                                ));
                            }

                            // Server type indicator (can be used for maintenance, PvP, etc.)
                            if server.server_type != ServerType::Normal {
                                let type_text = match server.server_type {
                                    ServerType::Maintenance => " [MAINTENANCE]",
                                    ServerType::PvP => " [PVP]",
                                    ServerType::PK => " [PK]",
                                    ServerType::Special(_) => " [SPECIAL]",
                                    ServerType::Normal => "", // Won't reach here due to if condition
                                };
                                parent.spawn((
                                    Text::new(type_text),
                                    TextFont::from_font_size(FONT_SIZE_BODY),
                                    TextColor(WARNING_COLOR),
                                    Node {
                                        margin: UiRect::left(Val::Px(SPACING_SM)),
                                        ..default()
                                    },
                                ));
                            }
                        });
                });

            // Right side - User count
            parent
                .spawn((Node {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::End,
                    ..default()
                },))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new(format!("{}", server.users)),
                        TextFont::from_font_size(FONT_SIZE_SUBTITLE),
                        TextColor(RUNIC_GLOW),
                    ));
                    parent.spawn((
                        Text::new("Players"),
                        TextFont::from_font_size(FONT_SIZE_SMALL),
                        TextColor(TEXT_SECONDARY),
                    ));
                });
        });
}

fn cleanup_server_selection_ui(
    mut commands: Commands,
    query: Query<Entity, With<ServerSelectionScreen>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_server_selection(
    mut interaction_query: Query<
        (
            &Interaction,
            &ServerListItem,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Changed<Interaction>, With<ServerListItem>),
    >,
    mut server_events: EventWriter<ServerSelectedEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut session: ResMut<UserSession>,
) {
    for (interaction, server_item, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BUTTON_PRESSED_TRANSPARENT.into();
                border_color.0 = RUNIC_GLOW;

                // Skip if server is under maintenance
                if server_item.server.server_type == ServerType::Maintenance {
                    warn!(
                        "Cannot select server {} - under maintenance",
                        server_item.server.name
                    );
                    continue;
                }

                info!("Server selected: {}", server_item.server.name);

                // Update session with selected server
                session.selected_server = Some(server_item.server.clone());

                // Send server selected event
                server_events.send(ServerSelectedEvent {
                    server: server_item.server.clone(),
                });

                // Transition to character selection
                next_state.set(GameState::CharacterSelection);
            }
            Interaction::Hovered => {
                *bg_color = BUTTON_HOVER_TRANSPARENT.into();
                border_color.0 = RUNIC_GLOW;
            }
            Interaction::None => {
                *bg_color = BUTTON_NORMAL_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
            }
        }
    }
}

fn handle_back_to_login(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<BackToLoginButton>),
    >,
    mut back_events: EventWriter<BackToLoginEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BUTTON_PRESSED_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;

                info!("Going back to login screen");
                back_events.send(BackToLoginEvent);
                next_state.set(GameState::Login);
            }
            Interaction::Hovered => {
                *bg_color = BUTTON_HOVER_TRANSPARENT.into();
                border_color.0 = RUNIC_GLOW;
            }
            Interaction::None => {
                *bg_color = BUTTON_NORMAL_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
            }
        }
    }
}

fn update_server_hover_effects(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        With<ServerListItem>,
    >,
) {
    for (interaction, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Hovered => {
                *bg_color = BUTTON_HOVER_TRANSPARENT.into();
                border_color.0 = RUNIC_GLOW;
            }
            Interaction::None => {
                *bg_color = BUTTON_NORMAL_TRANSPARENT.into();
                border_color.0 = BORDER_COLOR;
            }
            _ => {}
        }
    }
}
