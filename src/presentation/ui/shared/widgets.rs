use super::theme::*;
use crate::presentation::ui::screens::login::interactions::*;
use crate::presentation::ui::screens::server_selection::interactions::*;
use crate::infrastructure::assets::HierarchicalAssetManager;
use crate::infrastructure::assets::converters::decode_image_from_bytes;
use crate::infrastructure::networking::protocols::ro_login::{ServerInfo, ServerType};
use bevy::prelude::*;
use bevy_lunex::prelude::*;

#[derive(Clone, Copy)]
pub enum InputType {
    Username,
    Password,
}

#[derive(Clone, Copy)]
pub enum ButtonType {
    Login,
}

/// Create a Lunex text input field with interactive states
pub fn text_input(
    ui: &mut ChildSpawnerCommands,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    width: f32,
    input_type: InputType,
) -> Entity {
    let mut spawn_bundle = ui.spawn((
        Name::new(name.into()),
        UiLayout::window()
            .pos(position)
            .size((width, INPUT_HEIGHT))
            .pack(),
        UiColor::new(vec![
            (UiBase::id(), INPUT_BACKGROUND_TRANSPARENT),
            (
                UiHover::id(),
                INPUT_BACKGROUND_TRANSPARENT,
            ),
        ]),
        UiHover::new().forward_speed(10.0).backward_speed(5.0),
        Sprite::default(),
        Pickable::default(),
        LunexInput,
    ));

    // Add specific input type markers
    match input_type {
        InputType::Username => {
            spawn_bundle.insert((LunexUsernameInput, LunexFocusedInput));
        }
        InputType::Password => {
            spawn_bundle.insert(LunexPasswordInput);
        }
    }

    spawn_bundle
        .with_children(|ui| {
            // Simple border background - fills entire input area with transparent color by default, border color on hover
            ui.spawn((
                UiLayout::window().full().pack(),
                UiColor::new(vec![
                    (UiBase::id(), INPUT_BORDER.with_alpha(0.6)),
                    (UiHover::id(), INPUT_BORDER_FOCUS.with_alpha(0.9)),
                ]),
                UiHover::new().forward_speed(10.0).backward_speed(5.0),
                Sprite::default(),
                Pickable::IGNORE,
            ));

            // Input background - slightly smaller to create border effect
            ui.spawn((
                UiLayout::window()
                    .pos((Rl(50.0), Rl(50.0)))
                    .anchor(Anchor::Center)
                    .size((width - (BORDER_WIDTH * 2.0), INPUT_HEIGHT - (BORDER_WIDTH * 2.0)))
                    .pack(),
                UiColor::from(INPUT_BACKGROUND),
                Sprite::default(),
                Pickable::IGNORE,
            ));

            // Text content for the input
            ui.spawn((
                UiLayout::window()
                    .pos((Rh(10.0), Rl(50.0)))
                    .anchor(Anchor::CenterLeft)
                    .pack(),
                UiTextSize::from(Rh(60.0)),
                Text2d::new(""),
                TextFont {
                    font_size: FONT_SIZE_BODY,
                    ..default()
                },
                TextColor(ASHEN_WHITE),
                Pickable::IGNORE,
            ));
        })
        .observe(hover_set::<Pointer<Over>, true>)
        .observe(hover_set::<Pointer<Out>, false>)
        .id()
}

