//! Skill-cast bubbles: a frost-ringed pill above a caster's head naming the skill
//! being used. For timed casts it is spawned on `SkillCastStarted`, lives for
//! `cast_time` and carries a progress bar. Instant skills send no casting packet,
//! so for those a brief bubble is shown on execution (`SkillDamageReceived` /
//! `SkillEffectShown`). Either way the server `src_id` is resolved to a client
//! entity via `EntityRegistry` and the bubble follows the caster's head.

use std::collections::HashSet;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use net_contract::events::{SkillCastStarted, SkillDamageReceived, SkillEffectShown};

use crate::worldspace::{WorldAnchor, WorldOverlaySystems, raised_root, world_bar};

const LABEL_WIDTH: f32 = 260.0;
/// How long the bubble lingers for an instant skill, which has no cast phase.
const INSTANT_LABEL_SECS: f32 = 0.6;
/// Pixels from the caster's origin up to the bubble's bottom edge: over the head
/// and over a monster's health bar.
const LABEL_HEAD_GAP: f32 = 80.0;
/// Above nameplates (+100) so a cast reads over a name.
const LABEL_Z: i32 = super::WORLD_OVERLAY_Z + 160;
const CAST_BAR_SIZE: Vec2 = Vec2::new(84.0, 3.0);
const BUBBLE_FILL: Color = Color::srgba(0.047, 0.055, 0.071, 0.75);
const SKILL_INK: Color = Color::srgb_u8(0xd6, 0xf0, 0xff);

pub struct SkillCastLabelPlugin;

impl Plugin for SkillCastLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_cast_labels,
                spawn_instant_labels,
                expire_cast_labels,
                fill_cast_bars,
            )
                .chain()
                .in_set(WorldOverlaySystems::Sync),
        );
    }
}

/// A caster's bubble, gone when `timer` ends; its [`WorldAnchor`] names the caster.
#[derive(Component)]
struct SkillCastLabel {
    timer: Timer,
}

/// Fill node of a timed cast's progress bar; width tracks the label's timer.
#[derive(Component, Default, Clone)]
struct CastBarFill;

fn skill_name(skill_id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(skill_id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{skill_id}"))
}

fn cast_bubble(name: String, with_bar: bool) -> impl Scene {
    let bar: Vec<_> = with_bar
        .then(|| {
            bsn! {
                @world_bar(
                    CAST_BAR_SIZE,
                    Color::NONE,
                    bsn! { CastBarFill BackgroundColor(ro_dark::FROST_GLOW) }
                )
                Node { margin: {UiRect::top(px(1))} }
            }
        })
        .into_iter()
        .collect();
    let content = bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::new(px(9), px(9), px(3), px(5))},
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor(BUBBLE_FILL)
        BorderColor::all(ro_dark::FROST_GLOW.with_alpha(0.6))
        @ignore_picking()
        Children [
            @chrome_text(name, 11.0, SKILL_INK)
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            {bar}
        ]
    };
    raised_root(LABEL_WIDTH, LABEL_Z, content)
}

fn spawn_label(
    commands: &mut Commands,
    target: Entity,
    name: String,
    cast_time_ms: u32,
    with_bar: bool,
) {
    commands.spawn_scene(cast_bubble(name, with_bar)).insert((
        WorldAnchor {
            target,
            width: LABEL_WIDTH,
            drop: -LABEL_HEAD_GAP,
        },
        SkillCastLabel {
            timer: Timer::from_seconds(cast_time_ms as f32 / 1000.0, TimerMode::Once),
        },
    ));
}

fn spawn_cast_labels(
    mut events: MessageReader<SkillCastStarted>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<SkillCatalog>>,
    existing: Query<(Entity, &WorldAnchor), With<SkillCastLabel>>,
) {
    for event in events.read() {
        if event.cast_time == 0 {
            continue;
        }
        let Some(target) = registry.get_entity(event.src_id) else {
            continue;
        };

        // One label per caster: a new cast replaces the previous one.
        for (label, anchor) in &existing {
            if anchor.target == target {
                commands.entity(label).despawn();
            }
        }

        let name = skill_name(event.skill_id, catalog.as_deref());
        spawn_label(&mut commands, target, name, event.cast_time, true);
    }
}

/// Instant skills send no casting packet, so show a brief label when their effect
/// or damage lands instead. Skips casters that already have a label: a timed
/// cast in progress must not sprout a duplicate on completion.
fn spawn_instant_labels(
    mut damage: MessageReader<SkillDamageReceived>,
    mut effects: MessageReader<SkillEffectShown>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<SkillCatalog>>,
    existing: Query<&WorldAnchor, With<SkillCastLabel>>,
) {
    let mut labelled: HashSet<Entity> = existing.iter().map(|anchor| anchor.target).collect();
    let uses = damage
        .read()
        .map(|d| (d.src_id, d.skill_id))
        .chain(effects.read().map(|e| (e.src_id, e.skill_id)));

    for (src_id, skill_id) in uses {
        let Some(target) = registry.get_entity(src_id) else {
            continue;
        };
        if !labelled.insert(target) {
            continue;
        }
        let name = skill_name(skill_id, catalog.as_deref());
        let millis = (INSTANT_LABEL_SECS * 1000.0) as u32;
        spawn_label(&mut commands, target, name, millis, false);
    }
}

