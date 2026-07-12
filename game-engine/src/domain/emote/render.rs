use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use net_contract::events::EmoteShown;

use super::assets::EmoteAssets;
use super::table::emote_sound;
use crate::domain::audio::events::PlaySkillSfx;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::infrastructure::assets::ro_animation_asset::{FramePart, RoAnimationAsset};
use crate::utils::constants::SPRITE_WORLD_SCALE;

/// Local child offset lifting the emote quad above the unit's head. World up is
/// -Y, so this is more negative than the body's `SPRITE_BASE_Y_OFFSET` (-7.5).
const ABOVE_HEAD_Y: f32 = -12.0;

/// A playing above-head emote quad. `elapsed` is seconds since spawn;
/// `action_index` is the `emote_type` this quad renders (one action per emote,
/// `emotion.act` being direction-less). The quad plays its action once and is
/// despawned when the clock runs past the last frame.
#[derive(Component)]
pub struct ActiveEmote {
    action_index: usize,
    elapsed: f32,
}

/// Frame index for `elapsed_s` into a non-looping action, or `None` once the
/// action has played through (past its last frame) so the caller can despawn.
fn current_frame(elapsed_s: f32, delay_ms: f32, frame_count: usize) -> Option<usize> {
    if frame_count == 0 {
        return None;
    }
    let delay_s = delay_ms.max(1.0) / 1000.0;
    let index = (elapsed_s / delay_s) as usize;
    (index < frame_count).then_some(index)
}

/// First (only) part of a given action+frame; emote frames are single-part.
fn frame_part(
    animation: &RoAnimationAsset,
    action_index: usize,
    frame_index: usize,
) -> Option<&FramePart> {
    animation
        .actions
        .get(action_index)?
        .frames
        .get(frame_index)?
        .parts
        .first()
}

/// World-space quad scale for a sprite part, mirroring the body/cart sizing.
fn part_scale(part: &FramePart) -> Vec3 {
    let mut x = part.scale.x * part.texture_size.x * SPRITE_WORLD_SCALE;
    let y = part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE;
    if part.mirror {
        x = -x;
    }
    Vec3::new(x, y, 1.0)
}

