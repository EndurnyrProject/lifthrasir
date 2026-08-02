use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::asset::LoadState;
use bevy::prelude::*;
use net_contract::events::{StatusEffectChanged, UnitEntered, UnitStateChanged};

use super::components::EffectAnchor;
use super::sprite_effects::apply_animation_part;
use super::systems::spawn_effect;
use super::triggers::{descriptor_tint, load_effect, load_str_effect};
use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::RenderLayer;
use crate::domain::settings::GraphicsSettings;
use crate::domain::sprite::tags::{
    LAYER_EFFECT, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order,
};
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::infrastructure::effect::EffectCatalog;

/// aesir `OPT1_*` body-state wire ids (`Aesir.ZoneServer.Mmo.Opt1`, the rAthena
/// `e_sc_opt1` table). Single-valued: `UnitStateChanged.body_state` carries at
/// most one of these. Stone Curse's server-side wait phase still reports
/// `:stone`, so both petrify ids render identically.
const OPT1_STONE: u32 = 1;
const OPT1_FREEZE: u32 = 2;
const OPT1_SLEEP: u32 = 4;
const OPT1_STONEWAIT: u32 = 6;

/// Frozen-solid tint (ice blue), multiplied into every sprite layer material.
const ICE_BLUE: Color = Color::srgb(0.5, 0.75, 1.0);
/// Petrified tint (stone gray).
const STONE_GRAY: Color = Color::srgb(0.5, 0.5, 0.5);

/// Colour multiplied into a unit's sprite layers while a body-state pose is
/// active. Read every frame by [`apply_body_state_tint`]; its absence means the
/// layers render at their natural colour.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BodyStateTint(pub Color);

/// Holds a unit's sprite animation on the frame that was showing at `at_ms`.
/// Read by the body layer sync, which feeds this frozen timestamp to the
/// frame-index computation instead of the live clock. Deliberately not an
/// `AnimationState` variant so the behaviour state machine (Hit/Dead) never
/// fights it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AnimationPaused {
    pub at_ms: u32,
}

/// Latest `body_state` for units whose entity has not registered yet, keyed by
/// unit_id. On map load the local player gets a self-directed `UnitStateChange`
/// that can arrive before its entity registers in `EntityRegistry`; without this
/// a frozen local player would render unfrozen. Retried every frame until the
/// unit resolves. Latest value wins, so a freeze-then-clear pair collapses
/// correctly. Mirrors `PendingStatusParams`.
#[derive(Resource, Default)]
pub struct PendingBodyStates(HashMap<u32, u32>);

/// Maps a body-state wire id to its tint, or `None` for states with no visual
/// (0/none, stun, sleep, ...).
fn body_state_tint(body_state: u32) -> Option<Color> {
    match body_state {
        OPT1_FREEZE => Some(ICE_BLUE),
        OPT1_STONE | OPT1_STONEWAIT => Some(STONE_GRAY),
        _ => None,
    }
}

/// Reconciles a unit's freeze/stone visuals with its current `body_state`:
/// insert tint + animation pause when a petrify/freeze pose is active, remove
/// both otherwise. Consumes both channels that carry `body_state` — live
/// `UnitStateChanged` toggles and the spawn-time `UnitEntered` for units that
/// enter view already frozen (no follow-up state change arrives in that case) —
/// so it is ordered after entity spawning to resolve the entered unit.
#[allow(clippy::too_many_arguments)]
pub fn body_state_visuals(
    time: Res<Time>,
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingBodyStates>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children_query: Query<&Children>,
    frozen_overlays: Query<Entity, With<FrozenOverlay>>,
    sleep_overlays: Query<Entity, With<SleepOverlay>>,
) {
    let at_ms = (time.elapsed_secs() * 1000.0) as u32;

    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            pending.0.insert(event.unit_id, event.body_state);
            continue;
        };
        apply_body_state(
            &mut commands,
            entity,
            event.body_state,
            at_ms,
            &asset_server,
            &shared_quad,
            &mut materials,
            &children_query,
            &frozen_overlays,
            &sleep_overlays,
        );
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_body_state(
            &mut commands,
            entity,
            event.body_state,
            at_ms,
            &asset_server,
            &shared_quad,
            &mut materials,
            &children_query,
            &frozen_overlays,
            &sleep_overlays,
        );
    }

    pending.0.retain(|&unit_id, &mut body_state| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_body_state(
            &mut commands,
            entity,
            body_state,
            at_ms,
            &asset_server,
            &shared_quad,
            &mut materials,
            &children_query,
            &frozen_overlays,
            &sleep_overlays,
        );
        false
    });
}

#[allow(clippy::too_many_arguments)]
fn apply_body_state(
    commands: &mut Commands,
    entity: Entity,
    body_state: u32,
    at_ms: u32,
    asset_server: &AssetServer,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
    children_query: &Query<&Children>,
    frozen_overlays: &Query<Entity, With<FrozenOverlay>>,
    sleep_overlays: &Query<Entity, With<SleepOverlay>>,
) {
    if body_state == OPT1_SLEEP {
        spawn_sleep_overlay(
            commands,
            entity,
            asset_server,
            children_query,
            sleep_overlays,
        );
    } else {
        despawn_sleep_overlay(commands, entity, children_query, sleep_overlays);
    }

    let Some(color) = body_state_tint(body_state) else {
        commands
            .entity(entity)
            .remove::<(BodyStateTint, AnimationPaused)>();
        despawn_frozen_overlay(commands, entity, children_query, frozen_overlays);
        return;
    };

    commands
        .entity(entity)
        .insert((BodyStateTint(color), AnimationPaused { at_ms }));
    if body_state == OPT1_FREEZE {
        spawn_frozen_overlay(
            commands,
            entity,
            shared_quad,
            materials,
            children_query,
            frozen_overlays,
        );
        return;
    }

    despawn_frozen_overlay(commands, entity, children_query, frozen_overlays);
}

