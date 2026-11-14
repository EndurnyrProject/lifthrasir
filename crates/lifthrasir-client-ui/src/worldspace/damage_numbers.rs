//! Floating damage numbers. On each engine `DisplayDamageNumber` we spawn a
//! screen-space text node at the target's projected position, then pop, rise and
//! fade it and despawn on its timer. Position is captured at spawn and animated
//! purely in screen space (RO numbers float free of the entity once they appear),
//! so no per-frame projection or entity dependency is needed after spawn.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::Val2;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::combat::events::{DamageDisplayType, DisplayDamageNumber};
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::ignore_picking;

use crate::worldspace::{WorldCameraFilter, viewport_to_ui};

const LIFETIME_SECS: f32 = 1.0;
const FONT_SIZE: f32 = 18.0;
const CRIT_FONT_SIZE: f32 = 22.0;
/// Pixels above the entity origin where a number first appears.
const SPAWN_OFFSET_Y: f32 = 30.0;
/// Above nameplates so a number reads over a name.
const DAMAGE_Z: i32 = super::WORLD_OVERLAY_Z + 150;
const CRIT_INK: Color = Color::srgb_u8(0xff, 0xd2, 0x7a);

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
    /// Top position in pixels where the number settles after its pop.
    top: f32,
}

/// Deterministic horizontal jitter so stacked hits don't perfectly overlap.
#[derive(Resource, Default)]
struct DamageSpawnCounter(u32);

/// A damage number waiting out `DisplayDamageNumber::delay_secs` before it
/// appears, for staggering multi-hit skills into a readable sequence.
struct PendingDamageNumber {
    timer: Timer,
    entity: Entity,
    amount: i32,
    damage_type: DamageDisplayType,
}

fn damage_text(amount: i32, damage_type: DamageDisplayType) -> String {
    match damage_type {
        DamageDisplayType::Miss => "Miss".to_string(),
        DamageDisplayType::Critical => format!("{amount}!"),
        DamageDisplayType::Normal => amount.to_string(),
    }
}

fn damage_color(damage_type: DamageDisplayType, player_is_target: bool) -> Color {
    match damage_type {
        DamageDisplayType::Critical => CRIT_INK,
        DamageDisplayType::Miss => ro_dark::MUTED,
        DamageDisplayType::Normal if player_is_target => theme::DAMAGE_RECEIVED,
        DamageDisplayType::Normal => ro_dark::STRONG,
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

/// Where a number is `t` (0 to 1) of the way through its life: it pops in from
/// just below, overshoots its size, then drifts up and fades.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pose {
    /// Pixels below its resting place; negative is above.
    drop: f32,
    scale: f32,
    alpha: f32,
}

fn pose(t: f32) -> Pose {
    let span = |from: f32, to: f32, start: f32, end: f32| {
        from + (to - from) * ((t - start) / (end - start)).clamp(0.0, 1.0)
    };
    if t < 0.15 {
        return Pose {
            drop: span(6.0, 0.0, 0.0, 0.15),
            scale: span(0.8, 1.15, 0.0, 0.15),
            alpha: span(0.0, 1.0, 0.0, 0.15),
        };
    }
    let alpha = span(1.0, 0.0, 0.15, 1.0);
    if t < 0.3 {
        return Pose {
            drop: span(0.0, -4.0, 0.15, 0.3),
            scale: span(1.15, 1.0, 0.15, 0.3),
            alpha,
        };
    }
    Pose {
        drop: span(-4.0, -34.0, 0.3, 1.0),
        scale: 1.0,
        alpha,
    }
}

/// One number, centred on its `left` and starting out invisible.
fn number(text: String, size: f32, color: Color, left: f32) -> impl Scene {
    let centred = UiTransform {
        translation: Val2::percent(-50.0, 0.0),
        ..default()
    };
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(size)},
        }
        TextColor({color.with_alpha(0.0)})
        TextLayout { linebreak: LineBreak::NoWrap }
        TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
        Node { position_type: PositionType::Absolute, left: px(left) }
        centred
        GlobalZIndex(DAMAGE_Z)
        @ignore_picking()
    }
}

/// Where and how a number is put on screen.
#[derive(SystemParam)]
struct Stage<'w, 's> {
    commands: Commands<'w, 's>,
    camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), WorldCameraFilter>,
    targets: Query<'w, 's, (&'static GlobalTransform, Has<LocalPlayer>)>,
    ui_scale: Res<'w, UiScale>,
    counter: ResMut<'w, DamageSpawnCounter>,
}