/// Create a Lunex button with texture and hover/pressed states
pub fn textured_button(
    ui: &mut ChildSpawnerCommands,
    asset_manager: Option<&HierarchicalAssetManager>,
    images: &mut ResMut<Assets<Image>>,
    text: impl Into<String>,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    size: Option<(f32, f32)>,
    button_type: Option<ButtonType>,
) -> Entity {
    let (button_width, button_height) = size.unwrap_or((120.0, BUTTON_HEIGHT));

    // Try to load texture from hierarchical asset manager
    let texture_handle = if let Some(manager) = asset_manager {
        if let Ok(texture_data) = manager.load(TEXTURE_BUTTON) {
            match decode_image_from_bytes(&texture_data, TEXTURE_BUTTON) {
                Ok(image) => Some(images.add(image)),
                Err(e) => {
                    warn!("Failed to decode button texture: {}", e);
                    None
                }
            }
        } else {
            warn!("Failed to load button texture from asset manager");
            None
        }
    } else {
        warn!("No hierarchical asset manager available for textured button");
        None
    };

    let mut spawn_bundle = ui.spawn((
        Name::new(name.into()),
        UiLayout::window()
            .pos(position)
            .size((button_width, button_height))
            .pack(),
        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
        Pickable::default(),
    ));

    // Add specific button type markers
    if let Some(btn_type) = button_type {
        match btn_type {
            ButtonType::Login => {
                spawn_bundle.insert(LunexLoginButton);
            }
        }
    }

    spawn_bundle
        .with_children(|ui| {
            // Button background with states
            let mut background_spawn = ui.spawn((
                UiLayout::new(vec![
                    (UiBase::id(), UiLayout::window().full()),
                    (
                        UiHover::id(),
                        UiLayout::window().anchor(Anchor::Center).size(Rl(102.0)),
                    ),
                ]),
                Pickable::IGNORE,
            ));

            // Apply texture if available, otherwise fall back to solid color
            if let Some(texture) = texture_handle {
                background_spawn.insert(Sprite {
                    image: texture,
                    image_mode: SpriteImageMode::Sliced(TextureSlicer {
                        border: BorderRect::all(BUTTON_SLICE_BORDER),
                        center_scale_mode: SliceScaleMode::Stretch,
                        sides_scale_mode: SliceScaleMode::Stretch,
                        max_corner_scale: 1.0,
                    }),
                    ..default()
                });
            } else {
                // Fallback to solid color button style
                background_spawn.insert((Sprite::default()));
            }

            background_spawn.with_children(|ui| {
                // Button text
                ui.spawn((
                    UiLayout::window()
                        .pos(Rl(50.0))
                        .anchor(Anchor::Center)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), TEXT_PRIMARY),
                        (UiHover::id(), RUNIC_GLOW),
                    ]),
                    UiTextSize::from(Rh(20.0)),
                    Text2d::new(text.into()),
                    TextFont { ..default() },
                    Pickable::IGNORE,
                ));
            });
        })
        .id()
}

/// Create a Lunex checkbox
pub fn checkbox(
    ui: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
) -> Entity {
    ui.spawn((
        Name::new("Checkbox Container"),
        UiLayout::window()
            .pos(position)
            .size((200.0, CHECKBOX_SIZE))
            .pack(),
        LunexCheckbox { checked: false },
    ))
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
                (
                    UiHover::id(),
                    INPUT_BACKGROUND_TRANSPARENT,
                ),
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
                UiTextSize::from(Rh(80.0)),
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
            UiTextSize::from(Rh(100.0)),
            Text2d::new(label.into()),
            TextFont {
                font_size: FONT_SIZE_BODY,
                ..default()
            },
            TextColor(TEXT_PRIMARY),
            Pickable::IGNORE,
        ));
    })
    .id()
}

/// Create a status text area for displaying messages
pub fn status_text(ui: &mut ChildSpawnerCommands, position: impl Into<UiValue<Vec2>>) -> Entity {
    ui.spawn((
        Name::new("Status Text"),
        UiLayout::window()
            .pos(position)
            .anchor(Anchor::Center)
            .pack(),
        UiTextSize::from(Ab(FONT_SIZE_BODY)),
        Text2d::new(""),
        TextFont {
            font_size: FONT_SIZE_BODY,
            ..default()
        },
        TextColor(ERROR_COLOR),
        LunexStatusText,
    ))
    .id()
}

pub fn ro_button_with_width(text: impl Into<String>, width: f32) -> impl Bundle {
    ro_button_styled(text, Some(width), BUTTON_HEIGHT)
}

pub fn ro_button_styled(text: impl Into<String>, width: Option<f32>, height: f32) -> impl Bundle {
    let node_width = match width {
        Some(w) => Val::Px(w),
        None => Val::Auto,
    };

    (
        Button,
        Node {
            width: node_width,
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(SPACING_XL)),
            ..default()
        },
        BackgroundColor(BUTTON_NORMAL_TRANSPARENT),
        BorderRadius::all(Val::Px(RADIUS_MD)),
        RoButton,
    )
}

