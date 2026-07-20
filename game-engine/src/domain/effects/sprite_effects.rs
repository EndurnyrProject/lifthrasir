//! Persistent SPR/ACT sprite visuals for effects the classic client draws as
//! animated sprites rather than STR sequences (Fire Wall's `이팩트/firewall`,
//! Fire Pillar's `이팩트/화염진`).
//!
//! An [`EffectSprite`] marker declares "dress this entity with that animation";
//! [`spawn_effect_sprites`] loads the SPR/ACT pair once per path (shared through
//! [`EffectSpriteAssets`]), then replaces the marker with one billboard child per
//! ACT layer. [`sync_effect_sprites`] drives those children off the global clock,
//! looping the animation for as long as the parent lives — the parent (a
//! skill-unit cell) despawns the whole subtree when the server removes it.

use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy_persistent::prelude::Persistent;

use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::domain::settings::resources::Settings;
use crate::domain::sprite::tags::{
    layer_depth_bias, layer_order, LAYER_EFFECT, Z_OFFSET_PER_LAYER,
};
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::{FramePart, RoAnimationAsset};
use crate::utils::constants::SPRITE_WORLD_SCALE;

/// Effect sprites carry one action; the classic client plays it on a loop.
const EFFECT_SPRITE_ACTION: usize = 0;

/// Request to dress `entity` with the looping SPR/ACT animation at `path` (a
/// stem under `data/sprite/`, e.g. `이팩트/firewall`), tinted by `tint`.
/// Replaced by the part children once the animation is ready.
#[derive(Component, Debug, Clone)]
pub struct EffectSprite {
    pub path: String,
    pub tint: Color,
}

/// One ACT-layer quad of a spawned [`EffectSprite`]. `part` indexes into the
/// current frame's `parts`; the handle is kept per part so several different
/// effect sprites can animate side by side.
#[derive(Component, Debug, Clone)]
pub struct EffectSpritePart {
    animation: Handle<RoAnimationAsset>,
    part: usize,
}

/// Shared, processed effect animations keyed by sprite path, plus the SPR/ACT
/// handles still in flight. One entry serves every cell using that path.
#[derive(Resource, Default)]
pub struct EffectSpriteAssets {
    ready: HashMap<String, Handle<RoAnimationAsset>>,
    pending: HashMap<String, (Handle<RoSpriteAsset>, Handle<RoActAsset>)>,
}

pub(super) fn apply_animation_part(
    part: &FramePart,
    animation: &RoAnimationAsset,
    materials: &mut Assets<StandardMaterial>,
    material: &MeshMaterial3d<StandardMaterial>,
    mut transform: Mut<Transform>,
    mut visibility: Mut<Visibility>,
) {
    if let Some(texture) = animation.textures.get(part.texture_index) {
        set_layer_texture(materials, &material.0, texture);
    }

    let scale_x = part.scale.x
        * part.texture_size.x
        * SPRITE_WORLD_SCALE
        * if part.mirror { -1.0 } else { 1.0 };
    let current = *transform;
    transform.set_if_neq(Transform {
        scale: Vec3::new(
            scale_x,
            part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE,
            1.0,
        ),
        // World up is -Y and RO centres the sprite on the ACT position, so
        // the authored y offset lifts the sprite off the cell when negated.
        translation: Vec3::new(
            part.position.x * SPRITE_WORLD_SCALE,
            -part.position.y * SPRITE_WORLD_SCALE,
            current.translation.z,
        ),
        ..current
    });
    visibility.set_if_neq(Visibility::Inherited);
}