/// Multiplies each sprite layer's material `base_color` by its parent unit's
/// tint — the combination of its [`BodyStateTint`] (opt1 freeze/stone) and
/// [`Opt3Tint`] (opt3 Fury/Steel Body), multiplied together so both can apply at
/// once — or resets it to white when the unit has neither. This rides the same
/// per-frame path as the layer texture write, because those materials are
/// rewritten unconditionally every frame (retained-phase re-queue) — a one-shot
/// tint write would be lost. Covers every layer uniformly (body, head, weapon,
/// headgear, cart) since they are all `RenderLayer` children of the unit.
pub fn apply_body_state_tint(
    mut materials: ResMut<Assets<StandardMaterial>>,
    layers: Query<(&MeshMaterial3d<StandardMaterial>, &ChildOf), With<RenderLayer>>,
    tints: Query<&BodyStateTint>,
    opt3_tints: Query<&Opt3Tint>,
) {
    for (material_handle, child_of) in &layers {
        let parent = child_of.parent();
        let body = tints.get(parent).ok().map(|t| t.0);
        let opt3 = opt3_tints.get(parent).ok().map(|t| t.0);
        let desired = match (body, opt3) {
            (None, None) => Color::WHITE,
            (Some(c), None) | (None, Some(c)) => c,
            (Some(a), Some(b)) => multiply_colors(a, b),
        };

        // Read before mutating: `get_mut` marks the material changed (a retained-
        // phase re-queue) every call, so touch it only when the colour actually
        // differs. The steady state (untinted white) then costs a read, not a
        // write, keeping this off the per-frame asset-mutation hot path.
        if materials.get(&material_handle.0).map(|m| m.base_color) == Some(desired) {
            continue;
        }

        let Some(mut material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.base_color = desired;
    }
}

/// aesir `OPT3_*` (`virtue`) sprite-recolor bits (`Aesir.ZoneServer.Mmo.Opt3`,
/// the rAthena `e_sc_opt3` table). Unlike `body_state`/opt1 this is a *bitmask*:
/// `UnitStateChanged.virtue` is the bitwise-OR of every active opt3 status on the
/// unit, so more than one bit can be set at once. Only the Monk states that
/// recolour the sprite are handled; Blade Stop (32) is an icon-only pose with no
/// recolour, and the other bits (quicken, overthrust, ...) carry no tint here.
const OPT3_EXPLOSIONSPIRITS: u32 = 8;
const OPT3_STEELBODY: u32 = 16;

/// Fury (Explosion Spirits) tint: an enraged red flush.
const FURY_RED: Color = Color::srgb(1.0, 0.5, 0.42);
/// Mental Strength (Steel Body) tint: the iconic golden metal body.
const STEEL_GOLD: Color = Color::srgb(1.0, 0.8, 0.28);

/// Colour multiplied into a unit's sprite layers for its active opt3 (`virtue`)
/// bits, combined multiplicatively with [`BodyStateTint`] by
/// [`apply_body_state_tint`]. Absent means no opt3 recolour.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Opt3Tint(pub Color);

/// Latest `virtue` bitmask for units whose entity has not registered yet, keyed
/// by unit_id. Mirrors [`PendingBodyStates`] for the opt3 channel: the spawn-time
/// self-sync can arrive before the entity registers.
#[derive(Resource, Default)]
pub struct PendingVirtues(HashMap<u32, u32>);

/// Channel-wise sRGB multiply of two tints (white is the identity), matching how
/// `base_color` multiplies into the sprite texture.
fn multiply_colors(a: Color, b: Color) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgb(a.red * b.red, a.green * b.green, a.blue * b.blue)
}

/// Folds an opt3 (`virtue`) bitmask into a single sprite tint by multiplying the
/// tint of every recognised bit, or `None` when no tint-bearing bit is set (e.g.
/// Blade Stop alone). Fury (8) + Steel Body (16) yields red × gold.
fn virtue_tint(virtue: u32) -> Option<Color> {
    [
        (OPT3_EXPLOSIONSPIRITS, FURY_RED),
        (OPT3_STEELBODY, STEEL_GOLD),
    ]
    .into_iter()
    .filter(|(bit, _)| virtue & bit != 0)
    .fold(None, |acc, (_, color)| {
        Some(acc.map_or(color, |a| multiply_colors(a, color)))
    })
}

