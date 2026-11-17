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
//! A root that also carries [`EffectSpriteOnce`] instead plays its action once
//! from frame zero and despawns itself (one-shot skill hits).

use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::prelude::*;

use super::EffectAnchor;
use crate::rendering::constants::SPRITE_WORLD_SCALE;
use crate::rendering::layers::{LAYER_EFFECT, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order};
use crate::rendering::set_layer_texture;
use crate::rendering::{Billboard, SharedSpriteQuad};
use lifthrasir_assets::SpriteProcessingSettings;
use lifthrasir_assets::animation::SpriteAssetBank;
use lifthrasir_assets::animation::asset::{FramePart, RoAnimationAsset};
use lifthrasir_assets::animation::processor::RoAnimationProcessor;
use lifthrasir_assets::loaders::{RoActAsset, RoSpriteAsset};

/// Effect sprites default to their first action.
const EFFECT_SPRITE_ACTION: usize = 0;

/// Request to dress `entity` with the looping SPR/ACT animation at `path` (a
/// stem under `data/sprite/`, e.g. `이팩트/firewall`), tinted by `tint`.
/// Replaced by the part children once the animation is ready.
#[derive(Component, Debug, Clone)]
pub struct EffectSprite {
    pub path: String,
    pub tint: Color,
}

/// Selects an ACT action for an effect sprite; absent means action zero.
/// May change during playback, for example for a directional attachment.
#[derive(Component, Debug, Clone, Copy)]
pub struct EffectSpriteAction(pub usize);

/// Plays an [`EffectSprite`]'s action once from its first frame instead of
/// looping, then despawns the entity.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct EffectSpriteOnce;

/// Game time (ms) a one-shot sprite's parts spawned: its frame zero.
#[derive(Component, Debug, Clone, Copy)]
pub struct EffectSpriteStart(f32);

/// Spawns a one-shot SPR/ACT effect at `anchor` (a stem under `data/sprite/`)
/// that plays once and despawns.
pub fn spawn_effect_sprite_once(
    commands: &mut Commands,
    path: &str,
    anchor: EffectAnchor,
    tint: Color,
) -> Entity {
    let translation = match anchor {
        EffectAnchor::Position(position) => position,
        EffectAnchor::Entity(_) => Vec3::ZERO,
    };
    commands
        .spawn((
            EffectSprite {
                path: path.to_string(),
                tint,
            },
            EffectSpriteOnce,
            anchor,
            Transform::from_translation(translation),
            Visibility::default(),
        ))
        .id()
}

/// One ACT-layer quad of a spawned [`EffectSprite`]. `part` indexes into the
/// current frame's `parts`; the handle is kept per part so several different
/// effect sprites can animate side by side.
#[derive(Component, Debug, Clone)]
pub struct EffectSpritePart {
    animation: Handle<RoAnimationAsset>,
    part: usize,
    tint: Color,
}

/// Shared, processed effect animations keyed by sprite path, plus the SPR/ACT
/// handles still in flight. One entry serves every cell using that path.
#[derive(Resource, Default)]
pub struct EffectSpriteAssets {
    ready: HashMap<String, Handle<RoAnimationAsset>>,
    pending: HashMap<String, (Handle<RoSpriteAsset>, Handle<RoActAsset>)>,
}

