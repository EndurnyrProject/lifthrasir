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
use super::events::PlayProceduralVfx;
use super::systems::spawn_effect;
use crate::domain::audio::events::PlaySkillSfx;
use crate::domain::combat::events::{DamageDisplayType, DisplayDamageNumber};
use crate::domain::combat::systems::start_attack_animation;
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::registry::EntityRegistry;
use crate::infrastructure::effect::{EffectCatalog, LoadedEffectAsset, MapEffectCatalog};
use crate::utils::coordinates::spawn_coords_to_world_position;
use net_contract::events::{
    GroundSkillPlaced, SkillDamageReceived, SkillEffectShown, SpecialEffectShown,
};

/// Despawn timer for repeating `SpecialEffect` visuals: `SpecialEffect` is
/// fire-and-forget with no removal packet, so a `repeating` catalog entry
/// (e.g. EF_STORMGUST, EF_MAGNUS) would otherwise never set `finished` and
/// accumulate one entity per occurrence.
const SPECIAL_EFFECT_LIFETIME_SECS: f32 = 8.0;

/// Vertical offset from a unit's `Transform.translation` (its feet) to where a
/// procedural VFX anchors. Up is `-Y` in this world, so the offset is negative to
/// lift the burst off the ground and read it over the target's body rather than
/// clipping into the terrain.
const VFX_CENTER_HEIGHT: f32 = -2.0;

/// Write a `PlayProceduralVfx` for a descriptor's procedural key, anchored to the
/// resolved unit's body center. Non-critical (design §D6): skips with a `debug!`
/// when the unit has no transform, never inventing a default position.
fn emit_procedural_vfx(
    proc_vfx: &mut MessageWriter<PlayProceduralVfx>,
    transforms: &Query<&Transform>,
    descriptor: &EffectDescriptor,
    anchor: Entity,
) {
    let Some(key) = &descriptor.vfx else {
        return;
    };
    let Ok(transform) = transforms.get(anchor) else {
        debug!("No transform for procedural vfx anchor {anchor}");
        return;
    };
    proc_vfx.write(PlayProceduralVfx {
        key: key.clone(),
        position: transform.translation + Vec3::new(0.0, VFX_CENTER_HEIGHT, 0.0),
        color: descriptor_tint(descriptor),
    });
}

/// Resolve a unit by the gid aesir keys in-game packets on (see
/// `combat/systems.rs`). `None` when the unit is not registered.
fn resolve_gid(registry: &EntityRegistry, gid: u32) -> Option<Entity> {
    registry.get_entity(gid)
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
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut sfx: MessageWriter<PlaySkillSfx>,
    mut proc_vfx: MessageWriter<PlayProceduralVfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&registry, event.src_id);
        let target = resolve_gid(&registry, event.target_id);

        if let Some(src) = src {
            start_attack_animation(&mut commands, &mut behaviors, &transforms, src, target, 0);
        }

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.skill_id)) else {
            warn!("No effect catalog entry for skill {}", event.skill_id);
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
        emit_procedural_vfx(&mut proc_vfx, &transforms, descriptor, anchor_entity);
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
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut damage_display: MessageWriter<DisplayDamageNumber>,
    mut sfx: MessageWriter<PlaySkillSfx>,
    mut proc_vfx: MessageWriter<PlayProceduralVfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&registry, event.src_id);
        let Some(target) = resolve_gid(&registry, event.target_id) else {
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
            warn!("No effect catalog entry for skill {}", event.skill_id);
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
        emit_procedural_vfx(&mut proc_vfx, &transforms, descriptor, target);
    }
}

/// `GroundSkillPlaced` — cast-moment feedback only: caster motion, the landing
/// sound, and (for non-repeating descriptors) a one-shot STR at the converted
/// cell. Repeating descriptors (e.g. Storm Gust) spawn no visual here — their
/// persistent effect belongs to the skill-unit group/cell entities
/// (`domain/skill_units`), which own the whole lifetime and never rely on a
/// client-side despawn timer.
#[allow(clippy::too_many_arguments)]
pub fn on_ground_skill(
    mut events: MessageReader<GroundSkillPlaced>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    transforms: Query<&Transform>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        let src = resolve_gid(&registry, event.src_id);
        if let Some(src) = src {
            start_attack_animation(&mut commands, &mut behaviors, &transforms, src, None, 0);
        }

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.skill_id)) else {
            warn!(
                "No effect catalog entry for ground skill {}",
                event.skill_id
            );
            continue;
        };

        // A sound-only ground skill (no `str`), or a repeating one whose visual
        // now belongs to the skill unit, has no spawned effect to anchor to, so
        // its sound anchors to the caster if present.
        let emitter = if descriptor.repeating {
            src
        } else {
            let position = spawn_coords_to_world_position(event.x as u16, event.y as u16, 0, 0);
            match load_effect(&asset_server, descriptor) {
                Some(effect) => Some(spawn_effect(
                    &mut commands,
                    effect,
                    EffectAnchor::Position(position),
                    false,
                    descriptor_tint(descriptor),
                    None,
                )),
                None => src,
            }
        };

        if let Some(emitter) = emitter {
            play_sound(&mut sfx, descriptor, emitter);
        }
    }
}

