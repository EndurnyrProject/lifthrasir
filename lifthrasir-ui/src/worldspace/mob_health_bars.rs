//! Combat health bars over mobs. A bar pops above a mob on each
//! `DisplayDamageNumber` it is the target of, tracks the server-authoritative
//! `UnitHealth`, and fades out a few seconds after the last hit. Same
//! screen-projection pattern as nameplates: an absolute `bevy_ui` node whose
//! `left`/`top` follow `Camera::world_to_viewport` every frame.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::combat::events::DisplayDamageNumber;
use game_engine::domain::entities::components::UnitHealth;
use game_engine::domain::entities::markers::Mob;

use crate::theme;
use crate::worldspace::{WorldCameraFilter, viewport_to_ui};

const BAR_WIDTH: f32 = 44.0;
const BAR_HEIGHT: f32 = 6.0;
/// Pixels above the entity origin, below skill cast labels (88).
const BAR_HEAD_GAP: f32 = 74.0;
/// How long a bar lingers after the last hit before it is gone.
const LINGER_SECS: f32 = 5.0;
/// Fade-out window at the end of the linger.
const FADE_SECS: f32 = 0.8;
/// Above nameplates (100), below damage numbers (150).
const BAR_Z: i32 = 120;

pub struct MobHealthBarPlugin;

impl Plugin for MobHealthBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (engage_bars, tick_bars, follow_bars)
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all);
    }
}

#[derive(Component)]
struct MobHealthBar {
    target: Entity,
    linger: Timer,
}

#[derive(Component)]
struct MobHealthBarFill;

fn fill_percent(hp: u32, max_hp: u32) -> f32 {
    (hp as f32 / max_hp.max(1) as f32).clamp(0.0, 1.0) * 100.0
}

/// Alpha multiplier for the fade-out window at the end of the linger.
fn fade_alpha(remaining_secs: f32) -> f32 {
    (remaining_secs / FADE_SECS).clamp(0.0, 1.0)
}

fn spawn_bar(commands: &mut Commands, target: Entity) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(BAR_WIDTH),
            height: Val::Px(BAR_HEIGHT),
            padding: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(theme::FIELD),
        GlobalZIndex(BAR_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
        MobHealthBar {
            target,
            linger: Timer::from_seconds(LINGER_SECS, TimerMode::Once),
        },
        children![(
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme::HEALTH_RED),
            Pickable::IGNORE,
            MobHealthBarFill,
        )],
    ));
}

/// Pops (or re-arms) a bar for every mob a damage number targets.
fn engage_bars(
    mut events: MessageReader<DisplayDamageNumber>,
    mut commands: Commands,
    mobs: Query<(), (With<Mob>, With<UnitHealth>)>,
    mut bars: Query<&mut MobHealthBar>,
    mut spawned: Local<Vec<Entity>>,
) {
    spawned.clear();
    for event in events.read() {
        if mobs.get(event.entity).is_err() || spawned.contains(&event.entity) {
            continue;
        }
        match bars.iter_mut().find(|bar| bar.target == event.entity) {
            Some(mut bar) => bar.linger.reset(),
            None => {
                spawn_bar(&mut commands, event.entity);
                spawned.push(event.entity);
            }
        }
    }
}

/// Ticks the linger, despawns expired/orphaned bars, and updates fill + fade.
fn tick_bars(
    time: Res<Time>,
    mut commands: Commands,
    mut bars: Query<
        (Entity, &mut MobHealthBar, &mut BackgroundColor, &Children),
        Without<MobHealthBarFill>,
    >,
    targets: Query<&UnitHealth>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<MobHealthBarFill>>,
) {
    for (entity, mut bar, mut background, children) in &mut bars {
        bar.linger.tick(time.delta());
        let Ok(health) = targets.get(bar.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if bar.linger.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        let alpha = fade_alpha(bar.linger.remaining_secs());
        background.0 = theme::FIELD.with_alpha(theme::FIELD.alpha() * alpha);
        for child in children {
            let Ok((mut node, mut fill_color)) = fills.get_mut(*child) else {
                continue;
            };
            node.width = Val::Percent(fill_percent(health.hp, health.max_hp));
            fill_color.0 = theme::HEALTH_RED.with_alpha(alpha);
        }
    }
}

fn follow_bars(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut bars: Query<(&MobHealthBar, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (bar, mut node, mut visibility) in &mut bars {
        let Ok(target_transform) = targets.get(bar.target) else {
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - BAR_WIDTH / 2.0);
                node.top = Val::Px(pos.y - BAR_HEAD_GAP);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all(mut commands: Commands, bars: Query<Entity, With<MobHealthBar>>) {
    for entity in &bars {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fill_percent_is_clamped() {
        assert_eq!(fill_percent(50, 100), 50.0);
        assert_eq!(fill_percent(0, 100), 0.0);
        assert_eq!(fill_percent(200, 100), 100.0);
        assert_eq!(fill_percent(5, 0), 100.0);
    }

    #[test]
    fn fade_alpha_ramps_only_at_the_end() {
        assert_eq!(fade_alpha(LINGER_SECS), 1.0);
        assert_eq!(fade_alpha(FADE_SECS), 1.0);
        assert!(fade_alpha(FADE_SECS / 2.0) < 1.0);
        assert_eq!(fade_alpha(0.0), 0.0);
    }

    fn tick_app(target_alive: bool, elapsed: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let target = app.world_mut().spawn_empty().id();
        if target_alive {
            app.world_mut()
                .entity_mut(target)
                .insert(UnitHealth { hp: 30, max_hp: 60 });
        }
        let mut linger = Timer::from_seconds(LINGER_SECS, TimerMode::Once);
        linger.set_elapsed(Duration::from_secs_f32(elapsed));
        let fill = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(theme::HEALTH_RED),
                MobHealthBarFill,
            ))
            .id();
        let bar = app
            .world_mut()
            .spawn((
                MobHealthBar { target, linger },
                BackgroundColor(theme::FIELD),
                Node::default(),
            ))
            .add_child(fill)
            .id();
        app.add_systems(Update, tick_bars);
        app.update();
        (app, bar)
    }

    #[test]
    fn expired_bar_despawns() {
        let (mut app, bar) = tick_app(true, LINGER_SECS);
        assert!(app.world_mut().get_entity(bar).is_err());
    }

    #[test]
    fn orphaned_bar_despawns() {
        let (mut app, bar) = tick_app(false, 0.0);
        assert!(app.world_mut().get_entity(bar).is_err());
    }

    #[test]
    fn live_bar_tracks_hp_fill() {
        let (mut app, bar) = tick_app(true, 0.0);
        let world = app.world_mut();
        let children = world.entity(bar).get::<Children>().unwrap();
        let fill = children[0];
        let node = world.entity(fill).get::<Node>().unwrap();
        assert_eq!(node.width, Val::Percent(50.0));
    }
}
