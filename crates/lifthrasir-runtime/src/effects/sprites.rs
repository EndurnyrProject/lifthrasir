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

use crate::rendering::constants::SPRITE_WORLD_SCALE;
use crate::rendering::layers::{LAYER_EFFECT, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order};
use crate::rendering::set_layer_texture;
use crate::rendering::{Billboard, SharedSpriteQuad};
use lifthrasir_assets::SpriteProcessingSettings;
use lifthrasir_assets::animation::SpriteAssetBank;
use lifthrasir_assets::animation::asset::{FramePart, RoAnimationAsset};
use lifthrasir_assets::animation::processor::RoAnimationProcessor;
use lifthrasir_assets::loaders::{RoActAsset, RoSpriteAsset};

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
        // RO centres the sprite on the ACT position; the authored y offset
        // (already Y-up after extraction) lifts the sprite off the cell.
        translation: Vec3::new(
            part.position.x * SPRITE_WORLD_SCALE,
            part.position.y * SPRITE_WORLD_SCALE,
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
pub fn spawn_effect_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<EffectSpriteAssets>,
    mut bank: SpriteAssetBank,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Option<Res<SharedSpriteQuad>>,
    requests: Query<(Entity, &EffectSprite)>,
) {
    let Some(shared_quad) = shared_quad else {
        return;
    };

    for (entity, request) in &requests {
        let Some(animation) = resolve_animation(
            &asset_server,
            &mut assets,
            &bank.sprites,
            &bank.actions,
            &mut bank.animations,
            &mut bank.images,
            &bank.settings,
            &mut commands,
            entity,
            &request.path,
        ) else {
            continue;
        };

        let Some(action) = bank
            .animations
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
    settings: &SpriteProcessingSettings,
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
        None,
        LAYER_EFFECT,
        images,
        settings.upscaling,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeCorePlugin;
    use bevy::time::TimeUpdateStrategy;
    use lifthrasir_assets::{
        ContentAssetsPlugin,
        animation::{ActionData, FrameData},
    };
    use std::time::Duration;

    #[test]
    fn synthetic_sprite_animates_and_requeues_held_material_without_gameplay_resources() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin))
            .init_resource::<EffectSpriteAssets>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .add_systems(Update, (spawn_effect_sprites, sync_effect_sprites).chain());
        let first = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let second = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let frame = |texture_index| FrameData {
            parts: vec![FramePart {
                texture_index,
                transform: Mat4::IDENTITY,
                position: Vec2::ZERO,
                scale: Vec2::ONE,
                texture_size: Vec2::new(2.0, 4.0),
                color: Color::WHITE,
                mirror: false,
            }],
            ..default()
        };
        let animation = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(RoAnimationAsset {
                textures: vec![first.clone(), second.clone()],
                actions: vec![ActionData {
                    frames: vec![frame(0), frame(1)],
                    delay_ms: 100.0,
                }],
                layer: LAYER_EFFECT,
                sounds: vec![],
            });
        app.world_mut()
            .resource_mut::<EffectSpriteAssets>()
            .ready
            .insert("synthetic".into(), animation);
        app.world_mut().spawn(EffectSprite {
            path: "synthetic".into(),
            tint: Color::WHITE,
        });
        app.update();
        let parts: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<EffectSpritePart>>()
            .iter(app.world())
            .collect();
        assert_eq!(parts.len(), 1);
        let material = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(parts[0])
            .unwrap()
            .0
            .clone();
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap()
                .base_color_texture,
            Some(first)
        );
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        app.update();
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap()
                .base_color_texture,
            Some(second)
        );
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .clear();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        let events = app
            .world()
            .resource::<Messages<AssetEvent<StandardMaterial>>>();
        let mut cursor = events.get_cursor();
        assert!(
            cursor
                .read(events)
                .any(|event| matches!(event, AssetEvent::Modified { id } if *id == material.id())),
            "held textures must still mark the material modified for transparent-sort requeue"
        );
    }
}