/// `SpecialEffectShown` — a fire-and-forget visual effect keyed by an rAthena
/// `EF_*` id, spawned at the source unit's position via the same catalog map
/// effects use. Non-critical (design D6): `debug!` + skip on an unresolved
/// source, missing transform, or unmapped effect id.
pub fn on_special_effect(
    mut events: MessageReader<SpecialEffectShown>,
    mut commands: Commands,
    catalog: Option<Res<MapEffectCatalog>>,
    asset_server: Res<AssetServer>,
    registry: Res<EntityRegistry>,
    transforms: Query<&Transform>,
) {
    for event in events.read() {
        let Some(source) = resolve_gid(&registry, event.source_id) else {
            debug!("No entity for special effect source {}", event.source_id);
            continue;
        };

        let Ok(transform) = transforms.get(source) else {
            debug!("No transform for special effect source {source}");
            continue;
        };

        let Some(descriptor) = catalog.as_ref().and_then(|c| c.get(event.effect_id)) else {
            debug!("No map effect catalog entry for effect {}", event.effect_id);
            continue;
        };

        // ponytail: vfx-only descriptors (e.g. EF_SMOKE, EF_EMITTER) are
        // intentionally unhandled here — a looping ambient vfx doesn't fit a
        // fire-and-forget SpecialEffect, and the MapAmbientVfx bridge
        // `spawn_map_effects` uses is out of scope for this event. Add if a
        // vfx-only EF_* id needs to render from SpecialEffect.
        let Some(effect) = load_effect(&asset_server, descriptor) else {
            debug!("Special effect {} has no STR; skipping", event.effect_id);
            continue;
        };

        let lifetime = descriptor
            .repeating
            .then(|| Timer::from_seconds(SPECIAL_EFFECT_LIFETIME_SECS, TimerMode::Once));

        let position = transform.translation + Vec3::new(0.0, VFX_CENTER_HEIGHT, 0.0);
        spawn_effect(
            &mut commands,
            effect,
            EffectAnchor::Position(position),
            descriptor.repeating,
            descriptor_tint(descriptor),
            lifetime,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::components::ActiveEffect;
    use crate::domain::effects::systems::despawn_finished_effects;
    use crate::domain::entities::components::NetworkEntity;
    use crate::domain::entities::types::ObjectType;
    use crate::infrastructure::effect::EffectDataAsset;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::from_skill_effect_data(asset.0.skills)
    }

    fn seeded_map_catalog() -> MapEffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("seed RON");
        MapEffectCatalog::from_effect_data(asset.0.map)
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
            .add_message::<PlayProceduralVfx>()
            .add_message::<SpecialEffectShown>()
            .init_resource::<EntityRegistry>()
            .insert_resource(seeded_catalog());
        app
    }

    /// Spawns a `NetworkEntity` unit and registers it in `EntityRegistry`,
    /// mirroring how `spawning/systems.rs` and `skill_units` register real
    /// entities — `resolve_gid` looks units up through the registry, not by
    /// scanning `NetworkEntity` components.
    fn spawn_unit(app: &mut App, gid: u32) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                NetworkEntity::new(gid, gid, ObjectType::Pc),
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
        entity
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
    fn ground_skill_repeating_descriptor_spawns_no_effect() {
        let mut app = test_app();
        app.add_systems(Update, on_ground_skill);

        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(GroundSkillPlaced {
            skill_id: 89, // WZ_STORMGUST (seeded Ground, repeating: true)
            src_id: 100,
            level: 10,
            x: 40,
            y: 50,
            server_tick: 0,
        });

        app.update();

        assert_eq!(
            active_effects(&mut app),
            0,
            "repeating ground visuals belong to the skill-unit entity, not the trigger"
        );
    }

    #[test]
    fn ground_skill_non_repeating_descriptor_spawns_landing_effect_at_cell() {
        let mut app = test_app();
        // No non-repeating Ground skill is seeded in effects.ron today, so this
        // fabricates one to exercise the landing-flash path (e.g. Thunder Storm's
        // strike).
        app.insert_resource(EffectCatalog::from_skill_effect_data(
            std::collections::BTreeMap::from([(
                900_001,
                EffectDescriptor {
                    str: Some("stonecurse.str".to_string()),
                    placement: EffectPlacement::Ground,
                    color: [1.0, 1.0, 1.0, 1.0],
                    repeating: false,
                    ..Default::default()
                },
            )]),
        ));
        app.add_systems(Update, on_ground_skill);

        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(GroundSkillPlaced {
            skill_id: 900_001,
            src_id: 100,
            level: 10,
            x: 40,
            y: 50,
            server_tick: 0,
        });

        app.update();

        let positions = position_anchored(&mut app);
        assert_eq!(positions.len(), 1, "one landing effect spawned at the cell");
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
    fn procedural_skill_emits_vfx_without_spawning_effect() {
        let mut app = test_app();
        app.add_systems(Update, on_skill_damage);

        let target = spawn_unit(&mut app, 200);
        let target_pos = Vec3::new(3.0, 0.0, 7.0);
        app.world_mut()
            .entity_mut(target)
            .insert(Transform::from_translation(target_pos));
        let _src = spawn_unit(&mut app, 100);

        app.world_mut().write_message(SkillDamageReceived {
            skill_id: 5, // SM_BASH — procedural vfx "bash", no STR effect
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
            "procedural skill spawns no STR effect"
        );

        let vfx = app
            .world_mut()
            .resource_mut::<Messages<PlayProceduralVfx>>();
        let mut cursor = vfx.get_cursor();
        let emitted: Vec<_> = cursor.read(&vfx).collect();
        assert_eq!(emitted.len(), 1, "one procedural vfx emitted");
        assert_eq!(emitted[0].key, "bash");
        assert_eq!(
            emitted[0].position,
            target_pos + Vec3::new(0.0, VFX_CENTER_HEIGHT, 0.0)
        );

        let damage = app
            .world_mut()
            .resource_mut::<Messages<DisplayDamageNumber>>();
        let mut damage_cursor = damage.get_cursor();
        assert_eq!(
            damage_cursor.read(&damage).count(),
            1,
            "damage number still emitted"
        );

        let sfx = app.world_mut().resource_mut::<Messages<PlaySkillSfx>>();
        let mut sfx_cursor = sfx.get_cursor();
        assert_eq!(
            sfx_cursor.read(&sfx).count(),
            1,
            "sound still played for the procedural skill"
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

    #[test]
    fn special_effect_spawns_at_source_position() {
        let mut app = test_app();
        app.insert_resource(seeded_map_catalog());
        app.add_systems(Update, on_special_effect);

        let source_pos = Vec3::new(5.0, 0.0, 9.0);
        let source = spawn_unit(&mut app, 100);
        app.world_mut()
            .entity_mut(source)
            .insert(Transform::from_translation(source_pos));

        app.world_mut().write_message(SpecialEffectShown {
            source_id: 100,
            effect_id: 89, // EF_STORMGUST (seeded map entry)
        });

        app.update();

        let positions = position_anchored(&mut app);
        assert_eq!(positions.len(), 1, "one position-anchored effect spawned");
        assert_eq!(
            positions[0],
            source_pos + Vec3::new(0.0, VFX_CENTER_HEIGHT, 0.0)
        );
    }

    #[test]
    fn special_effect_repeating_effect_despawns_after_lifetime() {
        let mut app = test_app();
        app.insert_resource(seeded_map_catalog()).add_systems(
            Update,
            (on_special_effect, despawn_finished_effects).chain(),
        );

        let source = spawn_unit(&mut app, 100);
        app.world_mut()
            .entity_mut(source)
            .insert(Transform::default());

        app.world_mut().write_message(SpecialEffectShown {
            source_id: 100,
            effect_id: 89, // EF_STORMGUST, repeating: true
        });

        // Warm-up: zero-delta update establishes the Time baseline and spawns
        // the effect (mirrors `systems.rs`'s `warm_up`).
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();

        assert_eq!(
            active_effects(&mut app),
            1,
            "effect spawned before its lifetime expires"
        );

        // Advance past SPECIAL_EFFECT_LIFETIME_SECS in sub-max_delta steps
        // (mirrors `systems.rs`'s `advance`, staying under Time<Virtual>'s
        // default 0.25s max_delta clamp per step).
        let mut remaining = SPECIAL_EFFECT_LIFETIME_SECS + 0.5;
        while remaining > 0.0 {
            let dt = remaining.min(0.2);
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                dt,
            )));
            app.update();
            remaining -= dt;
        }

        assert_eq!(
            active_effects(&mut app),
            0,
            "repeating effect despawns once its lifetime expires"
        );
    }

    #[test]
    fn special_effect_unknown_effect_id_is_noop() {
        let mut app = test_app();
        app.insert_resource(seeded_map_catalog());
        app.add_systems(Update, on_special_effect);

        let source = spawn_unit(&mut app, 100);
        app.world_mut()
            .entity_mut(source)
            .insert(Transform::default());

        app.world_mut().write_message(SpecialEffectShown {
            source_id: 100,
            effect_id: 999_999, // not in the map catalog
        });

        app.update();

        assert_eq!(
            active_effects(&mut app),
            0,
            "no effect for unknown effect id"
        );
    }

    #[test]
    fn special_effect_unresolved_source_is_noop() {
        let mut app = test_app();
        app.insert_resource(seeded_map_catalog());
        app.add_systems(Update, on_special_effect);

        // No units spawned: source_id resolves to nothing.
        app.world_mut().write_message(SpecialEffectShown {
            source_id: 100,
            effect_id: 89,
        });

        app.update();

        assert_eq!(
            active_effects(&mut app),
            0,
            "no effect for unresolved source"
        );
    }
}