/// Resolves every [`EffectSprite`] request: starts the SPR/ACT load the first
/// time a path is seen, and once processed, spawns the part quads and drops the
/// marker so the request resolves exactly once.
///
/// A missing GRF sprite otherwise fails silently (`Assets::get` simply never
/// returns `Some`), so a `LoadState::Failed` on either handle drops the request
/// with a warning rather than retrying forever.
#[allow(clippy::too_many_arguments)]
pub fn spawn_effect_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<EffectSpriteAssets>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Option<Res<SharedSpriteQuad>>,
    settings: Res<Persistent<Settings>>,
    requests: Query<(Entity, &EffectSprite)>,
) {
    let Some(shared_quad) = shared_quad else {
        return;
    };

    for (entity, request) in &requests {
        let Some(animation) = resolve_animation(
            &asset_server,
            &mut assets,
            &sprites,
            &actions,
            &mut animations,
            &mut images,
            &settings,
            &mut commands,
            entity,
            &request.path,
        ) else {
            continue;
        };

        let Some(action) = animations
            .get(&animation)
            .and_then(|a| a.actions.get(EFFECT_SPRITE_ACTION))
        else {
            warn!(
                "Effect sprite {} has no action {EFFECT_SPRITE_ACTION}; skipping visual",
                request.path
            );
            commands.entity(entity).remove::<EffectSprite>();
            continue;
        };

        let parts = action
            .frames
            .iter()
            .map(|frame| frame.parts.len())
            .max()
            .unwrap_or(0);

        let z_offset = layer_order(LAYER_EFFECT) as f32 * Z_OFFSET_PER_LAYER;
        for part in 0..parts {
            let material = materials.add(StandardMaterial {
                base_color: request.tint,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                depth_bias: layer_depth_bias(LAYER_EFFECT) + part as f32 * 0.01,
                ..default()
            });

            commands.spawn((
                Mesh3d(shared_quad.mesh.clone()),
                MeshMaterial3d(material),
                Billboard,
                EffectSpritePart {
                    animation: animation.clone(),
                    part,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, z_offset + part as f32 * 0.001)),
                Visibility::Hidden,
                // A ground cell's flat click collider is the only click target;
                // the visual must never swallow the ray.
                Pickable::IGNORE,
                ChildOf(entity),
            ));
        }

        commands.entity(entity).remove::<EffectSprite>();
    }
}

/// Returns the processed animation for `path`, kicking off (and finishing) the
/// SPR/ACT load on the way. `None` while it is still loading; a failed load
/// drops the request from `entity` and also returns `None`.
#[allow(clippy::too_many_arguments)]
fn resolve_animation(
    asset_server: &AssetServer,
    assets: &mut EffectSpriteAssets,
    sprites: &Assets<RoSpriteAsset>,
    actions: &Assets<RoActAsset>,
    animations: &mut Assets<RoAnimationAsset>,
    images: &mut Assets<Image>,
    settings: &Persistent<Settings>,
    commands: &mut Commands,
    entity: Entity,
    path: &str,
) -> Option<Handle<RoAnimationAsset>> {
    if let Some(ready) = assets.ready.get(path) {
        return Some(ready.clone());
    }

    let handles = assets.pending.entry(path.to_string()).or_insert_with(|| {
        (
            asset_server.load(format!("ro://data/sprite/{path}.spr")),
            asset_server.load(format!("ro://data/sprite/{path}.act")),
        )
    });

    let failed = matches!(asset_server.load_state(&handles.0), LoadState::Failed(_))
        || matches!(asset_server.load_state(&handles.1), LoadState::Failed(_));
    if failed {
        warn!("Failed to load effect sprite {path}; visual disabled");
        commands.entity(entity).remove::<EffectSprite>();
        return None;
    }

    let (sprite, action) = (sprites.get(&handles.0)?, actions.get(&handles.1)?);
    let animation = animations.add(RoAnimationProcessor::process(
        &sprite.sprite,
        &action.action,
        LAYER_EFFECT,
        images,
        settings.graphics.upscaling,
    ));

    assets.pending.remove(path);
    assets.ready.insert(path.to_string(), animation.clone());
    Some(animation)
}

/// Drives every spawned effect-sprite part off the global clock, looping
/// [`EFFECT_SPRITE_ACTION`] the way the classic client does. A part with no data
/// at the current frame hides rather than showing stale geometry.
pub fn sync_effect_sprites(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: Query<(
        &EffectSpritePart,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let game_time_ms = time.elapsed_secs() * 1000.0;

    for (sprite_part, material_handle, transform, mut visibility) in &mut parts {
        let Some(animation) = animations.get(&sprite_part.animation) else {
            continue;
        };
        let Some(action) = animation.actions.get(EFFECT_SPRITE_ACTION) else {
            continue;
        };
        if action.frames.is_empty() {
            continue;
        }

        let delay = action.delay_ms.max(1.0);
        let frame = &action.frames[(game_time_ms / delay) as usize % action.frames.len()];

        let Some(part) = frame.parts.get(sprite_part.part) else {
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
