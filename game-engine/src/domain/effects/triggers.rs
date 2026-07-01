//! Wires the three skill domain events the network layer produces into effect
//! playback: spawn the STR effect at the right anchor, play the caster's attack
//! motion, play the per-effect sound, and (for damage) emit the existing
//! `DisplayDamageNumber`.
//!
//! Gameplay feedback (the target's damage number and the caster's attack motion)
//! plays for every skill, independent of the catalog: most skills have no `.str`
//! entry, and gating feedback on one would leave e.g. Bash with no number and no
//! swing. Only the STR visual effect and its sound are catalog-gated.
//!
//! Effects are non-critical (design D6): the visual portion early-returns on a
//! missing entity or missing catalog entry with a `debug!`, never panicking and
//! never inventing defaults.

use bevy::prelude::*;
use lifthrasir_data::{EffectDescriptor, EffectPlacement};
use moonshine_behavior::prelude::BehaviorMut;

use super::components::EffectAnchor;
use super::systems::spawn_effect;
use crate::domain::audio::events::PlaySkillSfx;
use crate::domain::combat::events::{DamageDisplayType, DisplayDamageNumber};
use crate::domain::combat::systems::start_attack_animation;
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::components::NetworkEntity;
use crate::infrastructure::effect::{EffectCatalog, LoadedEffectAsset};
use crate::utils::coordinates::spawn_coords_to_world_position;
use net_contract::events::{GroundSkillPlaced, SkillDamageReceived, SkillEffectShown};

/// Despawn timer for repeating ground effects (aesir sends no removal packet;
/// design §4 "Lifetime boundary"). A `RemoveGroundSkill` event would supersede.
const GROUND_EFFECT_LIFETIME_SECS: f32 = 8.0;

/// Resolve a unit by the gid aesir keys in-game packets on (see
/// `combat/systems.rs`). `None` when the unit is not in the world.
fn resolve_gid(network_entities: &Query<(Entity, &NetworkEntity)>, gid: u32) -> Option<Entity> {
    network_entities
        .iter()
        .find(|(_, ne)| ne.gid == gid)
        .map(|(e, _)| e)
}

/// The descriptor's RGBA tint as a Bevy `Color` (the data crate stays Bevy-free).
pub(crate) fn descriptor_tint(descriptor: &EffectDescriptor) -> Color {
    let [r, g, b, a] = descriptor.color;
    Color::srgba(r, g, b, a)
}

/// Load the descriptor's STR effect through the registered `.str` loader.
/// `None` for sound-only descriptors (no `str`), which spawn no visual.
pub(crate) fn load_effect(
    asset_server: &AssetServer,
    descriptor: &EffectDescriptor,
) -> Option<Handle<LoadedEffectAsset>> {
    descriptor
        .str
        .as_ref()
        .map(|name| asset_server.load(format!("ro://data/texture/effect/{}", name)))
}

/// Spawn the descriptor's STR effect when it has one, returning the entity the
/// sound should anchor to: the spawned effect if present, otherwise `fallback`
/// (sound-only skills like Bash anchor their sound to the fallback unit).
fn spawn_str_or_fallback(
    commands: &mut Commands,
    asset_server: &AssetServer,
    descriptor: &EffectDescriptor,
    anchor: EffectAnchor,
    fallback: Entity,
    lifetime: Option<Timer>,
) -> Entity {
    match load_effect(asset_server, descriptor) {
        Some(effect) => spawn_effect(
            commands,
            effect,
            anchor,
            descriptor.repeating,
            descriptor_tint(descriptor),
            lifetime,
        ),
        None => fallback,
    }
}

/// Play the descriptor's sound (if any) anchored to `emitter`.
fn play_sound(
    sfx: &mut MessageWriter<PlaySkillSfx>,
    descriptor: &EffectDescriptor,
    emitter: Entity,
) {
    if let Some(sound) = &descriptor.sound {
        sfx.write(PlaySkillSfx {
            emitter,
            sound: sound.clone(),
        });
    }
}

