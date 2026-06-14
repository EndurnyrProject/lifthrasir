//! Floating damage numbers. On each engine `DisplayDamageNumber` we spawn a
//! screen-space text node at the target's projected position, then rise + fade it
//! and despawn on its timer. Position is captured at spawn and animated purely in
//! screen space (RO numbers float free of the entity once they appear), so no
//! per-frame projection or entity dependency is needed after spawn.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::combat::events::{DamageDisplayType, DisplayDamageNumber};

use crate::theme;
use crate::worldspace::WorldspaceFont;

const LIFETIME_SECS: f32 = 0.9;
const RISE_SPEED_PX: f32 = 60.0;
const FONT_SIZE: f32 = 18.0;
const CRIT_FONT_SIZE: f32 = 22.0;
/// Pixels above the entity origin where a number first appears.
const SPAWN_OFFSET_Y: f32 = 30.0;
/// Above nameplates so a number reads over a name; below fade/cursor.
const DAMAGE_Z: i32 = 150;

pub struct DamageNumberPlugin;

impl Plugin for DamageNumberPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DamageSpawnCounter>();
        app.add_systems(
            Update,
            (spawn_damage_numbers, animate_damage_numbers).run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all);
    }
}

#[derive(Component)]
struct DamageNumber {
    timer: Timer,
    /// Current top position in pixels (decreases as it rises).
    top: f32,
}

/// Deterministic horizontal jitter so stacked hits don't perfectly overlap.
#[derive(Resource, Default)]
struct DamageSpawnCounter(u32);

fn damage_text(amount: i32, damage_type: DamageDisplayType) -> String {
    match damage_type {
        DamageDisplayType::Miss => "Miss".to_string(),
        _ => amount.to_string(),
    }
}

fn damage_color(damage_type: DamageDisplayType) -> Color {
    match damage_type {
        DamageDisplayType::Critical => theme::GOLD_YELLOW,
        DamageDisplayType::Miss => theme::POLISHED_STEEL,
        DamageDisplayType::Normal => theme::ASHEN_WHITE,
    }
}

fn font_size(damage_type: DamageDisplayType) -> f32 {
    if damage_type == DamageDisplayType::Critical {
        CRIT_FONT_SIZE
    } else {
        FONT_SIZE
    }
}

/// Spreads stacked numbers horizontally by stepping through a small fixed pattern.
fn horizontal_jitter(counter: u32) -> f32 {
    ((counter % 5) as f32 - 2.0) * 12.0
}

fn spawn_damage_numbers(
    mut events: MessageReader<DisplayDamageNumber>,
    mut commands: Commands,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    targets: Query<&GlobalTransform>,
    font: Res<WorldspaceFont>,
    mut counter: ResMut<DamageSpawnCounter>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for event in events.read() {
        let Ok(target_transform) = targets.get(event.entity) else {
            continue;
        };
        let Ok(screen) = camera.world_to_viewport(camera_transform, target_transform.translation())
        else {
            continue;
        };

        let left = screen.x + horizontal_jitter(counter.0);
        let top = screen.y - SPAWN_OFFSET_Y;
        counter.0 = counter.0.wrapping_add(1);

        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                ..default()
            },
            GlobalZIndex(DAMAGE_Z),
            Pickable::IGNORE,
            DamageNumber {
                timer: Timer::from_seconds(LIFETIME_SECS, TimerMode::Once),
                top,
            },
            children![(
                Text::new(damage_text(event.amount, event.damage_type)),
                TextFont {
                    font: font.0.clone(),
                    font_size: font_size(event.damage_type),
                    ..default()
                },
                TextColor(damage_color(event.damage_type)),
                Pickable::IGNORE,
            )],
        ));
    }
}

fn animate_damage_numbers(
    time: Res<Time>,
    mut commands: Commands,
    mut numbers: Query<(Entity, &mut DamageNumber, &mut Node, Option<&Children>)>,
    mut colors: Query<&mut TextColor>,
) {
    for (entity, mut number, mut node, children) in &mut numbers {
        number.timer.tick(time.delta());
        if number.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        number.top -= RISE_SPEED_PX * time.delta_secs();
        node.top = Val::Px(number.top);

        let alpha = 1.0 - number.timer.fraction();
        if let Some(children) = children {
            for child in children {
                if let Ok(mut color) = colors.get_mut(*child) {
                    color.0.set_alpha(alpha);
                }
            }
        }
    }
}

fn despawn_all(mut commands: Commands, numbers: Query<Entity, With<DamageNumber>>) {
    for entity in &numbers {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn text_and_color_vary_by_type() {
        assert_eq!(damage_text(120, DamageDisplayType::Normal), "120");
        assert_eq!(damage_text(0, DamageDisplayType::Miss), "Miss");
        assert_eq!(
            damage_color(DamageDisplayType::Critical),
            theme::GOLD_YELLOW
        );
        assert_eq!(font_size(DamageDisplayType::Critical), CRIT_FONT_SIZE);
        assert_eq!(font_size(DamageDisplayType::Normal), FONT_SIZE);
    }

    #[test]
    fn jitter_is_bounded_and_deterministic() {
        for counter in 0..20u32 {
            assert!(horizontal_jitter(counter).abs() <= 24.0);
        }
        assert_eq!(horizontal_jitter(0), horizontal_jitter(5));
    }

    #[test]
    fn finished_number_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut timer = Timer::from_seconds(LIFETIME_SECS, TimerMode::Once);
        timer.set_elapsed(Duration::from_secs_f32(LIFETIME_SECS));
        let number = app
            .world_mut()
            .spawn((DamageNumber { timer, top: 100.0 }, Node::default()))
            .id();

        app.add_systems(Update, animate_damage_numbers);
        app.update();

        assert!(app.world().get_entity(number).is_err());
    }
}
