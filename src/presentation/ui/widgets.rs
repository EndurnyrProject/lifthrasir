use super::theme::*;
use bevy::prelude::*;
use bevy_lunex::prelude::*;

/// Create a Lunex text input field with interactive states
pub fn text_input(
    commands: &mut Commands,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    width: f32,
) -> Entity {
    commands.spawn((
        Name::new(name.into()),
        UiLayout::window()
            .pos(position)
            .size((width, INPUT_HEIGHT))
            .pack(),
        UiColor::new(vec![
            (UiBase::id(), INPUT_BACKGROUND_TRANSPARENT),
            (UiHover::id(), Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE)),
        ]),
        UiHover::new().forward_speed(10.0).backward_speed(5.0),
        Sprite::default(),
        Pickable::default(),
    ))
    .with_children(|ui| {
        // Border effect on hover/focus
        ui.spawn((
            UiLayout::window().full().pack(),
            UiColor::new(vec![
                (UiBase::id(), Color::NONE),
                (UiHover::id(), RUNIC_GLOW.with_alpha(0.3)),
            ]),
            UiHover::new().forward_speed(10.0).backward_speed(5.0),
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
    .id()
}

/// Create a Lunex button with hover and pressed states
pub fn button(
    commands: &mut Commands,
    text: impl Into<String>,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    width: Option<f32>,
) -> Entity {
    let button_width = width.unwrap_or(120.0);

    commands.spawn((
        Name::new(name.into()),
        UiLayout::window()
            .pos(position)
            .size((button_width, BUTTON_HEIGHT))
            .pack(),
        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
    ))
    .with_children(|ui| {
        // Button background with states
        ui.spawn((
            UiLayout::new(vec![
                (UiBase::id(), UiLayout::window().full()),
                (UiHover::id(), UiLayout::window().anchor(Anchor::Center).size(Rl(102.0))),
            ]),
            UiHover::new().forward_speed(15.0).backward_speed(6.0),
            UiColor::new(vec![
                (UiBase::id(), BUTTON_NORMAL_TRANSPARENT),
                (UiHover::id(), BUTTON_HOVER_TRANSPARENT),
            ]),
            Sprite::default(),
            Pickable::IGNORE,
        ))
        .with_children(|ui| {
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
                UiHover::new().forward_speed(15.0).backward_speed(6.0),
                UiTextSize::from(Rh(55.0)),
                Text2d::new(text.into()),
                TextFont {
                    font_size: FONT_SIZE_BUTTON,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
    })
    .id()
}

/// Create a Lunex panel with transparency
pub fn panel(
    commands: &mut Commands,
    name: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    size: (f32, f32),
    background_color: Color,
) -> Entity {
    commands.spawn((
        Name::new(name.into()),
        UiLayout::window()
            .pos(position)
            .size(size)
            .pack(),
        UiColor::from(background_color),
        Sprite::default(),
    ))
    .id()
}

/// Create a Lunex label
pub fn label(
    commands: &mut Commands,
    text: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
    color: Color,
    font_size: f32,
) -> Entity {
    commands.spawn((
        UiLayout::window()
            .pos(position)
            .anchor(Anchor::CenterLeft)
            .pack(),
        UiTextSize::from(Rh(100.0)),
        Text2d::new(text.into()),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(color),
    ))
    .id()
}

/// Create a Lunex checkbox
pub fn checkbox(
    commands: &mut Commands,
    label: impl Into<String>,
    position: impl Into<UiValue<Vec2>>,
) -> Entity {
    commands.spawn((
        Name::new("Checkbox Container"),
        UiLayout::window()
            .pos(position)
            .size((200.0, CHECKBOX_SIZE))
            .pack(),
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
                (UiHover::id(), Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_SUBTLE)),
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
pub fn status_text(
    commands: &mut Commands,
    position: impl Into<UiValue<Vec2>>,
) -> Entity {
    commands.spawn((
        Name::new("Status Text"),
        UiLayout::window()
            .pos(position)
            .anchor(Anchor::Center)
            .pack(),
        UiTextSize::from(Rh(100.0)),
        Text2d::new(""),
        TextFont {
            font_size: FONT_SIZE_BODY,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.4, 0.4)),
    ))
    .id()
}

// Compatibility functions for old Bevy UI widgets (used by server_selection.rs)
use super::components::*;

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