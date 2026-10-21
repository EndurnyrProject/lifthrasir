use bevy::{
    prelude::*,
    text::{FontSize, FontSourceTemplate},
};

use crate::application::{EditorNotice, EditorSystems};

#[derive(Component, Default, Clone)]
struct NoticeLabel;

pub(super) struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, update_notice.in_set(EditorSystems::Present));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_scene(command_overlay());
    commands.spawn_scene(notice_overlay());
}

fn command_overlay() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            right: px(16),
            width: px(310),
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(Color::srgba(0.02, 0.025, 0.04, 0.88))
        BorderColor::all(Color::srgb(0.35, 0.45, 0.7))
        Pickable::IGNORE
        Children [
            (
                Text("Camera\nWASD - move · Space/Left Alt - up/down\nRMB - look · Wheel - speed\nP - toggle grid\n\nCollision\nLMB / arrows - select cell\nF - toggle walkability\nCtrl+Z / Ctrl+Y - undo / redo\nCtrl+S - save\n\nProps\nClick a prop, drag gizmo axis\nI/K J/L U/O - move\nQ/E - rotate · G/R - gizmo mode\nCtrl+D duplicate · Delete remove")
                TextFont {
                    font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
                    font_size: FontSize::Px(14.0),
                }
                TextColor(Color::srgb(0.9, 0.93, 1.0))
                Pickable::IGNORE
            )
        ]
    }
}

fn notice_overlay() -> impl Scene {
    bsn! {
        NoticeLabel
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: px(18),
            padding: UiRect::axes(px(12), px(8)),
        }
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0))
        Text("")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
            font_size: FontSize::Px(16.0),
        }
        TextColor(Color::srgb(0.9, 0.93, 1.0))
        Pickable::IGNORE
    }
}

fn update_notice(
    time: Res<Time>,
    mut notice: ResMut<EditorNotice>,
    mut labels: Query<(&mut Text, &mut BackgroundColor), With<NoticeLabel>>,
) {
    let Some(timer) = notice.expires.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if timer.is_finished() {
        notice.message.clear();
        notice.expires = None;
    }
    let visible = !notice.message.is_empty();
    for (mut label, mut background) in &mut labels {
        label.0 = notice.message.clone();
        background.0 = if visible {
            Color::srgba(0.02, 0.025, 0.04, 0.88)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
    }
}
