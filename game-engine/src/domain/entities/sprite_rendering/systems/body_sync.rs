use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::audio::events::PlayMobSfx;
use crate::domain::effects::AnimationPaused;
use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadLayer, MobSprite, PlayerSprite, RenderLayer, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

type BodyLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut BodyAttachPoint,
    ),
    Without<HeadLayer>,
>;

fn sync_body_layer_impl<T: ActionLayout>(
    game_time_ms: u32,
    animations: &Res<Assets<RoAnimationAsset>>,
    materials: &mut Assets<StandardMaterial>,
    parent_query: &Query<(&RoSpriteGeneric<T>, Option<&AnimationPaused>)>,
    layer_query: &mut BodyLayerQuery,
    mut sfx: Option<&mut MessageWriter<PlayMobSfx>>,
) {
    for (layer, child_of, material_handle, mut transform, mut attach_point) in
        layer_query.iter_mut()
    {
        let Ok((ro_sprite, paused)) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // A frozen/petrified unit holds its animation on the frame that was
        // showing when the pause began: feed that captured timestamp instead of
        // the live clock. The head and weapon layers ride the body's published
        // frame index, so freezing the body alone holds the whole character.
        let effective_time = paused.map_or(game_time_ms, |p| p.at_ms);

        let frame_index = ro_sprite.get_frame_index(animation, effective_time);
        let Some(frame) = ro_sprite.get_frame(animation, effective_time) else {
            continue;
        };

        // Fire once when crossing into a new frame that carries a sound id.
        // `as_mut()` reborrows the Option each iteration (MessageWriter is not DerefMut).
        if let Some(writer) = sfx.as_mut() {
            if frame_index != attach_point.frame_index {
                if let Some(name) = frame
                    .sound_id
                    .and_then(|id| animation.sounds.get(id as usize))
                    .filter(|name| !name.is_empty())
                {
                    writer.write(PlayMobSfx {
                        emitter: child_of.parent(),
                        sound: name.clone(),
                    });
                }
            }
        }

        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                set_layer_texture(materials, &material_handle.0, texture);
            }

            let sprite_width = part.texture_size.x;
            let sprite_height = part.texture_size.y;

            let mut scale_x = part.scale.x * sprite_width * SPRITE_WORLD_SCALE;
            let scale_y = part.scale.y * sprite_height * SPRITE_WORLD_SCALE;

            if part.mirror {
                scale_x = -scale_x;
            }

            // RO authors each frame so the sprite, drawn centered at its ACT
            // `position`, lands its feet on the ground anchor. `position` was
            // Y-negated on extraction into a Y-up space, and world up is -Y, so we
            // negate again to place the center: taller sprites carry a larger
            // `position.y` and are lifted more, which is what grounds them.
            let current = *transform;
            transform.set_if_neq(Transform {
                scale: Vec3::new(scale_x, scale_y, 1.0),
                translation: Vec3::new(
                    part.position.x * SPRITE_WORLD_SCALE,
                    -part.position.y * SPRITE_WORLD_SCALE,
                    current.translation.z,
                ),
                ..current
            });
        }

        attach_point.set_if_neq(BodyAttachPoint {
            attach_point: frame.attach_point.unwrap_or(attach_point.attach_point),
            frame_index,
            layer_pos: frame
                .parts
                .first()
                .map_or(attach_point.layer_pos, |part| part.position),
        });
    }
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_player_body_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<(&PlayerSprite, Option<&AnimationPaused>)>,
    mut layer_query: BodyLayerQuery,
    mut sfx_writer: MessageWriter<PlayMobSfx>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
        Some(&mut sfx_writer),
    );
}

#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_mob_body_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<(&MobSprite, Option<&AnimationPaused>)>,
    mut layer_query: BodyLayerQuery,
    mut sfx_writer: MessageWriter<PlayMobSfx>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
        Some(&mut sfx_writer),
    );
}
