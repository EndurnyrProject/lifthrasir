use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::{
    convert_sprite_frame_to_rgba, create_bevy_image, RoActAsset, RoPaletteAsset, RoSpriteAsset,
};
use crate::utils::constants::SPRITE_WORLD_SCALE;
use bevy::prelude::*;

/// Helper struct to hold validated animation assets
struct AnimationAssets<'a> {
    sprite: &'a RoSpriteAsset,
    action: &'a RoActAsset,
}

/// Retrieves and validates animation assets for a controller
/// Returns None if assets aren't loaded
fn get_animation_assets<'a>(
    controller: &RoAnimationController,
    sprites: &'a Assets<RoSpriteAsset>,
    actions: &'a Assets<RoActAsset>,
) -> Option<AnimationAssets<'a>> {
    let sprite = sprites.get(&controller.sprite_handle)?;
    let action = actions.get(&controller.action_handle)?;
    Some(AnimationAssets { sprite, action })
}

/// Checks if a sprite layer is a head layer during idle animation
/// Head layers have special doridori (head nodding) animation handling
fn is_head_layer_during_idle(
    sprite_layer: Option<&crate::domain::entities::character::components::visual::RoSpriteLayer>,
    action_index: usize,
) -> bool {
    let is_idle = action_index == 0;
    sprite_layer.is_some_and(|layer| {
        use crate::domain::entities::character::components::equipment::EquipmentSlot;
        use crate::domain::entities::character::components::visual::SpriteLayerType;
        matches!(
            layer.layer_type,
            SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadTop)
        )
    }) && is_idle
}

/// Calculates the effective frame count for an animation sequence
/// Head layers during idle divide by 3 to skip doridori variants (headDir 1 and 2)
fn calculate_effective_frame_count(
    action_seq: &crate::infrastructure::ro_formats::act::ActionSequence,
    is_head_layer_idle: bool,
) -> usize {
    if is_head_layer_idle {
        // Divide by 3 to skip doridori variants (headDir 1 and 2)
        action_seq.animations.len() / 3
    } else {
        action_seq.animations.len()
    }
}

/// Advances the animation frame based on elapsed time
fn advance_animation_frame(
    controller: &mut RoAnimationController,
    action: &crate::infrastructure::ro_formats::RoAction,
    sprite_layer: Option<&crate::domain::entities::character::components::visual::RoSpriteLayer>,
    time: &Time,
) {
    if !controller.paused {
        controller.timer += time.delta().as_millis() as f32;
    }

    if controller.timer < controller.current_delay {
        return;
    }

    controller.timer = 0.0;

    let Some(action_seq) = action.actions.get(controller.action_index) else {
        return;
    };

    let is_head_idle = is_head_layer_during_idle(sprite_layer, controller.action_index);
    let frame_count = calculate_effective_frame_count(action_seq, is_head_idle);

    controller.animation_index += 1;
    if controller.animation_index >= frame_count {
        controller.animation_index = if controller.loop_animation {
            0
        } else {
            frame_count.saturating_sub(1)
        };
    }

    controller.current_delay = action_seq.delay;
}

/// Renders the current animation frame for an entity
fn render_current_frame(
    entity: Entity,
    controller: &mut RoAnimationController,
    assets: &AnimationAssets,
    transform: &Transform,
    ro_palettes: &Assets<RoPaletteAsset>,
    images: &mut Assets<Image>,
    commands: &mut Commands,
) {
    let sprite = &assets.sprite.sprite;
    let action = &assets.action.action;
    let Some(action_seq) = action.actions.get(controller.action_index) else {
        return;
    };

    let Some(animation) = action_seq.animations.get(controller.animation_index) else {
        return;
    };

    let Some(first_layer) = animation.layers.first() else {
        return;
    };

    let sprite_index = first_layer.sprite_index.max(0) as usize;

    let Some(sprite_frame) = sprite.frames.get(sprite_index) else {
        return;
    };

    let custom_palette = controller
        .palette_handle
        .as_ref()
        .and_then(|handle| ro_palettes.get(handle));

    let rgba_data =
        convert_sprite_frame_to_rgba(sprite_frame, sprite.palette.as_ref(), custom_palette);

    let bevy_image = create_bevy_image(
        sprite_frame.width as u32,
        sprite_frame.height as u32,
        rgba_data,
    );

    let image_handle = images.add(bevy_image);

    // Apply ACT offset relative to parent (not accumulated)
    // ACT pos is a STATIC offset from character anchor, not a delta
    // ACT offsets are in pixel coordinates, scale to world units
    let layer_offset = Vec3::new(
        first_layer.pos[0] as f32 * SPRITE_WORLD_SCALE,
        -first_layer.pos[1] as f32 * SPRITE_WORLD_SCALE, // Negate Y: RO Y-negative=up, Bevy Y-positive=up
        transform.translation.z, // Preserve Z-layering (0.0 for body, 0.1 for head)
    );

    commands.entity(entity).insert((
        Sprite::from_image(image_handle),
        Transform::from_translation(layer_offset),
    ));

    controller.frame_index = sprite_index;
}

/// Animation system for RO sprites - supports palettes and configurable looping
pub fn animate_sprites(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoAnimationController,
        &Transform,
        Option<&crate::domain::entities::character::components::visual::RoSpriteLayer>,
    )>,
    ro_sprites: Res<Assets<RoSpriteAsset>>,
    ro_actions: Res<Assets<RoActAsset>>,
    ro_palettes: Res<Assets<RoPaletteAsset>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    for (entity, mut controller, transform, sprite_layer) in query.iter_mut() {
        let Some(assets) = get_animation_assets(&controller, &ro_sprites, &ro_actions) else {
            continue;
        };

        let action = &assets.action.action;

        advance_animation_frame(&mut controller, action, sprite_layer, &time);

        render_current_frame(
            entity,
            &mut controller,
            &assets,
            transform,
            &ro_palettes,
            &mut images,
            &mut commands,
        );
    }
}