/// Reconciles a unit's opt3 (`virtue`) sprite recolour with its current bitmask:
/// insert [`Opt3Tint`] with the combined tint of the active bits, remove it when
/// none apply. Consumes both channels carrying `virtue` — live `UnitStateChanged`
/// toggles and the spawn-time `UnitEntered` for units that enter view already
/// buffed — so it is ordered after entity spawning, mirroring [`body_state_visuals`].
pub fn virtue_visuals(
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingVirtues>,
    mut commands: Commands,
) {
    for event in state_changes.read() {
        match registry.get_entity(event.unit_id) {
            Some(entity) => apply_virtue(&mut commands, entity, event.virtue),
            None => {
                pending.0.insert(event.unit_id, event.virtue);
            }
        }
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_virtue(&mut commands, entity, event.virtue);
    }

    pending.0.retain(|&unit_id, &mut virtue| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_virtue(&mut commands, entity, virtue);
        false
    });
}

fn apply_virtue(commands: &mut Commands, entity: Entity, virtue: u32) {
    match virtue_tint(virtue) {
        Some(color) => {
            commands.entity(entity).insert(Opt3Tint(color));
        }
        None => {
            commands.entity(entity).remove::<Opt3Tint>();
        }
    }
}

/// `얼음땡.act` action indexes, verified by inspecting the extracted ACT
/// (grf-utils): action 0 is a minimal single-frame, single-layer variant
/// (unused here); action 1 is the 22-frame, 25-layer ice-encasement the
/// classic client draws over a frozen unit, subtly drifting/glinting across
/// its loop.
const FROZEN_ICE_ACTION_INDEX: usize = 1;

/// Number of ACT layers composing one frame of the ice-encasement animation
/// (`얼음땡.act` action 1), fixed across every frame of that action. The
/// overlay spawns exactly this many part-quad children, mirroring the cart's
/// `CART_ACT_PARTS`.
const FROZEN_OVERLAY_PARTS: usize = 25;

/// One ACT-layer part-quad of the ice-block overlay drawn over a frozen unit
/// (`얼음땡.act`/`.spr`, action [`FROZEN_ICE_ACTION_INDEX`]). `part` indexes
/// into the current frame's `parts` slice; [`sync_frozen_overlays`] drives
/// every part's texture/transform off the shared [`FrozenIceAssets`] each
/// frame, looping continuously while the overlay exists. Spawned/despawned as
/// a group by [`apply_body_state`] alongside [`BodyStateTint`]; stone gets no
/// overlay, only the tint.
#[derive(Component, Debug, Clone, Copy)]
pub struct FrozenOverlay {
    part: usize,
}

/// Repeating `sleep.str` effect attached to a sleeping unit.
#[derive(Component, Debug, Clone, Copy)]
pub struct SleepOverlay;

/// Shared, processed `얼음땡` (ice-block) overlay animation. Loaded once;
/// every frozen unit's [`FrozenOverlay`] parts read the same handle,
/// mirroring `domain::emote::assets::EmoteAssets`.
#[derive(Resource)]
pub struct FrozenIceAssets {
    pub animation: Handle<RoAnimationAsset>,
}

/// SPR/ACT handles still loading for [`FrozenIceAssets`].
#[derive(Resource)]
pub struct FrozenIceAssetsPending {
    spr: Handle<RoSpriteAsset>,
    act: Handle<RoActAsset>,
}

/// Kicks off the shared `얼음땡.spr`/`.act` load.
pub fn load_frozen_ice_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(FrozenIceAssetsPending {
        spr: asset_server.load(patterns::frozen_ice_sprite_path()),
        act: asset_server.load(patterns::frozen_ice_action_path()),
    });
}

/// Polls the pending handles; once both are loaded, processes them through
/// `RoAnimationProcessor` and inserts [`FrozenIceAssets`], then drops the
/// pending marker so this runs exactly once.
///
/// A missing GRF sprite otherwise fails silently (a known project gotcha):
/// `Assets::get` just never returns `Some`, so this would poll forever with
/// no overlay and no signal. Guarded here by checking `LoadState::Failed` on
/// either handle — logs once and drops the pending marker so the freeze
/// tint/pause still land, just without the ice overlay.
#[allow(clippy::too_many_arguments)]
pub fn finalize_frozen_ice_assets(
    mut commands: Commands,
    pending: Option<Res<FrozenIceAssetsPending>>,
    asset_server: Res<AssetServer>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<GraphicsSettings>,
) {
    let Some(pending) = pending else {
        return;
    };

    let spr_state = asset_server.load_state(&pending.spr);
    let act_state = asset_server.load_state(&pending.act);
    if let LoadState::Failed(err) = &spr_state {
        warn!("Failed to load frozen-ice overlay sprite: {err:?}; overlay disabled");
        commands.remove_resource::<FrozenIceAssetsPending>();
        return;
    }
    if let LoadState::Failed(err) = &act_state {
        warn!("Failed to load frozen-ice overlay action: {err:?}; overlay disabled");
        commands.remove_resource::<FrozenIceAssetsPending>();
        return;
    }

    let (Some(sprite), Some(action)) = (sprites.get(&pending.spr), actions.get(&pending.act))
    else {
        return;
    };

    let animation = RoAnimationProcessor::process(
        &sprite.sprite,
        &action.action,
        LAYER_EFFECT,
        &mut images,
        settings.upscaling,
    );

    commands.insert_resource(FrozenIceAssets {
        animation: animations.add(animation),
    });
    commands.remove_resource::<FrozenIceAssetsPending>();
}