impl Stage<'_, '_> {
    /// Shows `amount` over `entity`; nothing when it is gone or off screen.
    fn show(&mut self, entity: Entity, amount: i32, damage_type: DamageDisplayType) {
        let Ok((camera, camera_transform)) = self.camera.single() else {
            return;
        };
        let Ok((target, player_is_target)) = self.targets.get(entity) else {
            return;
        };
        let Ok(screen) = camera.world_to_viewport(camera_transform, target.translation()) else {
            return;
        };
        let position = viewport_to_ui(screen, &self.ui_scale);
        let left = position.x + horizontal_jitter(self.counter.0);
        self.counter.0 = self.counter.0.wrapping_add(1);

        self.commands
            .spawn_scene(number(
                damage_text(amount, damage_type),
                font_size(damage_type),
                damage_color(damage_type, player_is_target),
                left,
            ))
            .insert(DamageNumber {
                timer: Timer::from_seconds(LIFETIME_SECS, TimerMode::Once),
                top: position.y - SPAWN_OFFSET_Y,
            });
    }
}

fn spawn_damage_numbers(
    mut events: MessageReader<DisplayDamageNumber>,
    mut stage: Stage,
    time: Res<Time>,
    mut pending: Local<Vec<PendingDamageNumber>>,
) {
    for event in events.read() {
        if event.delay_secs <= 0.0 {
            stage.show(event.entity, event.amount, event.damage_type);
            continue;
        }
        pending.push(PendingDamageNumber {
            timer: Timer::from_seconds(event.delay_secs, TimerMode::Once),
            entity: event.entity,
            amount: event.amount,
            damage_type: event.damage_type,
        });
    }

    let delta = time.delta();
    pending.retain_mut(|hit| {
        hit.timer.tick(delta);
        if !hit.timer.is_finished() {
            return true;
        }
        stage.show(hit.entity, hit.amount, hit.damage_type);
        false
    });
}

fn animate_damage_numbers(
    time: Res<Time>,
    mut commands: Commands,
    mut numbers: Query<(
        Entity,
        &mut DamageNumber,
        &mut Node,
        &mut UiTransform,
        &mut TextColor,
    )>,
) {
    for (entity, mut number, mut node, mut transform, mut color) in &mut numbers {
        number.timer.tick(time.delta());
        if number.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let pose = pose(number.timer.fraction());
        node.top = Val::Px(number.top + pose.drop);
        transform.scale = Vec2::splat(pose.scale);
        color.0.set_alpha(pose.alpha);
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
        assert_eq!(damage_text(120, DamageDisplayType::Critical), "120!");
        assert_eq!(damage_text(0, DamageDisplayType::Miss), "Miss");
        assert_eq!(damage_color(DamageDisplayType::Critical, false), CRIT_INK);
        assert_eq!(
            damage_color(DamageDisplayType::Normal, false),
            ro_dark::STRONG
        );
        assert_eq!(
            damage_color(DamageDisplayType::Normal, true),
            theme::DAMAGE_RECEIVED
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
    fn a_number_pops_in_then_rises_and_fades() {
        let start = pose(0.0);
        assert_eq!((start.drop, start.scale, start.alpha), (6.0, 0.8, 0.0));
        let peak = pose(0.15);
        assert_eq!((peak.drop, peak.alpha), (0.0, 1.0));
        assert!((peak.scale - 1.15).abs() < 1e-5);
        let end = pose(1.0);
        assert_eq!((end.drop, end.scale, end.alpha), (-34.0, 1.0, 0.0));
        let steps: Vec<Pose> = (0..=20).map(|step| pose(step as f32 / 20.0)).collect();
        assert!(steps.windows(2).all(|pair| pair[1].drop <= pair[0].drop));
    }

    #[test]
    fn finished_number_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut timer = Timer::from_seconds(LIFETIME_SECS, TimerMode::Once);
        timer.set_elapsed(Duration::from_secs_f32(LIFETIME_SECS));
        let number = app
            .world_mut()
            .spawn((
                DamageNumber { timer, top: 100.0 },
                Node::default(),
                TextColor::default(),
            ))
            .id();

        app.add_systems(Update, animate_damage_numbers);
        app.update();

        assert!(app.world().get_entity(number).is_err());
    }
}
