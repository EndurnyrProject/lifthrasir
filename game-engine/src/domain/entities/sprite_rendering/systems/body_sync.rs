use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadLayer, MobSprite, PlayerSprite, RenderLayer, RoSpriteGeneric,
};
use crate::domain::entities::sprite_rendering::layout::ActionLayout;
use crate::domain::sprite::tags::SPRITE_BASE_Y_OFFSET;
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

fn sync_body_layer_impl<T: ActionLayout>(
    game_time_ms: u32,
    animations: &Res<Assets<RoAnimationAsset>>,
    materials: &mut Assets<StandardMaterial>,
    parent_query: &Query<&RoSpriteGeneric<T>>,
    layer_query: &mut Query<
        (
            &RenderLayer,
            &ChildOf,
            &MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut BodyAttachPoint,
        ),
        Without<HeadLayer>,
    >,
) {
    for (layer, child_of, material_handle, mut transform, mut attach_point) in
        layer_query.iter_mut()
    {
        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        let frame_index = ro_sprite.get_frame_index(animation, game_time_ms);
        let Some(frame) = ro_sprite.get_frame(animation, game_time_ms) else {
            continue;
        };

        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                if let Some(material) = materials.get_mut(&material_handle.0) {
                    material.base_color_texture = Some(texture.clone());
                }
            }

            let sprite_width = part.texture_size.x;
            let sprite_height = part.texture_size.y;

            let mut scale_x = part.scale.x * sprite_width * SPRITE_WORLD_SCALE;
            let scale_y = part.scale.y * sprite_height * SPRITE_WORLD_SCALE;

            if part.mirror {
                scale_x = -scale_x;
            }

            transform.scale = Vec3::new(scale_x, scale_y, 1.0);

            // Apply layer position offset plus base Y offset to lift sprite above ground
            transform.translation.x = part.position.x * SPRITE_WORLD_SCALE;
            transform.translation.y = part.position.y * SPRITE_WORLD_SCALE + SPRITE_BASE_Y_OFFSET;

            attach_point.layer_pos = part.position;
        }

        attach_point.frame_index = frame_index;
        if let Some(ap) = frame.attach_point {
            attach_point.attach_point = ap;
        }
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
    parent_query: Query<&PlayerSprite>,
    mut layer_query: Query<
        (
            &RenderLayer,
            &ChildOf,
            &MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut BodyAttachPoint,
        ),
        Without<HeadLayer>,
    >,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
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
    parent_query: Query<&MobSprite>,
    mut layer_query: Query<
        (
            &RenderLayer,
            &ChildOf,
            &MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut BodyAttachPoint,
        ),
        Without<HeadLayer>,
    >,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
    );
}
