//! Health bars over monsters. A bar pops above a monster when a damage number
//! lands on it, tracks the server-authoritative `UnitHealth` and fades out a few
//! seconds after the last hit. The locked attack target keeps its bar for as long
//! as the lock holds, and a boss always shows a wider bar with its health percent.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::LetterSpacing;
use lifthrasir_client::domain::combat::components::DeadEntity;
use lifthrasir_client::domain::combat::events::DisplayDamageNumber;
use lifthrasir_client::domain::entities::components::UnitHealth;
use lifthrasir_client::domain::entities::markers::{Boss, Mob};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;

use crate::screens::shared::outlined_text;
use crate::worldspace::{
    LockedUnit, WorldAnchor, WorldOverlaySystems, fill_width, raised_root, world_bar,
};

const BAR_SIZE: Vec2 = Vec2::new(44.0, 6.0);
const BOSS_BAR_SIZE: Vec2 = Vec2::new(110.0, 8.0);
/// Wider than the boss bar and its label, so both centre on the monster.
const ROOT_WIDTH: f32 = 160.0;
/// Pixels from the monster's origin up to the bar's bottom edge, clearing the head.
const BAR_HEAD_GAP: f32 = 68.0;
/// How long a bar lingers after the last hit before it is gone.
const LINGER_SECS: f32 = 5.0;
/// Fade-out window at the end of the linger.
const FADE_SECS: f32 = 0.8;
/// Above nameplates (+100), below damage numbers (+150).
const BAR_Z: i32 = super::WORLD_OVERLAY_Z + 120;
const HP_TOP: Color = Color::srgb_u8(0xd0, 0x54, 0x4a);
const HP_BOTTOM: Color = Color::srgb_u8(0x8e, 0x2f, 0x28);

pub struct MobHealthBarPlugin;

impl Plugin for MobHealthBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (engage_bars, tick_bars)
                .chain()
                .in_set(WorldOverlaySystems::Sync),
        );
    }
}

/// A monster's bar; its [`WorldAnchor`] names the monster.
#[derive(Component)]
struct MobHealthBar {
    linger: Timer,
}

/// The bar's trough, which fades with the bar back from its `ring` colour.
#[derive(Component, Default, Clone)]
struct MobBarTrough {
    ring: Color,
}

#[derive(Component, Default, Clone)]
struct MobBarFill;

/// A boss bar's health percent.
#[derive(Component, Default, Clone)]
struct MobBarLabel;

/// Alpha multiplier for the fade-out window at the end of the linger.
fn fade_alpha(remaining_secs: f32) -> f32 {
    (remaining_secs / FADE_SECS).clamp(0.0, 1.0)
}

fn hp_gradient(alpha: f32) -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(HP_TOP.with_alpha(alpha), percent(0)),
        ColorStop::new(HP_BOTTOM.with_alpha(alpha), percent(100)),
    ]))
}

/// NOTE: the server's boss class covers minibosses too, so the label says BOSS
/// where the design says MVP. Switch once the protocol tells the two apart.
fn boss_label(health: &UnitHealth) -> String {
    let percent = u64::from(health.hp) * 100 / u64::from(health.max_hp.max(1));
    format!("BOSS · {}%", percent.min(100))
}

fn bar(boss: bool) -> impl Scene {
    let (size, ring) = if boss {
        (BOSS_BAR_SIZE, ro_dark::PRIMARY_STROKE)
    } else {
        (BAR_SIZE, Color::BLACK.with_alpha(0.6))
    };
    let spacing = LetterSpacing::Px(1.2);
    let label: Vec<_> = boss
        .then(|| {
            bsn! {
                MobBarLabel
                @outlined_text(String::new(), 10.0, ro_dark::RARE_RING)
                spacing
            }
        })
        .into_iter()
        .collect();
    let trough = MobBarTrough { ring };
    let paint = hp_gradient(1.0);
    let content = bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(2),
        }
        @ignore_picking()
        Children [
            {label}
            --
            trough
            @world_bar(size, ring, bsn! { MobBarFill paint })
        ]
    };
    raised_root(ROOT_WIDTH, BAR_Z, content)
}

/// A monster that can carry a bar: alive, with known health.
type LiveMob = (With<Mob>, With<UnitHealth>, Without<DeadEntity>);

/// Pops a bar for every live monster that was just hit, is the locked target or is
/// a boss, and re-arms the bars those monsters already have.
fn engage_bars(
    mut events: MessageReader<DisplayDamageNumber>,
    mut commands: Commands,
    locked: LockedUnit,
    mobs: Query<(Entity, Has<Boss>), LiveMob>,
    mut bars: Query<(&WorldAnchor, &mut MobHealthBar)>,
    mut engaged: Local<HashSet<Entity>>,
) {
    engaged.clear();
    engaged.extend(events.read().map(|event| event.entity));
    engaged.extend(locked.entity());
    engaged.extend(mobs.iter().filter(|(_, boss)| *boss).map(|(mob, _)| mob));

    for (anchor, mut bar) in &mut bars {
        if engaged.remove(&anchor.target) {
            bar.linger.reset();
        }
    }
    for target in engaged.drain() {
        let Ok((_, boss)) = mobs.get(target) else {
            continue;
        };
        commands.spawn_scene(bar(boss)).insert((
            WorldAnchor {
                target,
                width: ROOT_WIDTH,
                drop: -BAR_HEAD_GAP,
            },
            MobHealthBar {
                linger: Timer::from_seconds(LINGER_SECS, TimerMode::Once),
            },
        ));
    }
}