/// Poses one ACT layer quad and colours it `tint` times the layer's authored
/// RGBA. The ACT colour carries real intent: `이팩트/torch_01`'s halo layer is
/// a ~8x-scaled glow at ~13% alpha, which draws as an opaque square without it.
pub(super) fn apply_animation_part(
    part: &FramePart,
    animation: &RoAnimationAsset,
    tint: Color,
    materials: &mut Assets<StandardMaterial>,
    material: &MeshMaterial3d<StandardMaterial>,
    mut transform: Mut<Transform>,
    mut visibility: Mut<Visibility>,
) {
    if let Some(texture) = animation.textures.get(part.texture_index) {
        set_layer_texture(materials, &material.0, texture);
    }
    let color = layer_color(tint, part.color);
    if let Some(mut material) = materials.get_mut(&material.0) {
        material.base_color = color;
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

/// Channel-wise sRGBA multiply, alpha included.
fn layer_color(tint: Color, layer: Color) -> Color {
    let (a, b) = (tint.to_srgba(), layer.to_srgba());
    Color::srgba(
        a.red * b.red,
        a.green * b.green,
        a.blue * b.blue,
        a.alpha * b.alpha,
    )
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
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<EffectSpriteAssets>,
    mut bank: SpriteAssetBank,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Option<Res<SharedSpriteQuad>>,
    requests: Query<(
        Entity,
        &EffectSprite,
        Option<&EffectSpriteAction>,
        Has<EffectSpriteOnce>,
    )>,
) {
    let Some(shared_quad) = shared_quad else {
        return;
    };

    for (entity, request, selected_action, once) in &requests {
        let action_index = selected_action.map_or(EFFECT_SPRITE_ACTION, |action| action.0);
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

        let Some(animation_data) = bank.animations.get(&animation) else {
            continue;
        };
        if animation_data.actions.get(action_index).is_none() {
            warn!(
                "Effect sprite {} has no action {action_index}; skipping visual",
                request.path
            );
            commands.entity(entity).remove::<EffectSprite>();
            continue;
        };

        let parts = animation_data
            .actions
            .iter()
            .flat_map(|action| &action.frames)
            .map(|frame| frame.parts.len())
            .max()
            .unwrap_or(0);

        let z_offset = layer_order(LAYER_EFFECT) as f32 * Z_OFFSET_PER_LAYER;
        for part in 0..parts {
            let material = materials.add(StandardMaterial {
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
                    tint: request.tint,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, z_offset + part as f32 * 0.001)),
                Visibility::Hidden,
                // A ground cell's flat click collider is the only click target;
                // the visual must never swallow the ray.
                Pickable::IGNORE,
                ChildOf(entity),
            ));
        }

        if once {
            commands
                .entity(entity)
                .insert(EffectSpriteStart(time.elapsed_secs() * 1000.0));
        }
        commands.entity(entity).remove::<EffectSprite>();
    }
}

type UnplayedOneShot = (
    With<EffectSpriteOnce>,
    Without<EffectSprite>,
    Without<EffectSpriteStart>,
);

