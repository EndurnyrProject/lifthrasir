use super::shared::theme::*;
use bevy::prelude::*;
use std::time::Duration;

#[derive(Component)]
pub struct PopupNotification;

#[derive(Component)]
pub struct PopupTimer {
    pub timer: Timer,
}

#[derive(Component)]
pub struct PopupMessage;

#[derive(Component, Clone, Copy, Debug)]
pub enum PopupType {
    Error,
    Success,
    Warning,
    Info,
}

#[derive(Event)]
pub struct ShowPopupEvent {
    pub message: String,
    pub popup_type: PopupType,
    pub duration: Duration,
}

impl ShowPopupEvent {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            popup_type: PopupType::Error,
            duration: Duration::from_secs(5),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            popup_type: PopupType::Success,
            duration: Duration::from_secs(3),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            popup_type: PopupType::Warning,
            duration: Duration::from_secs(4),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            popup_type: PopupType::Info,
            duration: Duration::from_secs(3),
        }
    }
}

pub fn spawn_popup_system(
    mut commands: Commands,
    mut events: EventReader<ShowPopupEvent>,
    existing_popup: Query<Entity, With<PopupNotification>>,
) {
    for event in events.read() {
        // Remove existing popup if any
        for entity in existing_popup.iter() {
            commands.entity(entity).despawn();
        }

        let (bg_color, border_color, text_color, icon) = match event.popup_type {
            PopupType::Error => (ERROR_COLOR_TRANSPARENT, ERROR_COLOR, TEXT_PRIMARY, "X"),
            PopupType::Success => (SUCCESS_COLOR_TRANSPARENT, SUCCESS_COLOR, TEXT_PRIMARY, "V"),
            PopupType::Warning => (WARNING_COLOR_TRANSPARENT, WARNING_COLOR, TEXT_PRIMARY, "!"),
            PopupType::Info => (INFO_COLOR_TRANSPARENT, RUNIC_GLOW, TEXT_PRIMARY, "i"),
        };

        // Create popup container
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(50.0),
                    align_self: AlignSelf::Center,
                    padding: UiRect::all(Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    column_gap: Val::Px(15.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                BackgroundColor(bg_color),
                BorderColor(border_color),
                BorderRadius::all(Val::Px(RADIUS_MD)),
                PopupNotification,
                PopupTimer {
                    timer: Timer::new(event.duration, TimerMode::Once),
                },
                event.popup_type,
            ))
            .with_children(|parent| {
                // Icon
                parent.spawn((
                    Text::new(icon),
                    TextFont::from_font_size(24.0),
                    TextColor(text_color),
                ));

                // Message
                parent.spawn((
                    Text::new(event.message.clone()),
                    TextFont::from_font_size(FONT_SIZE_BODY),
                    TextColor(text_color),
                    PopupMessage,
                ));

                // Close button
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            margin: UiRect::left(Val::Px(20.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new("✕"),
                            TextFont::from_font_size(18.0),
                            TextColor(text_color),
                        ));
                    });
            });
    }
}

pub fn update_popup_timer_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut PopupTimer), With<PopupNotification>>,
) {
    for (entity, mut popup_timer) in query.iter_mut() {
        popup_timer.timer.tick(time.delta());
        if popup_timer.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn handle_popup_close_system(
    mut commands: Commands,
    interaction_query: Query<(&Interaction, Entity), (Changed<Interaction>, With<Button>)>,
    parent_query: Query<&ChildOf>,
    popup_query: Query<Entity, With<PopupNotification>>,
) {
    for (interaction, button_entity) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            if let Ok(parent) = parent_query.get(button_entity) {
                if popup_query.contains(parent.parent()) {
                    commands.entity(parent.parent()).despawn();
                }
            }
        }
    }
}

pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ShowPopupEvent>().add_systems(
            Update,
            (
                spawn_popup_system,
                update_popup_timer_system,
                handle_popup_close_system,
            ),
        );
    }
}