fn spawn_sleep_overlay(
    commands: &mut Commands,
    entity: Entity,
    asset_server: &AssetServer,
    children_query: &Query<&Children>,
    overlays: &Query<Entity, With<SleepOverlay>>,
) {
    let already_present = children_query
        .get(entity)
        .is_ok_and(|children| children.iter().any(|child| overlays.contains(child)));
    if already_present {
        return;
    }

    let overlay = spawn_effect(
        commands,
        load_str_effect(asset_server, "sleep.str"),
        EffectAnchor::Position(Vec3::ZERO),
        true,
        Color::WHITE,
        None,
    );
    commands
        .entity(overlay)
        .insert((SleepOverlay, ChildOf(entity)));
}

fn despawn_sleep_overlay(
    commands: &mut Commands,
    entity: Entity,
    children_query: &Query<&Children>,
    overlays: &Query<Entity, With<SleepOverlay>>,
) {
    let Ok(children) = children_query.get(entity) else {
        return;
    };
    for child in children.iter() {
        if overlays.contains(child) {
            commands.entity(child).despawn();
        }
    }
}

/// Spawns the [`FROZEN_OVERLAY_PARTS`] part-quad children of the ice-block
/// overlay, empty (hidden) until [`sync_frozen_overlays`] fills them in from
/// [`FrozenIceAssets`] — spawning does not wait on the asset load. A no-op if
/// `entity` already carries an overlay: a freeze -> freeze transition must
/// not stack a second set.
fn spawn_frozen_overlay(
    commands: &mut Commands,
    entity: Entity,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
    children_query: &Query<&Children>,
    overlays: &Query<Entity, With<FrozenOverlay>>,
) {
    let already_present = children_query
        .get(entity)
        .is_ok_and(|children| children.iter().any(|child| overlays.contains(child)));
    if already_present {
        return;
    }

    let z_offset = layer_order(LAYER_EFFECT) as f32 * Z_OFFSET_PER_LAYER;

    for part in 0..FROZEN_OVERLAY_PARTS {
        let material = materials.add(StandardMaterial {
            base_color_texture: None,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            // Later ACT layers draw on top, mirroring the cart's per-part bias.
            depth_bias: layer_depth_bias(LAYER_EFFECT) + part as f32 * 0.01,
            ..default()
        });

        commands.spawn((
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(material),
            Billboard,
            FrozenOverlay { part },
            Transform::from_translation(Vec3::new(0.0, 0.0, z_offset + part as f32 * 0.001)),
            Visibility::Hidden,
            ChildOf(entity),
        ));
    }
}

/// Despawns every [`FrozenOverlay`] child of `entity`, if any.
fn despawn_frozen_overlay(
    commands: &mut Commands,
    entity: Entity,
    children_query: &Query<&Children>,
    overlays: &Query<Entity, With<FrozenOverlay>>,
) {
    let Ok(children) = children_query.get(entity) else {
        return;
    };
    for child in children.iter() {
        if overlays.contains(child) {
            commands.entity(child).despawn();
        }
    }
}

type FrozenOverlayQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static FrozenOverlay,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
>;

/// Drives every ice-block overlay part-quad from the shared
/// [`FrozenIceAssets`] animation, looping [`FROZEN_ICE_ACTION_INDEX`]
/// continuously off the global clock (mirroring the cart's
/// `sync_cart_layer`, which also derives its frame off wall-clock time rather
/// than a per-instance elapsed). A part with no data at the current frame, or
/// the asset not being loaded yet, hides rather than showing stale geometry.
pub fn sync_frozen_overlays(
    time: Res<Time>,
    frozen_ice: Option<Res<FrozenIceAssets>>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut overlays: FrozenOverlayQuery,
) {
    let Some(frozen_ice) = frozen_ice.as_deref() else {
        return;
    };
    let Some(animation) = animations.get(&frozen_ice.animation) else {
        return;
    };
    let Some(action) = animation.actions.get(FROZEN_ICE_ACTION_INDEX) else {
        return;
    };
    if action.frames.is_empty() {
        return;
    }

    let delay = action.delay_ms.max(1.0);
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    let frame = &action.frames[(game_time_ms as f32 / delay) as usize % action.frames.len()];

    for (overlay, material_handle, transform, mut visibility) in &mut overlays {
        let Some(part) = frame.parts.get(overlay.part) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        apply_animation_part(
            part,
            animation,
            &mut materials,
            material_handle,
            transform,
            visibility,
        );
    }
}

/// aesir `e_option` (`OPTION_*`) sprite effect-state bit for Sight
/// (`Aesir.ZoneServer.Mmo.Option`, `:sight => 1`; MG_SIGHT's `option: :sight`
/// applies `SC_SIGHT` with this bit). Matches rAthena `OPTION_SIGHT`.
const OPTION_SIGHT: u32 = 1;

/// Orbit radius, in world units, of the Sight fireball — well outside the
/// body sprite so the flame circles the character instead of clipping it.
const SIGHT_ORBIT_RADIUS: f32 = 9.0;
/// Full revolution period of the Sight fireball.
const SIGHT_ORBIT_PERIOD_SECS: f32 = 2.0;
/// Vertical offset lifting the orbit above the unit's feet; world up is `-Y`.
const SIGHT_ORBIT_LIFT: f32 = -8.0;