/// Despawns a one-shot root whose request resolved without spawning parts (a
/// failed load or a missing action), which would otherwise live forever.
pub fn despawn_unplayed_one_shot_sprites(
    mut commands: Commands,
    unplayed: Query<Entity, UnplayedOneShot>,
) {
    for entity in &unplayed {
        commands.entity(entity).try_despawn();
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
/// the selected ACT action. A part with no data at the current frame hides
/// rather than showing stale geometry.
pub fn sync_effect_sprites(
    mut commands: Commands,
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected_actions: Query<&EffectSpriteAction>,
    starts: Query<&EffectSpriteStart>,
    mut parts: Query<(
        &EffectSpritePart,
        &ChildOf,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let game_time_ms = time.elapsed_secs() * 1000.0;

    for (sprite_part, parent, material_handle, transform, mut visibility) in &mut parts {
        let Some(animation) = animations.get(&sprite_part.animation) else {
            continue;
        };
        let action_index = selected_actions
            .get(parent.parent())
            .map_or(EFFECT_SPRITE_ACTION, |action| action.0);
        let Some(action) = animation.actions.get(action_index) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let delay = action.delay_ms.max(1.0);
        let frame_index = match starts.get(parent.parent()) {
            Ok(start) => {
                let index = ((game_time_ms - start.0) / delay) as usize;
                if index >= action.frames.len() {
                    commands.entity(parent.parent()).try_despawn();
                    continue;
                }
                index
            }
            Err(_) if action.frames.is_empty() => {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
            Err(_) => (game_time_ms / delay) as usize % action.frames.len(),
        };
        let frame = &action.frames[frame_index];

        let Some(part) = frame.parts.get(sprite_part.part) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        apply_animation_part(
            part,
            animation,
            sprite_part.tint,
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
    fn selected_action_controls_both_part_spawning_and_playback() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin))
            .init_resource::<EffectSpriteAssets>()
            .add_systems(Update, (spawn_effect_sprites, sync_effect_sprites).chain());
        let texture = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let animation = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(RoAnimationAsset {
                textures: vec![texture.clone()],
                actions: vec![
                    ActionData {
                        frames: vec![],
                        delay_ms: 100.0,
                    },
                    ActionData {
                        frames: vec![FrameData {
                            parts: vec![FramePart {
                                texture_index: 0,
                                transform: Mat4::IDENTITY,
                                position: Vec2::ZERO,
                                scale: Vec2::ONE,
                                texture_size: Vec2::ONE,
                                color: Color::WHITE,
                                mirror: false,
                            }],
                            ..default()
                        }],
                        delay_ms: 100.0,
                    },
                ],
                layer: LAYER_EFFECT,
                sounds: vec![],
            });
        app.world_mut()
            .resource_mut::<EffectSpriteAssets>()
            .ready
            .insert("selected".into(), animation);
        let root = app
            .world_mut()
            .spawn((
                EffectSprite {
                    path: "selected".into(),
                    tint: Color::WHITE,
                },
                EffectSpriteAction(1),
            ))
            .id();
        app.update();
        let children = app
            .world()
            .get::<Children>(root)
            .expect("selected action spawns a part");
        assert_eq!(children.len(), 1);
        let child = children.iter().next().unwrap();
        assert_eq!(
            *app.world().get::<Visibility>(child).unwrap(),
            Visibility::Inherited
        );
        let material = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(child)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
                .unwrap()
                .base_color_texture,
            Some(texture)
        );
        app.world_mut()
            .entity_mut(root)
            .insert(EffectSpriteAction(0));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(child).unwrap(),
            Visibility::Hidden
        );
        app.world_mut()
            .entity_mut(root)
            .insert(EffectSpriteAction(1));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(child).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn one_shot_sprite_plays_its_frames_once_then_despawns() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin))
            .init_resource::<EffectSpriteAssets>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(
                Update,
                (
                    spawn_effect_sprites,
                    sync_effect_sprites,
                    despawn_unplayed_one_shot_sprites,
                )
                    .chain(),
            );
        let textures: Vec<_> = (0..2)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<Image>>()
                    .add(Image::default())
            })
            .collect();
        let frame = |texture_index| FrameData {
            parts: vec![FramePart {
                texture_index,
                transform: Mat4::IDENTITY,
                position: Vec2::ZERO,
                scale: Vec2::ONE,
                texture_size: Vec2::ONE,
                color: Color::WHITE,
                mirror: false,
            }],
            ..default()
        };
        let animation = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(RoAnimationAsset {
                textures: textures.clone(),
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
            .insert("once".into(), animation);
        // Spawn so the next update lands on an odd global frame: a looping sprite
        // would start on frame one there, a one-shot on frame zero.
        app.update();
        while !(app.world().resource::<Time>().elapsed().as_millis() / 100).is_multiple_of(2) {
            app.update();
        }
        let root = spawn_effect_sprite_once(
            &mut app.world_mut().commands(),
            "once",
            EffectAnchor::Position(Vec3::ZERO),
            Color::WHITE,
        );
        app.world_mut().flush();

        let texture_of_part = |app: &mut App| {
            let part = app
                .world_mut()
                .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<EffectSpritePart>>()
                .single(app.world())
                .unwrap()
                .0
                .clone();
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&part)
                .unwrap()
                .base_color_texture
                .clone()
        };

        app.update();
        assert_eq!(texture_of_part(&mut app), Some(textures[0].clone()));
        app.update();
        assert_eq!(texture_of_part(&mut app), Some(textures[1].clone()));
        app.update();
        app.update();
        assert!(
            app.world().get_entity(root).is_err(),
            "despawned after the last frame"
        );
    }

    #[test]
    fn act_layer_color_multiplies_into_the_request_tint() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin))
            .init_resource::<EffectSpriteAssets>()
            .add_systems(Update, (spawn_effect_sprites, sync_effect_sprites).chain());
        let texture = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        // torch_01's halo: a yellow glow authored at alpha 30/255.
        let halo = Color::srgba(1.0, 0.95, 0.3, 0.12);
        let animation = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(RoAnimationAsset {
                textures: vec![texture],
                actions: vec![ActionData {
                    frames: vec![FrameData {
                        parts: vec![FramePart {
                            texture_index: 0,
                            transform: Mat4::IDENTITY,
                            position: Vec2::ZERO,
                            scale: Vec2::ONE,
                            texture_size: Vec2::ONE,
                            color: halo,
                            mirror: false,
                        }],
                        ..default()
                    }],
                    delay_ms: 100.0,
                }],
                layer: LAYER_EFFECT,
                sounds: vec![],
            });
        app.world_mut()
            .resource_mut::<EffectSpriteAssets>()
            .ready
            .insert("halo".into(), animation);
        app.world_mut().spawn(EffectSprite {
            path: "halo".into(),
            tint: Color::srgba(0.5, 1.0, 1.0, 0.5),
        });
        app.update();

        let material = app
            .world_mut()
            .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<EffectSpritePart>>()
            .single(app.world())
            .unwrap()
            .0
            .clone();
        let color = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color
            .to_srgba();
        assert!((color.red - 0.5).abs() < 1e-4);
        assert!((color.blue - 0.3).abs() < 1e-4);
        assert!((color.alpha - 0.06).abs() < 1e-4);
    }

    #[test]
    fn one_shot_sprite_that_cannot_play_is_despawned() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, despawn_unplayed_one_shot_sprites);
        let root = app.world_mut().spawn(EffectSpriteOnce).id();
        app.update();
        assert!(app.world().get_entity(root).is_err());
    }

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
