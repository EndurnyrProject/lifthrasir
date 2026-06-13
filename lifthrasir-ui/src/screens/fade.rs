use bevy::prelude::*;
use game_engine::core::state::GameState;

use crate::theme;

const FADE_DURATION_SECS: f32 = 0.3;

pub struct FadeTransitionPlugin;

impl Plugin for FadeTransitionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_fade_overlay);
        app.add_systems(Update, (start_fade_on_transition, tick_fade).chain());
    }
}

/// Full-screen overlay that fades from opaque to transparent after a state change,
/// revealing the newly entered screen.
#[derive(Component)]
struct ScreenFade {
    timer: Timer,
}

fn spawn_fade_overlay(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme::FORGE_SOOT.with_alpha(0.0)),
        GlobalZIndex(i32::MAX - 1),
        Pickable::IGNORE,
        ScreenFade {
            timer: Timer::from_seconds(FADE_DURATION_SECS, TimerMode::Once),
        },
    ));
}

fn start_fade_on_transition(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut fade: Single<(&mut ScreenFade, &mut BackgroundColor)>,
) {
    let triggered = transitions
        .read()
        .any(|event| event.entered.is_some() && event.entered != event.exited);

    if !triggered {
        return;
    }

    let (screen_fade, background) = &mut *fade;
    screen_fade.timer.reset();
    background.0.set_alpha(1.0);
}

fn tick_fade(time: Res<Time>, mut fade: Single<(&mut ScreenFade, &mut BackgroundColor)>) {
    let (screen_fade, background) = &mut *fade;

    if screen_fade.timer.is_finished() {
        return;
    }

    screen_fade.timer.tick(time.delta());
    background
        .0
        .set_alpha(screen_fade.timer.fraction_remaining());
}
