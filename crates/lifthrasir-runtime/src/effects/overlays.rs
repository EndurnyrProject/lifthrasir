use super::lookup::load_str_effect;
use super::sprites::apply_animation_part;
use super::{EffectAnchor, spawn_effect};
use crate::actors::RenderLayer;
use crate::rendering::layers::{LAYER_EFFECT, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order};
use crate::rendering::{Billboard, SharedSpriteQuad};
use bevy::asset::LoadState;
use bevy::prelude::*;
use lifthrasir_assets::animation::{RoAnimationAsset, RoAnimationProcessor, SpriteAssetBank};
use lifthrasir_assets::loaders::{RoActAsset, RoSpriteAsset};
use lifthrasir_assets::paths;
use std::f32::consts::TAU;

/// A top-level visual anchor following a unit; the caller owns status interpretation.
#[derive(Component, Debug, Clone, Copy)]
pub struct SightOrbit {
    pub unit: Entity,
}

/// One top-level sphere anchor; the caller owns the requested ring size.
#[derive(Component, Debug, Clone, Copy)]
pub struct SpiritSphereOrbit {
    pub unit: Entity,
    /// This sphere's slot within the ring, `0..count`.
    pub index: u32,
    /// Ring size when spawned; fixes each sphere's angular offset.
    pub count: u32,
}
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

/// Colour multiplied into a unit's sprite layers for its active opt3 (`virtue`)
/// bits, combined multiplicatively with [`BodyStateTint`] by
/// [`apply_body_state_tint`]. Absent means no opt3 recolour.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Opt3Tint(pub Color);

/// Channel-wise sRGB multiply of two tints (white is the identity), matching how
/// `base_color` multiplies into the sprite texture.
pub fn multiply_colors(a: Color, b: Color) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgb(a.red * b.red, a.green * b.green, a.blue * b.blue)
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
/// a group by the caller alongside its tint/pause markers.
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
        spr: asset_server.load(paths::frozen_ice_sprite_path()),
        act: asset_server.load(paths::frozen_ice_action_path()),
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
pub fn finalize_frozen_ice_assets(
    mut commands: Commands,
    pending: Option<Res<FrozenIceAssetsPending>>,
    asset_server: Res<AssetServer>,
    mut bank: SpriteAssetBank,
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

    let (Some(sprite), Some(action)) = (
        bank.sprites.get(&pending.spr),
        bank.actions.get(&pending.act),
    ) else {
        return;
    };

    let animation = RoAnimationProcessor::process(
        &sprite.sprite,
        &action.action,
        None,
        LAYER_EFFECT,
        &mut bank.images,
        bank.settings.upscaling,
    );

    commands.insert_resource(FrozenIceAssets {
        animation: bank.animations.add(animation),
    });
    commands.remove_resource::<FrozenIceAssetsPending>();
}

pub fn spawn_sleep_overlay(
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

pub fn despawn_sleep_overlay(
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
pub fn spawn_frozen_overlay(
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
pub fn despawn_frozen_overlay(
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

/// Orbit radius, in world units, of the Sight fireball — well outside the
/// body sprite so the flame circles the character instead of clipping it.
const SIGHT_ORBIT_RADIUS: f32 = 9.0;
/// Full revolution period of the Sight fireball.
const SIGHT_ORBIT_PERIOD_SECS: f32 = 2.0;
/// Vertical offset lifting the orbit above the unit's feet.
const SIGHT_ORBIT_LIFT: f32 = 8.0;

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

/// Orbit radius, in world units, of each spirit sphere — wide enough that the
/// ring clears the body sprite instead of clipping through it.
const SPHERE_ORBIT_RADIUS: f32 = 11.0;
/// Full revolution period of the sphere ring.
const SPHERE_ORBIT_PERIOD_SECS: f32 = 3.0;
/// Vertical offset lifting the ring to the unit's upper body.
const SPHERE_ORBIT_LIFT: f32 = 10.0;
/// Amplitude of the per-sphere vertical bob.
const SPHERE_BOB_AMPLITUDE: f32 = 0.8;
/// Period of the per-sphere vertical bob.
const SPHERE_BOB_PERIOD_SECS: f32 = 1.6;
/// Phase offset between neighbouring spheres' bobs so the ring undulates
/// instead of moving as one rigid plate.
const SPHERE_BOB_PHASE_STEP: f32 = 1.7;

/// Moves every [`SpiritSphereOrbit`] anchor along the shared ring around its
/// unit's current position, each sphere offset by its slot angle and bobbing
/// gently out of phase with its neighbours. An orbit whose unit is gone
/// (despawned, map change) is despawned here, taking the dressed ball with it.
pub fn orbit_spirit_spheres(
    time: Res<Time>,
    mut commands: Commands,
    units: Query<&GlobalTransform>,
    mut orbits: Query<(Entity, &SpiritSphereOrbit, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    let base_angle = elapsed * (TAU / SPHERE_ORBIT_PERIOD_SECS);
    for (orbit_entity, orbit, mut transform) in &mut orbits {
        let Ok(unit) = units.get(orbit.unit) else {
            commands.entity(orbit_entity).despawn();
            continue;
        };
        let slot = orbit.index as f32 * TAU / orbit.count.max(1) as f32;
        let (sin, cos) = (base_angle + slot).sin_cos();
        let bob = (elapsed * (TAU / SPHERE_BOB_PERIOD_SECS)
            + orbit.index as f32 * SPHERE_BOB_PHASE_STEP)
            .sin()
            * SPHERE_BOB_AMPLITUDE;
        transform.translation = unit.translation()
            + Vec3::new(
                cos * SPHERE_ORBIT_RADIUS,
                SPHERE_ORBIT_LIFT + bob,
                sin * SPHERE_ORBIT_RADIUS,
            );
    }
}