/// `SkillEffectShown` — a no-damage skill effect: spawn anchored per placement,
/// play caster motion on the source, play the sound.
#[allow(clippy::too_many_arguments)]
pub fn on_skill_effect(
    mut events: MessageReader<SkillEffectShown>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    network_entities: Query<(Entity, &NetworkEntity)>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&network_entities, event.src_id);
        let target = resolve_gid(&network_entities, event.target_id);

        if let Some(src) = src {
            start_attack_animation(&mut commands, &mut behaviors, &transforms, src, target, 0);
        }

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.skill_id)) else {
            debug!("No effect catalog entry for skill {}", event.skill_id);
            continue;
        };

        let anchor_entity = match descriptor.placement {
            EffectPlacement::Caster => src,
            // Ground placement is not expected from this packet; fall back to the
            // target unit so the effect still anchors sensibly.
            EffectPlacement::Target | EffectPlacement::Ground => target,
        };

        let Some(anchor_entity) = anchor_entity else {
            debug!(
                "No entity for skill effect {} (src {}, target {})",
                event.skill_id, event.src_id, event.target_id
            );
            continue;
        };

        let emitter = spawn_str_or_fallback(
            &mut commands,
            &asset_server,
            descriptor,
            EffectAnchor::Entity(anchor_entity),
            anchor_entity,
            None,
        );

        play_sound(&mut sfx, descriptor, emitter);
    }
}

/// `SkillDamageReceived` — like `on_skill_effect`, plus the existing
/// `DisplayDamageNumber` for the target.
#[allow(clippy::too_many_arguments)]
pub fn on_skill_damage(
    mut events: MessageReader<SkillDamageReceived>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    network_entities: Query<(Entity, &NetworkEntity)>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut damage_display: MessageWriter<DisplayDamageNumber>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&network_entities, event.src_id);
        let Some(target) = resolve_gid(&network_entities, event.target_id) else {
            debug!(
                "No target entity for skill damage {} (target {})",
                event.skill_id, event.target_id
            );
            continue;
        };

        // Damage number and caster motion are gameplay feedback, not part of the
        // STR visual effect: they play for every damage skill, including ones with
        // no catalog entry (e.g. Bash).
        damage_display.write(DisplayDamageNumber {
            entity: target,
            amount: event.damage.max(0),
            damage_type: DamageDisplayType::Normal,
        });

        if let Some(src) = src {
            start_attack_animation(
                &mut commands,
                &mut behaviors,
                &transforms,
                src,
                Some(target),
                event.src_delay as i32,
            );
        }

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.skill_id)) else {
            debug!("No effect catalog entry for skill {}", event.skill_id);
            continue;
        };

        let emitter = spawn_str_or_fallback(
            &mut commands,
            &asset_server,
            descriptor,
            EffectAnchor::Entity(target),
            target,
            None,
        );

        play_sound(&mut sfx, descriptor, emitter);
    }
}