pub fn ro_panel_custom(width: f32, height: f32, background_color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(WINDOW_PADDING)),
            border: UiRect::all(Val::Px(BORDER_WIDTH * 2.0)),
            ..default()
        },
        BackgroundColor(background_color),
        BorderColor(BORDER_COLOR),
        BorderRadius::all(Val::Px(RADIUS_LG)),
        RoPanel,
    )
}

/// Create a server card widget with Lunex
pub fn server_card(
    ui: &mut ChildSpawnerCommands,
    server: &ServerInfo,
    index: usize,
    position: impl Into<UiValue<Vec2>>,
) -> Entity {
    let is_maintenance = server.server_type == ServerType::Maintenance;

    ui.spawn((
        Name::new(format!("Server Card {}", server.name)),
        UiLayout::window()
            .pos(position)
            .size((SERVER_CARD_WIDTH, SERVER_CARD_HEIGHT))
            .pack(),
        UiColor::new(vec![
            (UiBase::id(), SERVER_CARD_BG),
            (UiHover::id(), SERVER_CARD_HOVER),
        ]),
        UiHover::new().forward_speed(10.0).backward_speed(5.0),
        Sprite::default(),
        OnHoverSetCursor::new(if is_maintenance {
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
        // Card border effect
        ui.spawn((
            UiLayout::window().full().pack(),
            UiColor::new(vec![
                (UiBase::id(), SERVER_CARD_BORDER),
                (UiHover::id(), RUNIC_GLOW.with_alpha(0.6)),
            ]),
            UiHover::new().forward_speed(10.0).backward_speed(5.0),
            Sprite::default(),
            Pickable::IGNORE,
        ));

        // Card background (slightly smaller for border effect)
        ui.spawn((
            UiLayout::window()
                .pos(Rl(50.0))
                .anchor(Anchor::Center)
                .size((SERVER_CARD_WIDTH - 2.0, SERVER_CARD_HEIGHT - 2.0))
                .pack(),
            UiColor::from(SERVER_CARD_BG),
            Sprite::default(),
            Pickable::IGNORE,
        ))
        .with_children(|ui| {
            // Left section: Server info
            ui.spawn((
                UiLayout::window()
                    .pos((Rh(10.0), Rl(50.0)))
                    .anchor(Anchor::CenterLeft)
                    .size((240.0, Rl(80.0)))
                    .pack(),
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Server name
                ui.spawn((
                    UiLayout::window()
                        .pos((0.0, Rl(30.0)))
                        .anchor(Anchor::CenterLeft)
                        .pack(),
                    UiTextSize::from(Ab(FONT_SIZE_SUBTITLE)),
                    Text2d::new(&server.name),
                    TextFont {
                        font_size: FONT_SIZE_SUBTITLE,
                        ..default()
                    },
                    TextColor(if is_maintenance { TEXT_SECONDARY } else { TEXT_PRIMARY }),
                    Pickable::IGNORE,
                ));

                // Server badges row
                ui.spawn((
                    UiLayout::window()
                        .pos((0.0, Rl(70.0)))
                        .anchor(Anchor::CenterLeft)
                        .size((200.0, 24.0))
                        .pack(),
                    Pickable::IGNORE,
                ))
                .with_children(|ui| {
                    let mut badge_offset = 0.0;

                    // NEW badge
                    if server.new_server > 0 {
                        spawn_server_badge(ui, "NEW", BADGE_NEW, badge_offset);
                        badge_offset += 50.0;
                    }

                    // Server type badge
                    match server.server_type {
                        ServerType::Maintenance => {
                            spawn_server_badge(ui, "MAINT", BADGE_MAINTENANCE, badge_offset);
                        }
                        ServerType::PvP => {
                            spawn_server_badge(ui, "PVP", BADGE_PVP, badge_offset);
                        }
                        ServerType::PK => {
                            spawn_server_badge(ui, "PK", BADGE_PK, badge_offset);
                        }
                        _ => {}
                    }
                });
            });

            // Right section: Player count
            ui.spawn((
                UiLayout::window()
                    .pos((Rh(10.0), Rl(50.0)))
                    .anchor(Anchor::CenterRight)
                    .size((100.0, Rl(80.0)))
                    .pack(),
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Player count number
                ui.spawn((
                    UiLayout::window()
                        .pos(Rl((50.0, 35.0)))
                        .anchor(Anchor::Center)
                        .pack(),
                    UiTextSize::from(Ab(FONT_SIZE_HEADING)),
                    Text2d::new(format!("{}", server.users)),
                    TextFont {
                        font_size: FONT_SIZE_HEADING,
                        ..default()
                    },
                    TextColor(get_population_color(server.users as u32)),
                    Pickable::IGNORE,
                ));

                // "Players" label
                ui.spawn((
                    UiLayout::window()
                        .pos(Rl((50.0, 65.0)))
                        .anchor(Anchor::Center)
                        .pack(),
                    UiTextSize::from(Ab(FONT_SIZE_SMALL)),
                    Text2d::new("Players"),
                    TextFont {
                        font_size: FONT_SIZE_SMALL,
                        ..default()
                    },
                    TextColor(TEXT_SECONDARY),
                    Pickable::IGNORE,
                ));

                // Player count bar
                spawn_player_count_bar(ui, server.users as u32);
            });
        });
    })
    .id()
}

/// Spawn a server status badge
fn spawn_server_badge(
    ui: &mut ChildSpawnerCommands,
    text: &str,
    color: Color,
    x_offset: f32,
) {
    ui.spawn((
        UiLayout::window()
            .pos((x_offset, Rl(50.0)))
            .anchor(Anchor::CenterLeft)
            .size((45.0, 20.0))
            .pack(),
        UiColor::from(color.with_alpha(0.2)),
        Sprite::default(),
        Pickable::IGNORE,
    ))
    .with_children(|ui| {
        // Badge text
        ui.spawn((
            UiLayout::window()
                .pos(Rl(50.0))
                .anchor(Anchor::Center)
                .pack(),
            UiTextSize::from(Ab(FONT_SIZE_SMALL - 2.0)),
            Text2d::new(text),
            TextFont {
                font_size: FONT_SIZE_SMALL - 2.0,
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        ));
    });
}

/// Create a player count bar visualization
fn spawn_player_count_bar(ui: &mut ChildSpawnerCommands, player_count: u32) {
    // Assume max capacity of 5000 for visualization
    let max_capacity = 5000.0;
    let fill_percentage = (player_count as f32 / max_capacity).min(1.0);
    let bar_color = get_population_color(player_count);

    // Bar background
    ui.spawn((
        UiLayout::window()
            .pos(Rl((50.0, 85.0)))
            .anchor(Anchor::Center)
            .size((80.0, 6.0))
            .pack(),
        UiColor::from(GAUGE_BACKGROUND),
        Sprite::default(),
        Pickable::IGNORE,
    ))
    .with_children(|ui| {
        // Bar fill
        ui.spawn((
            UiLayout::window()
                .pos((0.0, Rl(50.0)))
                .anchor(Anchor::CenterLeft)
                .size((80.0 * fill_percentage, 6.0))
                .pack(),
            UiColor::from(bar_color),
            Sprite::default(),
            Pickable::IGNORE,
        ));
    });
}

/// Get color based on player population
fn get_population_color(player_count: u32) -> Color {
    if player_count < 1000 {
        GAUGE_LOW
    } else if player_count < 2500 {
        GAUGE_MEDIUM
    } else if player_count < 4000 {
        GAUGE_HIGH
    } else {
        GAUGE_FULL
    }
}

/// Create a scrollable server list container
pub fn server_list_container(
    ui: &mut ChildSpawnerCommands,
    position: impl Into<UiValue<Vec2>>,
    size: (f32, f32),
) -> Entity {
    ui.spawn((
        Name::new("Server List Container"),
        UiLayout::window()
            .pos(position)
            .size(size)
            .pack(),
        LunexServerList,
        Pickable::default(),
    ))
    .with_children(|ui| {
        // Scrollable content area
        ui.spawn((
            Name::new("Server List Content"),
            UiLayout::window()
                .pos((0.0, 0.0))
                .anchor(Anchor::TopLeft)
                .size((size.0, 1000.0)) // Height will be dynamic based on server count
                .pack(),
            Pickable::IGNORE,
        ));
    })
    .id()
}
