use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct FpsRoot;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct FpsCounterPlugin;

#[auto_add_system(
    plugin = crate::presentation::ui::fps_counter::FpsCounterPlugin,
    schedule = Startup
)]
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

#[auto_add_system(
    plugin = crate::presentation::ui::fps_counter::FpsCounterPlugin,
    schedule = Update
)]
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