/// Spawns a one-shot billboard for each inbound [`EmoteShown`]. Resolves the gid
/// to any registered unit (player, NPC, or mob), guards the action index, and
/// replaces (never stacks) an existing emote on the same unit. Skips silently
/// when the gid is unresolved, the assets are not ready yet, or the emote is out
/// of range.
#[allow(clippy::too_many_arguments)]
pub fn spawn_emote(
    mut events: MessageReader<EmoteShown>,
    registry: Res<EntityRegistry>,
    assets: Option<Res<EmoteAssets>>,
    animations: Res<Assets<RoAnimationAsset>>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<(Entity, &ChildOf), With<ActiveEmote>>,
    mut sfx: MessageWriter<PlaySkillSfx>,
    mut commands: Commands,
) {
    // target -> its current emote child, seeded from the world and kept live as
    // we spawn: deferred `commands.spawn` is invisible to `existing`, so two
    // events for the same gid in one frame would otherwise both miss and stack.
    let mut current: HashMap<Entity, Entity> = existing
        .iter()
        .map(|(child, child_of)| (child_of.parent(), child))
        .collect();

    for event in events.read() {
        let Some(target) = registry.get_entity(event.gid) else {
            continue;
        };
        let Some(assets) = assets.as_deref() else {
            continue;
        };
        let Some(animation) = animations.get(&assets.animation) else {
            continue;
        };

        let action_index = event.emote_type as usize;
        if action_index >= animation.actions.len() {
            continue;
        }

        if let Some(child) = current.remove(&target) {
            commands.entity(child).despawn();
        }

        let first = frame_part(animation, action_index, 0);
        let texture = first.and_then(|part| animation.textures.get(part.texture_index).cloned());
        let scale = first.map(part_scale).unwrap_or(Vec3::ONE);

        let material = materials.add(StandardMaterial {
            base_color_texture: texture,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        let child = commands
            .spawn((
                Mesh3d(shared_quad.mesh.clone()),
                MeshMaterial3d(material),
                Billboard,
                Transform {
                    translation: Vec3::new(0.0, ABOVE_HEAD_Y, 0.0),
                    scale,
                    ..default()
                },
                Visibility::default(),
                ActiveEmote {
                    action_index,
                    elapsed: 0.0,
                },
                ChildOf(target),
            ))
            .id();
        current.insert(target, child);

        if let Some(sound) = emote_sound(event.emote_type) {
            sfx.write(PlaySkillSfx {
                emitter: target,
                sound: sound.to_string(),
            });
        }
    }
}

/// Advances each [`ActiveEmote`] clock, drives its quad to the current frame's
/// texture and size, and despawns the child once the action has played through
/// once. Parenting via `ChildOf` means a unit that despawns takes its emote with
/// it, so there is no follow or orphan-cleanup path here.
pub fn advance_and_despawn_emotes(
    time: Res<Time>,
    assets: Option<Res<EmoteAssets>>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut emotes: Query<(
        Entity,
        &mut ActiveEmote,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
    )>,
    mut commands: Commands,
) {
    let Some(assets) = assets.as_deref() else {
        return;
    };
    let Some(animation) = animations.get(&assets.animation) else {
        return;
    };

    let dt = time.delta_secs();
    for (entity, mut emote, material, mut transform) in emotes.iter_mut() {
        emote.elapsed += dt;

        let Some(action) = animation.actions.get(emote.action_index) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(frame_index) = current_frame(emote.elapsed, action.delay_ms, action.frames.len())
        else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(part) = frame_part(animation, emote.action_index, frame_index) else {
            continue;
        };

        if let Some(texture) = animation.textures.get(part.texture_index) {
            set_layer_texture(&mut materials, &material.0, texture);
        }
        transform.scale = part_scale(part);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::billboard::create_sprite_quad_mesh;
    use crate::infrastructure::assets::ro_animation_asset::{ActionData, FrameData};

    fn part(texture_index: usize) -> FramePart {
        FramePart {
            texture_index,
            transform: Mat4::IDENTITY,
            position: Vec2::ZERO,
            scale: Vec2::ONE,
            texture_size: Vec2::splat(32.0),
            color: Color::WHITE,
            mirror: false,
        }
    }

    fn action(frame_count: usize, delay_ms: f32) -> ActionData {
        ActionData {
            frames: (0..frame_count)
                .map(|_| FrameData {
                    parts: vec![part(0)],
                    ..default()
                })
                .collect(),
            delay_ms,
        }
    }

    fn animation(actions: usize) -> RoAnimationAsset {
        RoAnimationAsset {
            textures: vec![Handle::default()],
            actions: (0..actions).map(|_| action(2, 100.0)).collect(),
            ..default()
        }
    }

    #[test]
    fn current_frame_none_without_frames() {
        assert_eq!(current_frame(0.0, 100.0, 0), None);
    }

    #[test]
    fn current_frame_starts_at_zero() {
        assert_eq!(current_frame(0.0, 100.0, 2), Some(0));
    }

    #[test]
    fn current_frame_advances_at_delay_boundary() {
        assert_eq!(current_frame(0.05, 100.0, 2), Some(0));
        assert_eq!(current_frame(0.1, 100.0, 2), Some(1));
    }

    #[test]
    fn current_frame_finishes_past_last_frame() {
        assert_eq!(current_frame(0.2, 100.0, 2), None);
    }

    #[test]
    fn part_scale_applies_world_scale_and_mirror() {
        let mut p = part(0);
        p.texture_size = Vec2::new(10.0, 20.0);
        let scale = part_scale(&p);
        assert_eq!(scale, Vec3::new(2.0, 4.0, 1.0));

        p.mirror = true;
        assert_eq!(part_scale(&p).x, -2.0);
    }

    #[test]
    fn frame_part_guards_out_of_range() {
        let anim = animation(1);
        assert!(frame_part(&anim, 0, 0).is_some());
        assert!(frame_part(&anim, 5, 0).is_none());
        assert!(frame_part(&anim, 0, 9).is_none());
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<RoAnimationAsset>()
            .add_message::<EmoteShown>()
            .add_message::<PlaySkillSfx>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, spawn_emote);

        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(create_sprite_quad_mesh());
        app.insert_resource(SharedSpriteQuad { mesh });

        let handle = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(animation(3));
        app.insert_resource(EmoteAssets {
            animation: handle,
            thumbnails: Vec::new(),
        });
        app
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit(app: &mut App, gid: u32, emote_type: u32) {
        app.world_mut()
            .resource_mut::<Messages<EmoteShown>>()
            .write(EmoteShown { gid, emote_type });
        app.update();
    }

    fn emote_children(app: &mut App, parent: Entity) -> Vec<Entity> {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &ChildOf), With<ActiveEmote>>();
        query
            .iter(app.world())
            .filter(|(_, child_of)| child_of.parent() == parent)
            .map(|(entity, _)| entity)
            .collect()
    }

    #[test]
    fn resolved_emote_spawns_one_child() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, 7, 1);

        assert_eq!(emote_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn second_emote_replaces_not_stacks() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, 7, 1);
        emit(&mut app, 7, 2);

        assert_eq!(emote_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn two_same_frame_emotes_replace_not_stack() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        {
            let mut messages = app.world_mut().resource_mut::<Messages<EmoteShown>>();
            messages.write(EmoteShown {
                gid: 7,
                emote_type: 1,
            });
            messages.write(EmoteShown {
                gid: 7,
                emote_type: 2,
            });
        }
        app.update();

        let children = emote_children(&mut app, unit);
        assert_eq!(children.len(), 1);
        assert_eq!(
            app.world()
                .get::<ActiveEmote>(children[0])
                .unwrap()
                .action_index,
            2
        );
    }

    #[test]
    fn unresolved_gid_spawns_nothing() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, 999, 1);

        assert!(emote_children(&mut app, unit).is_empty());
    }

    #[test]
    fn out_of_range_emote_spawns_nothing() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, 7, 88);

        assert!(emote_children(&mut app, unit).is_empty());
    }

    #[test]
    fn advance_despawns_after_play_through() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .init_asset::<RoAnimationAsset>()
            .add_systems(Update, advance_and_despawn_emotes);

        let handle = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(animation(3));
        app.insert_resource(EmoteAssets {
            animation: handle,
            thumbnails: Vec::new(),
        });
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());

        let emote = app
            .world_mut()
            .spawn((
                ActiveEmote {
                    action_index: 0,
                    elapsed: 1.0,
                },
                MeshMaterial3d(material),
                Transform::default(),
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(emote).is_err());
    }
}
