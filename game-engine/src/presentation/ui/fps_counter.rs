use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct FpsRoot;

pub struct FpsCounterPlugin;

impl Plugin for FpsCounterPlugin {
    fn build(&self, app: &mut App) {
        // Note: FrameTimeDiagnosticsPlugin should be added by the application setup
        // to avoid duplicate plugin errors
        app.add_systems(Startup, setup_fps_counter)
            .add_systems(Update, update_fps_text);
    }
}

fn setup_fps_counter(mut commands: Commands) {
    commands
        .spawn((
            FpsRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(40.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .with_children(|parent| {
            parent.spawn((
                FpsText,
                Text::new("FPS: --"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn update_fps_text(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&mut Text, &mut TextColor), With<FpsText>>,
) {
    for (mut text, mut color) in &mut query {
        if let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
        {
            **text = format!("FPS: {:.0}", fps);

            *color = if fps >= 120.0 {
                TextColor(Color::srgb(0.0, 1.0, 0.0))
            } else if fps >= 60.0 {
                TextColor(Color::srgb(1.0, 1.0, 0.0))
            } else if fps >= 30.0 {
                TextColor(Color::srgb(1.0, 0.5, 0.0))
            } else {
                TextColor(Color::srgb(1.0, 0.0, 0.0))
            };
        }
    }
}