/// `GroundSkillPlaced` — spawn a position-anchored effect at the converted cell,
/// play caster motion on the source, play the sound.
#[allow(clippy::too_many_arguments)]
pub fn on_ground_skill(
    mut events: MessageReader<GroundSkillPlaced>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    network_entities: Query<(Entity, &NetworkEntity)>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&network_entities, event.src_id);
        if let Some(src) = src {
            start_attack_animation(&mut commands, &mut behaviors, &transforms, src, None, 0);
        }

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.skill_id)) else {
            debug!(
                "No effect catalog entry for ground skill {}",
                event.skill_id
            );
            continue;
        };

        let position = spawn_coords_to_world_position(event.x as u16, event.y as u16, 0, 0);

        let lifetime = descriptor
            .repeating
            .then(|| Timer::from_seconds(GROUND_EFFECT_LIFETIME_SECS, TimerMode::Once));

        // A sound-only ground skill (no `str`) has no spawned effect to anchor to,
        // so its sound anchors to the caster if present.
        let emitter = match load_effect(&asset_server, descriptor) {
            Some(effect) => Some(spawn_effect(
                &mut commands,
                effect,
                EffectAnchor::Position(position),
                descriptor.repeating,
                descriptor_tint(descriptor),
                lifetime,
            )),
            None => src,
        };

        if let Some(emitter) = emitter {
            play_sound(&mut sfx, descriptor, emitter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::components::ActiveEffect;
    use crate::domain::entities::types::ObjectType;
    use crate::infrastructure::effect::SkillEffectDataAsset;

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/skill_effects.ron");
        let asset = ron::from_str::<SkillEffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::from_skill_effect_data(asset.0)
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<LoadedEffectAsset>()
            .add_message::<SkillEffectShown>()
            .add_message::<SkillDamageReceived>()
            .add_message::<GroundSkillPlaced>()
            .add_message::<DisplayDamageNumber>()
            .add_message::<PlaySkillSfx>()
            .insert_resource(seeded_catalog());
        app
    }

    fn spawn_unit(app: &mut App, gid: u32) -> Entity {
        app.world_mut()
            .spawn((
                NetworkEntity::new(gid, gid, ObjectType::Pc),
                Transform::default(),
            ))
            .id()
    }

    fn active_effects(app: &mut App) -> usize {
        app.world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count()
    }

    fn position_anchored(app: &mut App) -> Vec<Vec3> {
        app.world_mut()
            .query::<&EffectAnchor>()
            .iter(app.world())
            .filter_map(|anchor| match anchor {
                EffectAnchor::Position(p) => Some(*p),
                EffectAnchor::Entity(_) => None,
            })
            .collect()
    }

    #[test]
    fn skill_damage_spawns_effect_and_emits_damage_number() {
        let mut app = test_app();
        app.add_systems(Update, on_skill_damage);

        let target = spawn_unit(&mut app, 200);
        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(SkillDamageReceived {
            skill_id: 28, // AL_HEAL (seeded Target)
            level: 1,
            src_id: 100,
            target_id: 200,
            server_tick: 0,
            damage: 123,
            div: 1,
            type_: 0,
            src_delay: 200,
            dst_delay: 100,
        });

        app.update();

        assert_eq!(active_effects(&mut app), 1, "one effect instance spawned");

        let messages = app
            .world_mut()
            .resource_mut::<Messages<DisplayDamageNumber>>();
        let mut cursor = messages.get_cursor();
        let emitted: Vec<_> = cursor.read(&messages).collect();
        assert_eq!(emitted.len(), 1, "one damage number emitted");
        assert_eq!(emitted[0].entity, target);
        assert_eq!(emitted[0].amount, 123);
    }

    #[test]
    fn ground_skill_spawns_position_anchored_effect_at_cell() {
        let mut app = test_app();
        app.add_systems(Update, on_ground_skill);

        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(GroundSkillPlaced {
            skill_id: 89, // WZ_STORMGUST (seeded Ground)
            src_id: 100,
            level: 10,
            x: 40,
            y: 50,
            server_tick: 0,
        });

        app.update();

        let positions = position_anchored(&mut app);
        assert_eq!(positions.len(), 1, "one position-anchored ground effect");
        assert_eq!(positions[0], spawn_coords_to_world_position(40, 50, 0, 0));
    }

    #[test]
    fn unknown_skill_id_shows_damage_without_effect() {
        let mut app = test_app();
        app.add_systems(Update, on_skill_damage);

        let target = spawn_unit(&mut app, 200);
        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(SkillDamageReceived {
            skill_id: 999_999, // not in the catalog (e.g. Bash has no STR effect)
            level: 1,
            src_id: 100,
            target_id: 200,
            server_tick: 0,
            damage: 50,
            div: 1,
            type_: 0,
            src_delay: 0,
            dst_delay: 0,
        });

        app.update();

        assert_eq!(
            active_effects(&mut app),
            0,
            "no STR effect for unknown skill"
        );

        let messages = app
            .world_mut()
            .resource_mut::<Messages<DisplayDamageNumber>>();
        let mut cursor = messages.get_cursor();
        let emitted: Vec<_> = cursor.read(&messages).collect();
        assert_eq!(emitted.len(), 1, "damage number still emitted");
        assert_eq!(emitted[0].entity, target);
        assert_eq!(emitted[0].amount, 50);
    }

    #[test]
    fn sound_only_skill_plays_sound_without_spawning_effect() {
        let mut app = test_app();
        app.add_systems(Update, on_skill_damage);

        let target = spawn_unit(&mut app, 200);
        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(SkillDamageReceived {
            skill_id: 5, // SM_BASH — sound-only, no STR effect
            level: 1,
            src_id: 100,
            target_id: 200,
            server_tick: 0,
            damage: 75,
            div: 1,
            type_: 0,
            src_delay: 0,
            dst_delay: 0,
        });

        app.update();

        assert_eq!(
            active_effects(&mut app),
            0,
            "no STR effect for a sound-only skill"
        );

        let sfx = app.world_mut().resource_mut::<Messages<PlaySkillSfx>>();
        let mut cursor = sfx.get_cursor();
        let emitted: Vec<_> = cursor.read(&sfx).collect();
        assert_eq!(emitted.len(), 1, "sound-only skill still plays its sound");
        assert_eq!(
            emitted[0].emitter, target,
            "sound anchors to the target unit"
        );
    }

    #[test]
    fn unknown_gid_is_a_noop() {
        let mut app = test_app();
        app.add_systems(Update, on_skill_damage);

        // No units spawned: the target gid resolves to nothing.
        app.world_mut().write_message(SkillDamageReceived {
            skill_id: 28,
            level: 1,
            src_id: 100,
            target_id: 200,
            server_tick: 0,
            damage: 50,
            div: 1,
            type_: 0,
            src_delay: 0,
            dst_delay: 0,
        });

        app.update();

        assert_eq!(active_effects(&mut app), 0, "no effect for unknown gid");
    }
}
