//! Skill-cast labels: a screen-space pill above a caster's head showing the name
//! of the skill being used. For timed casts it is spawned on `SkillCastStarted`
//! and lives for `cast_time` (disappears when the cast finishes). Instant skills
//! send no casting packet, so for those a brief label is shown on execution
//! (`SkillDamageReceived` / `SkillEffectShown`). Either way the server `src_id`
//! is resolved to a client entity via `EntityRegistry` and the pill follows the
//! caster's head each frame.

use std::collections::HashSet;

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::EntityRegistry;
use game_engine::infrastructure::skill::SkillCatalog;
use net_contract::events::{SkillCastStarted, SkillDamageReceived, SkillEffectShown};

use crate::theme;
use crate::worldspace::{viewport_to_ui, WorldCameraFilter, WorldspaceFont};

const LABEL_WIDTH: f32 = 260.0;
const LABEL_FONT_SIZE: f32 = 13.0;
/// How long the label lingers for an instant skill, which has no cast phase.
const INSTANT_LABEL_SECS: f32 = 0.6;
/// Pixels above the caster's projected origin. The sprite origin sits near the
/// body centre, so this clears the head. NOTE: fixed screen offset, not
/// zoom-scaled — tune live via BRP if it drifts off the head.
const LABEL_HEAD_GAP: f32 = 88.0;
/// Above nameplates (100) so a cast reads over a name; below fade/cursor.
const LABEL_Z: i32 = 160;

pub struct SkillCastLabelPlugin;

impl Plugin for SkillCastLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_cast_labels,
                spawn_instant_labels,
                expire_cast_labels,
                follow_cast_labels,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_cast_labels);
    }
}

#[derive(Component)]
struct SkillCastLabel {
    target: Entity,
    timer: Timer,
}

fn skill_name(skill_id: u32, catalog: Option<&SkillCatalog>) -> String {
    catalog
        .and_then(|c| c.get(skill_id))
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| format!("#{skill_id}"))
}

fn spawn_cast_labels(
    mut events: MessageReader<SkillCastStarted>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<SkillCatalog>>,
    font: Res<WorldspaceFont>,
    existing: Query<(Entity, &SkillCastLabel)>,
) {
    for event in events.read() {
        if event.cast_time == 0 {
            continue;
        }
        let Some(target) = registry.get_entity(event.src_id) else {
            continue;
        };

        // One label per caster: a new cast replaces the previous one.
        for (label_entity, label) in &existing {
            if label.target == target {
                commands.entity(label_entity).despawn();
            }
        }

        let name = skill_name(event.skill_id, catalog.as_deref());
        spawn_label(&mut commands, &font, target, &name, event.cast_time);
    }
}

/// Instant skills send no casting packet, so show a brief label when their effect
/// or damage lands instead. Skips casters that already have a label — a timed
/// cast in progress must not sprout a duplicate on completion.
fn spawn_instant_labels(
    mut damage: MessageReader<SkillDamageReceived>,
    mut effects: MessageReader<SkillEffectShown>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<SkillCatalog>>,
    font: Res<WorldspaceFont>,
    existing: Query<&SkillCastLabel>,
) {
    let mut labelled: HashSet<Entity> = existing.iter().map(|label| label.target).collect();
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
        spawn_label(
            &mut commands,
            &font,
            target,
            &name,
            (INSTANT_LABEL_SECS * 1000.0) as u32,
        );
    }
}

fn spawn_label(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    name: &str,
    cast_time_ms: u32,
) {
    commands.spawn((
        // Transparent positioning wrapper: a fixed width centered on the caster
        // keeps the content-sized pill horizontally centered regardless of name length.
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(LABEL_WIDTH),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(LABEL_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
        SkillCastLabel {
            target,
            timer: Timer::from_seconds(cast_time_ms as f32 / 1000.0, TimerMode::Once),
        },
        children![(
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            children![(
                Text::new(name),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: LABEL_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(theme::GOLD),
                Pickable::IGNORE,
            )],
        )],
    ));
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

fn follow_cast_labels(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut labels: Query<(Entity, &SkillCastLabel, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(target_transform) = targets.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - LABEL_WIDTH / 2.0);
                node.top = Val::Px(pos.y - LABEL_HEAD_GAP);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all_cast_labels(mut commands: Commands, labels: Query<Entity, With<SkillCastLabel>>) {
    for entity in &labels {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_skill_id_falls_back_to_hash_id() {
        assert_eq!(skill_name(42, None), "#42");
    }

    #[test]
    fn instant_skill_use_spawns_one_label_per_caster() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<SkillDamageReceived>();
        app.add_message::<SkillEffectShown>();
        app.insert_resource(WorldspaceFont(Handle::default()));

        let caster = app.world_mut().spawn_empty().id();
        let mut registry = EntityRegistry::default();
        registry.register_entity(7, caster);
        app.insert_resource(registry);

        app.add_systems(Update, spawn_instant_labels);

        // Two hits from the same caster in one frame yield a single label.
        let mut damage = app
            .world_mut()
            .resource_mut::<Messages<SkillDamageReceived>>();
        for _ in 0..2 {
            damage.write(SkillDamageReceived {
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
            });
        }
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&SkillCastLabel>().iter(world).count(), 1);
    }

    #[test]
    fn finished_label_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let target = app.world_mut().spawn(GlobalTransform::default()).id();
        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
        timer.set_elapsed(std::time::Duration::from_secs(1));
        let label = app.world_mut().spawn(SkillCastLabel { target, timer }).id();

        app.add_systems(Update, expire_cast_labels);
        app.update();

        assert!(app.world().get_entity(label).is_err());
    }
}