/// The anchor entity orbiting a unit with the Sight option bit set. A
/// TOP-LEVEL entity following `unit` from the outside (the STR-effect
/// `EffectAnchor::Entity` pattern), NOT a child of it — effect visuals do not
/// render reliably as unit children. There is no `sight.str` asset in the GRF
/// (verified when filling the effect catalog); the domain spawns only this bare
/// anchor (moved every frame by [`orbit_sight_visuals`], despawned there when
/// the unit disappears) and the presentation layer dresses it with the classic
/// flame visual (`dress_sight_orbits` in
/// `presentation/rendering/effects/skill_fx.rs`).
#[derive(Component, Debug, Clone, Copy)]
pub struct SightOrbit {
    pub unit: Entity,
}

/// Latest `effect_state` for units whose entity has not registered yet, keyed
/// by unit_id. Same shape and rationale as [`PendingBodyStates`].
#[derive(Resource, Default)]
pub struct PendingEffectStates(HashMap<u32, u32>);

/// Reconciles a unit's Sight orbit visual with its current `effect_state`:
/// spawns the orbit child when the option bit is set and none exists yet,
/// despawns it when the bit clears. Consumes both channels that carry
/// `effect_state` — live `UnitStateChanged` toggles and the spawn-time
/// `UnitEntered` for units that enter view already sighted — mirroring
/// [`body_state_visuals`].
pub fn option_visuals(
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingEffectStates>,
    mut commands: Commands,
    transforms: Query<&Transform>,
    orbits: Query<(Entity, &SightOrbit)>,
) {
    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            pending.0.insert(event.unit_id, event.effect_state);
            continue;
        };
        apply_sight_state(
            &mut commands,
            entity,
            event.effect_state,
            &transforms,
            &orbits,
        );
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_sight_state(
            &mut commands,
            entity,
            event.effect_state,
            &transforms,
            &orbits,
        );
    }

    pending.0.retain(|&unit_id, &mut effect_state| {
        let Some(entity) = registry.get_entity(unit_id) else {
            return true;
        };
        apply_sight_state(&mut commands, entity, effect_state, &transforms, &orbits);
        false
    });
}

fn apply_sight_state(
    commands: &mut Commands,
    entity: Entity,
    effect_state: u32,
    transforms: &Query<&Transform>,
    orbits: &Query<(Entity, &SightOrbit)>,
) {
    let sight_on = effect_state & OPTION_SIGHT != 0;
    let existing = orbits
        .iter()
        .find(|(_, orbit)| orbit.unit == entity)
        .map(|(orbit_entity, _)| orbit_entity);

    match (sight_on, existing) {
        (true, None) => {
            // Start at the unit so the first frame never flashes at the origin.
            let start = transforms
                .get(entity)
                .map(|t| t.translation)
                .unwrap_or_default();
            commands.spawn((
                SightOrbit { unit: entity },
                Transform::from_translation(start),
                Visibility::default(),
            ));
        }
        (false, Some(orbit_entity)) => {
            commands.entity(orbit_entity).despawn();
        }
        _ => {}
    }
}

/// Moves every [`SightOrbit`] anchor along its fixed radius/period circle
/// around the tracked unit's current position, lifted above the ground —
/// the same outside-follow the STR `EffectAnchor::Entity` path uses, plus the
/// circular offset. An orbit whose unit is gone (despawned, map change) is
/// despawned here, taking the dressed flame with it.
pub fn orbit_sight_visuals(
    time: Res<Time>,
    mut commands: Commands,
    units: Query<&GlobalTransform>,
    mut orbits: Query<(Entity, &SightOrbit, &mut Transform)>,
) {
    let angle = time.elapsed_secs() * (TAU / SIGHT_ORBIT_PERIOD_SECS);
    let (sin, cos) = angle.sin_cos();
    for (orbit_entity, orbit, mut transform) in &mut orbits {
        let Ok(unit) = units.get(orbit.unit) else {
            commands.entity(orbit_entity).despawn();
            continue;
        };
        transform.translation = unit.translation()
            + Vec3::new(
                cos * SIGHT_ORBIT_RADIUS,
                SIGHT_ORBIT_LIFT,
                sin * SIGHT_ORBIT_RADIUS,
            );
    }
}

/// Marks a status-driven aura effect spawned as a child of the unit it
/// belongs to, tagged with the EFST that owns it so [`efst_auras`] can find
/// and despawn the right child on `on=false`.
#[derive(Component, Debug, Clone, Copy)]
pub struct StatusAura {
    pub efst: u32,
}