/// The nodes of a bar that follow its monster's health and its fade.
#[derive(SystemParam)]
struct BarParts<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    troughs: Query<
        'w,
        's,
        (
            &'static MobBarTrough,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
        ),
    >,
    fills: Query<'w, 's, (&'static mut Node, &'static mut BackgroundGradient), With<MobBarFill>>,
    labels: Query<'w, 's, &'static mut Text, With<MobBarLabel>>,
}

impl BarParts<'_, '_> {
    fn paint(&mut self, bar: Entity, health: &UnitHealth, alpha: f32) {
        for part in self.children.iter_descendants(bar) {
            if let Ok((trough, mut fill, mut ring)) = self.troughs.get_mut(part) {
                fill.0 = Color::BLACK.with_alpha(0.7 * alpha);
                *ring = BorderColor::all(trough.ring.with_alpha(trough.ring.alpha() * alpha));
            }
            if let Ok((mut node, mut gradient)) = self.fills.get_mut(part) {
                node.width = fill_width(health.hp.into(), health.max_hp.into());
                *gradient = hp_gradient(alpha);
            }
            if let Ok(mut text) = self.labels.get_mut(part) {
                let label = boss_label(health);
                if text.0 != label {
                    text.0 = label;
                }
            }
        }
    }
}

/// Ticks the linger, drops the bars that ran out or whose monster died, and paints
/// the rest.
fn tick_bars(
    time: Res<Time>,
    mut commands: Commands,
    mut bars: Query<(Entity, &WorldAnchor, &mut MobHealthBar)>,
    units: Query<&UnitHealth, Without<DeadEntity>>,
    mut parts: BarParts,
) {
    for (entity, anchor, mut bar) in &mut bars {
        bar.linger.tick(time.delta());
        let live = units
            .get(anchor.target)
            .ok()
            .filter(|_| !bar.linger.is_finished());
        let Some(health) = live else {
            commands.entity(entity).despawn();
            continue;
        };
        parts.paint(entity, health, fade_alpha(bar.linger.remaining_secs()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::combat::events::DamageDisplayType;
    use lifthrasir_client::domain::entities::EntityRegistry;
    use lifthrasir_client::domain::input::LockedTarget;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        app.add_message::<DisplayDamageNumber>()
            .init_resource::<LockedTarget>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, (engage_bars, tick_bars).chain());
        app
    }

    fn mob(app: &mut App, hp: u32) -> Entity {
        app.world_mut()
            .spawn((Mob, UnitHealth { hp, max_hp: 60 }))
            .id()
    }

    fn hit(app: &mut App, entity: Entity) {
        app.world_mut().write_message(DisplayDamageNumber {
            entity,
            amount: 10,
            damage_type: DamageDisplayType::Normal,
            delay_secs: 0.0,
        });
    }

    fn barred(app: &mut App) -> Vec<Entity> {
        let world = app.world_mut();
        world
            .query_filtered::<&WorldAnchor, With<MobHealthBar>>()
            .iter(world)
            .map(|anchor| anchor.target)
            .collect()
    }

    /// Runs the bars' timers past `secs`.
    fn age(app: &mut App, secs: f32) {
        let world = app.world_mut();
        for mut bar in world.query::<&mut MobHealthBar>().iter_mut(world) {
            bar.linger.set_elapsed(Duration::from_secs_f32(secs));
        }
        app.update();
    }

    #[test]
    fn fade_alpha_ramps_only_at_the_end() {
        assert_eq!(fade_alpha(LINGER_SECS), 1.0);
        assert_eq!(fade_alpha(FADE_SECS), 1.0);
        assert!(fade_alpha(FADE_SECS / 2.0) < 1.0);
        assert_eq!(fade_alpha(0.0), 0.0);
    }

    #[test]
    fn a_hit_pops_one_bar_that_tracks_health_and_runs_out() {
        let mut app = app();
        let poring = mob(&mut app, 30);
        mob(&mut app, 60);
        hit(&mut app, poring);
        hit(&mut app, poring);

        app.update();
        app.update();

        assert_eq!(barred(&mut app), [poring]);
        let world = app.world_mut();
        let fill = world
            .query_filtered::<&Node, With<MobBarFill>>()
            .single(world)
            .unwrap();
        assert_eq!(fill.width, Val::Percent(50.0));

        age(&mut app, LINGER_SECS);
        assert!(barred(&mut app).is_empty());
    }

    #[test]
    fn the_locked_target_keeps_its_bar_past_the_linger() {
        let mut app = app();
        let poring = mob(&mut app, 30);
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(7, poring);
        app.world_mut().resource_mut::<LockedTarget>().gid = Some(7);
        app.update();

        age(&mut app, LINGER_SECS);
        assert_eq!(barred(&mut app), [poring]);

        app.world_mut().resource_mut::<LockedTarget>().gid = None;
        age(&mut app, LINGER_SECS);
        assert!(barred(&mut app).is_empty());
    }

    #[test]
    fn a_boss_always_shows_its_bar_with_a_percent_until_it_dies() {
        let mut app = app();
        let lord = mob(&mut app, 45);
        app.world_mut().entity_mut(lord).insert(Boss);

        app.update();
        app.update();

        assert_eq!(barred(&mut app), [lord]);
        let world = app.world_mut();
        let label = world
            .query_filtered::<&Text, With<MobBarLabel>>()
            .single(world)
            .unwrap();
        assert_eq!(label.0, "BOSS · 75%");

        app.world_mut().entity_mut(lord).insert(DeadEntity);
        app.update();
        assert!(barred(&mut app).is_empty());
    }

    #[test]
    fn a_hit_on_something_that_is_no_monster_pops_nothing() {
        let mut app = app();
        let player = app.world_mut().spawn(UnitHealth { hp: 1, max_hp: 2 }).id();
        hit(&mut app, player);

        app.update();

        assert!(barred(&mut app).is_empty());
    }
}
