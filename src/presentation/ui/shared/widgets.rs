use super::theme::*;
use crate::presentation::ui::screens::login::interactions::*;
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

// ============================================================================
// Scroll Components
// ============================================================================

/// Tracks scroll state for a scrollable panel
#[derive(Component)]
pub struct ScrollablePanel {
    pub scroll_offset: f32,
    pub max_height: f32,
    pub content_height: f32,
    pub scroll_speed: f32,
}

impl ScrollablePanel {
    pub fn new(max_height: f32) -> Self {
        Self {
            scroll_offset: 0.0,
            max_height,
            content_height: 0.0,
            scroll_speed: SCROLL_SPEED,
        }
    }

    /// Get maximum scroll offset
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.max_height).max(0.0)
    }

    /// Check if scrollbar should be visible
    pub fn needs_scrollbar(&self) -> bool {
        self.content_height > self.max_height
    }

    /// Get scroll position as ratio [0.0, 1.0]
    pub fn scroll_ratio(&self) -> f32 {
        let max_scroll = self.max_scroll();
        if max_scroll > 0.0 {
            self.scroll_offset / max_scroll
        } else {
            0.0
        }
    }

    /// Get visible content ratio [0.0, 1.0]
    pub fn visible_ratio(&self) -> f32 {
        if self.content_height > 0.0 {
            (self.max_height / self.content_height).min(1.0)
        } else {
            1.0
        }
    }
}

/// Marker for the scrollable content container
#[derive(Component)]
pub struct ScrollContent;

/// Marker for the scrollbar container
#[derive(Component)]
pub struct ScrollBar;

/// Marker for the scrollbar thumb (draggable part)
#[derive(Component)]
pub struct ScrollThumb {
    pub is_dragging: bool,
    pub drag_start_y: f32,
    pub scroll_start: f32,
}

impl Default for ScrollThumb {
    fn default() -> Self {
        Self {
            is_dragging: false,
            drag_start_y: 0.0,
            scroll_start: 0.0,
        }
    }
}

/// Marker for the scrollbar track
#[derive(Component)]
pub struct ScrollTrack;

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
            (UiHover::id(), INPUT_BACKGROUND_TRANSPARENT),
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
                    .size((
                        width - (BORDER_WIDTH * 2.0),
                        INPUT_HEIGHT - (BORDER_WIDTH * 2.0),
                    ))
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
    asset_server: &AssetServer,
    text: impl Into<String>,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    size: Option<(f32, f32)>,
    button_type: Option<ButtonType>,
) -> Entity {
    let (button_width, button_height) = size.unwrap_or((120.0, BUTTON_HEIGHT));

    // Load texture using AssetServer
    let texture_handle: Handle<Image> = asset_server.load(TEXTURE_BUTTON);

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

            // Apply texture using AssetServer handle
            {
                background_spawn.insert(Sprite {
                    image: texture_handle,
                    image_mode: SpriteImageMode::Sliced(TextureSlicer {
                        border: BorderRect::all(BUTTON_SLICE_BORDER),
                        center_scale_mode: SliceScaleMode::Stretch,
                        sides_scale_mode: SliceScaleMode::Stretch,
                        max_corner_scale: 1.0,
                    }),
                    ..default()
                });
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
                (UiHover::id(), INPUT_BACKGROUND_TRANSPARENT),
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

/// Create a scrollable panel with support for overflow content
/// Returns a tuple: (panel_entity, content_entity)
/// Add your scrollable children to the content_entity
pub fn scrollable_panel(
    ui: &mut ChildSpawnerCommands,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    width: f32,
    max_height: f32,
) -> (Entity, Entity) {
    let panel_name = name.into();
    let content_width = width - SCROLLBAR_WIDTH - SPACING_SM;
    let mut content_entity = Entity::PLACEHOLDER;

    let panel_entity = ui
        .spawn((
            Name::new(format!("{} - Scrollable Panel", panel_name)),
            UiLayout::window()
                .pos(position)
                .size((width, max_height))
                .pack(),
            ScrollablePanel::new(max_height),
            Pickable::default(),
        ))
        .with_children(|ui| {
            // Clipping container (for future clipping implementation)
            ui.spawn((
                Name::new("Clip Container"),
                UiLayout::window()
                    .pos((0.0, 0.0))
                    .size((content_width, max_height))
                    .pack(),
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Scrollable content container - this is what users add children to
                content_entity = ui
                    .spawn((
                        Name::new("Scroll Content"),
                        UiLayout::window()
                            .pos((0.0, 0.0))
                            .size((content_width, max_height))
                            .pack(),
                        ScrollContent,
                        Pickable::IGNORE,
                    ))
                    .id();
            });

            // Scrollbar (initially hidden)
            ui.spawn((
                Name::new("Scrollbar"),
                UiLayout::window()
                    .pos((content_width + SPACING_SM, 0.0))
                    .size((SCROLLBAR_WIDTH, max_height))
                    .pack(),
                ScrollBar,
                Visibility::Hidden, // Initially hidden until content overflows
                Pickable::IGNORE,
            ))
            .with_children(|ui| {
                // Scrollbar track (background)
                ui.spawn((
                    Name::new("Scroll Track"),
                    UiLayout::window().full().pack(),
                    UiColor::from(SCROLLBAR_TRACK),
                    Sprite::default(),
                    ScrollTrack,
                    Pickable::default(),
                ));

                // Scrollbar thumb (draggable part)
                ui.spawn((
                    Name::new("Scroll Thumb"),
                    UiLayout::window()
                        .pos((0.0, 0.0))
                        .size((SCROLLBAR_WIDTH, SCROLLBAR_MIN_THUMB_HEIGHT))
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), SCROLLBAR_THUMB),
                        (UiHover::id(), SCROLLBAR_THUMB_HOVER),
                    ]),
                    UiHover::new().forward_speed(10.0).backward_speed(5.0),
                    Sprite::default(),
                    ScrollThumb::default(),
                    OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>);
            });
        })
        .id();

    (panel_entity, content_entity)
}