/// Attaches/detaches an `efsts`-mapped repeating STR as a child
/// of the unit named by `StatusEffectChanged.unit_id`. Most EFSTs have no
/// catalog entry (only auras like Energy Coat do), so a catalog miss is a
/// silent no-op, not a warning — the sparse catalog is by design.
///
/// A `unit_id` that has not registered yet on `on=true` is warned and
/// skipped rather than buffered: unlike Sight (a single bit reapplied from
/// `UnitEntered` at spawn), an aura can be one of many concurrent EFSTs per
/// unit, so a correct buffer would need a per-efst map — not worth it for a
/// window that only matters if the aura's `StatusEffectChanged` races ahead
/// of the unit's own spawn packet. `on=false` for an unresolved unit is a
/// silent no-op: there is nothing to despawn.
pub fn efst_auras(
    mut events: MessageReader<StatusEffectChanged>,
    registry: Res<EntityRegistry>,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    children_query: Query<&Children>,
    auras: Query<&StatusAura>,
) {
    for event in events.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            if event.on {
                warn!(
                    "StatusEffectChanged(efst {}, on) for unresolved unit {}; aura skipped",
                    event.efst, event.unit_id
                );
            }
            continue;
        };

        let existing_child = children_query.get(entity).ok().and_then(|children| {
            children
                .iter()
                .find(|child| auras.get(*child).is_ok_and(|aura| aura.efst == event.efst))
        });

        if !event.on {
            if let Some(child) = existing_child {
                commands.entity(child).despawn();
            }
            continue;
        }

        if existing_child.is_some() {
            continue;
        }

        let Some(descriptor) = catalog.as_deref().and_then(|c| c.efst(event.efst)) else {
            continue;
        };
        let Some(handle) = load_effect(&asset_server, descriptor) else {
            continue;
        };
        let effect = spawn_effect(
            &mut commands,
            handle,
            EffectAnchor::Position(Vec3::ZERO),
            descriptor.repeating,
            descriptor_tint(descriptor),
            None,
        );
        commands
            .entity(effect)
            .insert((StatusAura { efst: event.efst }, ChildOf(entity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingBodyStates>()
            .init_resource::<PendingVirtues>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<crate::infrastructure::effect::LoadedEffectAsset>()
            .add_systems(Update, (body_state_visuals, virtue_visuals));

        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(crate::domain::entities::billboard::create_sprite_quad_mesh());
        app.insert_resource(SharedSpriteQuad { mesh });
        app
    }

    fn frozen_overlay_parts(app: &mut App, unit: Entity) -> usize {
        app.world()
            .get::<Children>(unit)
            .map(|children| {
                children
                    .iter()
                    .filter(|child| app.world().get::<FrozenOverlay>(*child).is_some())
                    .count()
            })
            .unwrap_or(0)
    }

    fn sleep_overlays(app: &mut App, unit: Entity) -> usize {
        app.world()
            .get::<Children>(unit)
            .map(|children| {
                children
                    .iter()
                    .filter(|child| app.world().get::<SleepOverlay>(*child).is_some())
                    .count()
            })
            .unwrap_or(0)
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit_state(app: &mut App, unit_id: u32, body_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id,
                body_state,
                health_state: 0,
                effect_state: 0,
                virtue: 0,
            });
        app.update();
    }

    fn entered_stub() -> UnitEntered {
        UnitEntered {
            gid: 0,
            aid: 0,
            object_type: 0,
            job: 0,
            x: 0,
            y: 0,
            dir: 0,
            speed: 0,
            hp: 0,
            max_hp: 0,
            clevel: 0,
            body_state: 0,
            health_state: 0,
            effect_state: 0,
            virtue: 0,
            spirit_sphere_count: 0,
            head: 0,
            weapon: 0,
            shield: 0,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            head_palette: 0,
            body_palette: 0,
            head_dir: 0,
            robe: 0,
            guild_id: 0,
            guild_name: String::new(),
            emblem_id: 0,
            sex: 0,
            is_boss: false,
            name: String::new(),
            moving: false,
            dst_x: 0,
            dst_y: 0,
            move_start_time: 0,
        }
    }

    fn emit_entered(app: &mut App, gid: u32, body_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
                gid,
                body_state,
                ..entered_stub()
            });
        app.update();
    }

    #[test]
    fn freeze_inserts_tint_and_pause() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);

        let tint = app.world().get::<BodyStateTint>(unit);
        assert_eq!(tint, Some(&BodyStateTint(ICE_BLUE)));
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }

    #[test]
    fn sleep_attaches_one_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_SLEEP);

        assert_eq!(sleep_overlays(&mut app, unit), 1);
        let child = app
            .world()
            .get::<Children>(unit)
            .unwrap()
            .iter()
            .find(|child| app.world().get::<SleepOverlay>(*child).is_some())
            .unwrap();
        let effect = app
            .world()
            .get::<super::super::components::ActiveEffect>(child)
            .unwrap();
        assert!(effect.repeating);
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(effect.effect.id())
                .unwrap()
                .to_string(),
            "ro://data/texture/effect/sleep.str"
        );
    }

    #[test]
    fn clearing_sleep_removes_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_SLEEP);
        assert_eq!(sleep_overlays(&mut app, unit), 1);

        emit_state(&mut app, 7, 0);
        assert_eq!(sleep_overlays(&mut app, unit), 0);
    }

    #[test]
    fn sleep_does_not_tint_or_pause_animation() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_SLEEP);

        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
    }

    #[test]
    fn stone_inserts_gray_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_STONE);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
    }

    #[test]
    fn stonewait_inserts_gray_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_STONEWAIT);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
    }

    #[test]
    fn clearing_body_state_removes_both() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert!(app.world().get::<BodyStateTint>(unit).is_some());

        emit_state(&mut app, 7, 0);
        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
    }

    fn emit_virtue(app: &mut App, unit_id: u32, virtue: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id,
                body_state: 0,
                health_state: 0,
                effect_state: 0,
                virtue,
            });
        app.update();
    }

    #[test]
    fn fury_inserts_red_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_virtue(&mut app, 7, OPT3_EXPLOSIONSPIRITS);

        assert_eq!(app.world().get::<Opt3Tint>(unit), Some(&Opt3Tint(FURY_RED)));
    }

    #[test]
    fn steel_body_inserts_gold_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_virtue(&mut app, 7, OPT3_STEELBODY);

        assert_eq!(
            app.world().get::<Opt3Tint>(unit),
            Some(&Opt3Tint(STEEL_GOLD))
        );
    }

    #[test]
    fn blade_stop_alone_inserts_no_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        // Blade Stop (32) is an icon-only pose with no sprite recolour.
        emit_virtue(&mut app, 7, 32);

        assert!(app.world().get::<Opt3Tint>(unit).is_none());
    }

    #[test]
    fn fury_and_steel_body_combine_multiplicatively() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        // Bitmask OR of Fury (8) + Steel Body (16) = 24.
        emit_virtue(&mut app, 7, OPT3_EXPLOSIONSPIRITS | OPT3_STEELBODY);

        assert_eq!(
            app.world().get::<Opt3Tint>(unit),
            Some(&Opt3Tint(multiply_colors(FURY_RED, STEEL_GOLD)))
        );
    }

    #[test]
    fn clearing_virtue_removes_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_virtue(&mut app, 7, OPT3_STEELBODY);
        assert!(app.world().get::<Opt3Tint>(unit).is_some());

        emit_virtue(&mut app, 7, 0);
        assert!(app.world().get::<Opt3Tint>(unit).is_none());
    }

    #[test]
    fn entered_with_virtue_tints_spawn_time() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        // A unit that walks into view already under Steel Body carries opt3 on
        // its spawn packet, so the tint applies without a follow-up state change.
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
                gid: 7,
                virtue: OPT3_STEELBODY,
                ..entered_stub()
            });
        app.update();

        assert_eq!(
            app.world().get::<Opt3Tint>(unit),
            Some(&Opt3Tint(STEEL_GOLD))
        );
    }

    #[test]
    fn virtue_before_registration_is_buffered_then_applied() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();

        // opt3 arrives before the entity registers (map-load self-sync race).
        emit_virtue(&mut app, 7, OPT3_STEELBODY);
        register(&mut app, 7, unit);
        app.update();

        assert_eq!(
            app.world().get::<Opt3Tint>(unit),
            Some(&Opt3Tint(STEEL_GOLD))
        );
    }

    #[test]
    fn virtue_tint_folds_only_recognised_bits() {
        assert_eq!(virtue_tint(0), None);
        assert_eq!(virtue_tint(32), None); // Blade Stop only.
        assert_eq!(virtue_tint(OPT3_EXPLOSIONSPIRITS), Some(FURY_RED));
        assert_eq!(virtue_tint(OPT3_STEELBODY), Some(STEEL_GOLD));
        // Unknown high bits alongside a known one are ignored.
        assert_eq!(virtue_tint(OPT3_STEELBODY | 1024), Some(STEEL_GOLD));
    }

    #[test]
    fn unit_entered_frozen_applies_at_spawn() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, 7, OPT1_FREEZE);

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }

    #[test]
    fn unresolved_unit_does_not_touch_other_units() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 999, OPT1_FREEZE);

        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
    }

    #[test]
    fn state_before_registration_applies_after_it() {
        let mut app = app();

        // Freeze arrives while the unit's entity has not registered yet (the
        // local-player zone-in race): buffered, not dropped.
        emit_state(&mut app, 7, OPT1_FREEZE);

        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);
        app.update();

        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
    }

    #[test]
    fn freeze_spawns_ice_overlay_parts() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);

        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);
    }

    #[test]
    fn clearing_freeze_despawns_ice_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);

        emit_state(&mut app, 7, 0);
        assert_eq!(frozen_overlay_parts(&mut app, unit), 0);
    }

    #[test]
    fn stone_spawns_no_ice_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_STONE);

        assert_eq!(frozen_overlay_parts(&mut app, unit), 0);
    }

    #[test]
    fn freeze_sleep_and_stone_transitions_keep_visuals_distinct() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);
        assert_eq!(sleep_overlays(&mut app, unit), 0);

        emit_state(&mut app, 7, OPT1_SLEEP);
        assert!(app.world().get::<BodyStateTint>(unit).is_none());
        assert!(app.world().get::<AnimationPaused>(unit).is_none());
        assert_eq!(frozen_overlay_parts(&mut app, unit), 0);
        assert_eq!(sleep_overlays(&mut app, unit), 1);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(ICE_BLUE))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);
        assert_eq!(sleep_overlays(&mut app, unit), 0);

        emit_state(&mut app, 7, OPT1_STONE);
        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
        assert!(app.world().get::<AnimationPaused>(unit).is_some());
        assert_eq!(frozen_overlay_parts(&mut app, unit), 0);
        assert_eq!(sleep_overlays(&mut app, unit), 0);
    }

    #[test]
    fn freeze_then_stone_removes_overlay_and_grays_tint() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);

        emit_state(&mut app, 7, OPT1_STONE);

        assert_eq!(frozen_overlay_parts(&mut app, unit), 0);
        assert_eq!(
            app.world().get::<BodyStateTint>(unit),
            Some(&BodyStateTint(STONE_GRAY))
        );
    }

    #[test]
    fn repeated_freeze_does_not_stack_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_FREEZE);
        emit_state(&mut app, 7, OPT1_FREEZE);

        assert_eq!(frozen_overlay_parts(&mut app, unit), FROZEN_OVERLAY_PARTS);
    }

    /// `apply_body_state` is called from three places (state changes,
    /// `UnitEntered`, and the pending drain), and a slept unit keeps receiving
    /// `UnitStateChanged` with the same `body_state`, so the spawn guard is
    /// load-bearing rather than decorative.
    #[test]
    fn repeated_sleep_does_not_stack_overlay() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, 7, OPT1_SLEEP);
        emit_state(&mut app, 7, OPT1_SLEEP);

        assert_eq!(sleep_overlays(&mut app, unit), 1);
    }

    fn sight_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .init_resource::<PendingEffectStates>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Update, option_visuals);
        app
    }

    fn emit_effect_state(app: &mut App, unit_id: u32, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id,
                body_state: 0,
                health_state: 0,
                effect_state,
                virtue: 0,
            });
        app.update();
    }

    fn orbits_for(app: &mut App, unit: Entity) -> Vec<Entity> {
        app.world_mut()
            .query::<(Entity, &SightOrbit)>()
            .iter(app.world())
            .filter(|(_, orbit)| orbit.unit == unit)
            .map(|(entity, _)| entity)
            .collect()
    }

    #[test]
    fn sight_bit_spawns_orbit_anchor() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);

        assert_eq!(
            orbits_for(&mut app, unit).len(),
            1,
            "sight bit spawns a top-level orbit anchor tracking the unit"
        );
    }

    #[test]
    fn clearing_sight_bit_despawns_orbit_anchor() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);
        assert_eq!(orbits_for(&mut app, unit).len(), 1);

        emit_effect_state(&mut app, 7, 0);
        assert!(
            orbits_for(&mut app, unit).is_empty(),
            "clearing the bit despawns the orbit anchor"
        );
    }

    #[test]
    fn repeated_sight_bit_does_not_stack_anchors() {
        let mut app = sight_app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);
        emit_effect_state(&mut app, 7, OPTION_SIGHT | 2); // sight stays on, another bit toggles

        assert_eq!(
            orbits_for(&mut app, unit).len(),
            1,
            "re-applying the bit does not spawn a duplicate"
        );
    }

    #[test]
    fn sight_state_before_registration_applies_after_it() {
        let mut app = sight_app();

        emit_effect_state(&mut app, 7, OPTION_SIGHT);

        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);
        app.update();

        assert_eq!(
            orbits_for(&mut app, unit).len(),
            1,
            "buffered sight state applies once the unit resolves"
        );
    }

    #[test]
    fn orbit_despawns_when_its_unit_disappears() {
        let mut app = sight_app();
        app.add_systems(Update, orbit_sight_visuals);
        let unit = app.world_mut().spawn(Transform::default()).id();
        register(&mut app, 7, unit);

        emit_effect_state(&mut app, 7, OPTION_SIGHT);
        assert_eq!(orbits_for(&mut app, unit).len(), 1);

        app.world_mut().entity_mut(unit).despawn();
        app.update();

        assert!(
            orbits_for(&mut app, unit).is_empty(),
            "an orbit whose unit is gone self-despawns"
        );
    }

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset =
            ron::from_str::<crate::infrastructure::effect::EffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::build(&asset.0).expect("seed catalog builds")
    }

    const EFST_ENERGYCOAT: u32 = 31; // aesir Efst.id(:energycoat).

    fn aura_app(catalog: EffectCatalog) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<crate::infrastructure::effect::LoadedEffectAsset>()
            .add_message::<StatusEffectChanged>()
            .init_resource::<EntityRegistry>()
            .insert_resource(catalog)
            .add_systems(Update, efst_auras);
        app
    }

    fn emit_status_effect(app: &mut App, unit_id: u32, efst: u32, on: bool) {
        app.world_mut()
            .resource_mut::<Messages<StatusEffectChanged>>()
            .write(StatusEffectChanged {
                unit_id,
                efst,
                on,
                total_ms: 0,
                remain_ms: 0,
            });
        app.update();
    }

    fn aura_children(app: &mut App, unit: Entity) -> Vec<u32> {
        let world = app.world();
        world
            .get::<Children>(unit)
            .map(|children| {
                children
                    .iter()
                    .filter_map(|child| world.get::<StatusAura>(child).map(|aura| aura.efst))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn efst_on_with_catalog_hit_attaches_aura_child() {
        let mut app = aura_app(seeded_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);
    }

    #[test]
    fn efst_off_detaches_aura_child() {
        let mut app = aura_app(seeded_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);
        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, false);
        assert!(aura_children(&mut app, unit).is_empty());
    }

    #[test]
    fn efst_on_without_catalog_entry_is_a_noop() {
        let mut app = aura_app(seeded_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, 9999, true);

        assert!(aura_children(&mut app, unit).is_empty());
    }

    #[test]
    fn efst_repeated_on_does_not_stack_children() {
        let mut app = aura_app(seeded_catalog());
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);
        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        assert_eq!(aura_children(&mut app, unit), vec![EFST_ENERGYCOAT]);
    }

    #[test]
    fn efst_on_for_unresolved_unit_is_warn_and_skip() {
        let mut app = aura_app(seeded_catalog());

        // No entity registered for unit_id 7: must not panic, and there is
        // nothing to attach the aura to.
        emit_status_effect(&mut app, 7, EFST_ENERGYCOAT, true);

        let mut auras = app.world_mut().query::<&StatusAura>();
        assert_eq!(auras.iter(app.world()).count(), 0);
    }
}