/// Expire labels when their cast finishes. Kept separate from positioning so a
/// label still despawns on time even in the frames where the world camera is
/// momentarily absent.
fn expire_cast_labels(
    time: Res<Time>,
    mut labels: Query<(Entity, &mut SkillCastLabel)>,
    mut commands: Commands,
) {
    for (entity, mut label) in &mut labels {
        label.timer.tick(time.delta());
        if label.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Grow each cast bar's fill with its label's timer (the same timer that
/// expires the label, so bar and lifetime can never drift apart).
fn fill_cast_bars(
    labels: Query<&SkillCastLabel>,
    parents: Query<&ChildOf>,
    mut fills: Query<(Entity, &mut Node), With<CastBarFill>>,
) {
    for (entity, mut node) in &mut fills {
        let Some(label) = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| labels.get(ancestor).ok())
        else {
            continue;
        };
        node.width = Val::Percent(label.timer.fraction() * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        app.add_message::<SkillCastStarted>()
            .add_message::<SkillDamageReceived>()
            .add_message::<SkillEffectShown>()
            .init_resource::<EntityRegistry>()
            .add_systems(
                Update,
                (
                    spawn_cast_labels,
                    spawn_instant_labels,
                    expire_cast_labels,
                    fill_cast_bars,
                )
                    .chain(),
            );
        app
    }

    fn caster(app: &mut App, gid: u32) -> Entity {
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, caster);
        caster
    }

    fn label_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&SkillCastLabel>().iter(world).count()
    }

    #[test]
    fn unknown_skill_id_falls_back_to_hash_id() {
        assert_eq!(skill_name(42, None), "#42");
    }

    #[test]
    fn instant_skill_use_spawns_one_label_per_caster() {
        let mut app = app();
        caster(&mut app, 7);

        // Two hits from the same caster in one frame yield a single label.
        for _ in 0..2 {
            app.world_mut().write_message(SkillDamageReceived {
                skill_id: 5,
                level: 1,
                src_id: 7,
                target_id: 9,
                server_tick: 0,
                damage: 100,
                div: 1,
                type_: 0,
                src_delay: 0,
                dst_delay: 0,
                missed: false,
            });
        }
        app.update();

        assert_eq!(label_count(&mut app), 1);
        let world = app.world_mut();
        let fills = world.query::<&CastBarFill>().iter(world).count();
        assert_eq!(
            fills, 0,
            "an instant skill has no cast to show progress for"
        );
    }

    #[test]
    fn a_timed_cast_names_the_skill_over_a_bar_and_replaces_the_last_cast() {
        let mut app = app();
        let mage = caster(&mut app, 7);
        let cast = |skill_id| SkillCastStarted {
            src_id: 7,
            target_id: 0,
            x: 0,
            y: 0,
            skill_id,
            property: 0,
            cast_time: 2_000,
        };

        app.world_mut().write_message(cast(19));
        app.update();
        app.world_mut().write_message(cast(20));
        app.update();

        let world = app.world_mut();
        let anchors: Vec<Entity> = world
            .query_filtered::<&WorldAnchor, With<SkillCastLabel>>()
            .iter(world)
            .map(|anchor| anchor.target)
            .collect();
        assert_eq!(anchors, [mage]);
        let names: Vec<&str> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.as_str())
            .collect();
        assert_eq!(names, ["#20"]);
        assert_eq!(world.query::<&CastBarFill>().iter(world).count(), 1);
    }

    #[test]
    fn cast_bar_fill_tracks_timer_fraction() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, fill_cast_bars);

        let mut timer = Timer::from_seconds(2.0, TimerMode::Once);
        timer.set_elapsed(Duration::from_secs(1));
        let root = app.world_mut().spawn(SkillCastLabel { timer }).id();
        let track = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
        let fill = app
            .world_mut()
            .spawn((Node::default(), CastBarFill, ChildOf(track)))
            .id();

        app.update();

        let width = app.world().get::<Node>(fill).unwrap().width;
        assert_eq!(width, Val::Percent(50.0));
    }

    #[test]
    fn finished_label_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
        timer.set_elapsed(Duration::from_secs(1));
        let label = app.world_mut().spawn(SkillCastLabel { timer }).id();

        app.add_systems(Update, expire_cast_labels);
        app.update();

        assert!(app.world().get_entity(label).is_err());
    }
}
