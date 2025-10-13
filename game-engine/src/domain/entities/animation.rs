use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::{
    convert_sprite_frame_to_rgba, create_bevy_image, RoActAsset, RoPaletteAsset, RoSpriteAsset,
};
use crate::utils::constants::SPRITE_WORLD_SCALE;
use bevy::prelude::*;

/// Animation system for RO sprites - supports palettes and configurable looping
/// Includes special handling for head layers with doridori animation
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
        let sprite_asset = ro_sprites.get(&controller.sprite_handle);
        let action_asset = ro_actions.get(&controller.action_handle);

        if sprite_asset.is_none() || action_asset.is_none() {
            continue;
        }

        if let (Some(sprite_asset), Some(action_asset)) = (sprite_asset, action_asset) {
            let sprite = &sprite_asset.sprite;
            let action = &action_asset.action;

            // Update timer only if not paused
            if !controller.paused {
                controller.timer += time.delta().as_millis() as f32;
            }

            // Check if we need to advance to next frame
            if controller.timer >= controller.current_delay {
                controller.timer = 0.0;

                // Get current action sequence
                if let Some(action_seq) = action.actions.get(controller.action_index) {
                    // Check if this is a head layer during IDLE action
                    // Head animations have 3x frames for doridori (head nodding), only use first 1/3
                    let is_head_layer = sprite_layer.is_some_and(|layer| {
                        use crate::domain::entities::character::components::visual::SpriteLayerType;
                        use crate::domain::entities::character::components::equipment::EquipmentSlot;
                        matches!(
                            layer.layer_type,
                            SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
                                | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
                                | SpriteLayerType::Equipment(EquipmentSlot::HeadTop)
                        )
                    });
                    let is_idle = controller.action_index == 0;

                    // Calculate effective frame count
                    let frame_count = if is_head_layer && is_idle {
                        // Divide by 3 to skip doridori variants (headDir 1 and 2)
                        action_seq.animations.len() / 3
                    } else {
                        action_seq.animations.len()
                    };

                    // Advance animation index
                    controller.animation_index += 1;
                    if controller.animation_index >= frame_count {
                        if controller.loop_animation {
                            controller.animation_index = 0;
                        } else {
                            // Stop at last frame if not looping
                            controller.animation_index = frame_count.saturating_sub(1);
                        }
                    }

                    // Update delay for next frame
                    controller.current_delay = action_seq.delay;
                }
            }

            // Get current animation and its first layer to determine sprite frame
            if let Some(action_seq) = action.actions.get(controller.action_index) {
                if let Some(animation) = action_seq.animations.get(controller.animation_index) {
                    if let Some(first_layer) = animation.layers.first() {
                        // Handle negative sprite indices (use index 0 as fallback)
                        let sprite_index = if first_layer.sprite_index < 0 {
                            0
                        } else {
                            first_layer.sprite_index as usize
                        };

                        // Ensure sprite index is valid
                        if let Some(sprite_frame) = sprite.frames.get(sprite_index) {
                            // Get custom palette if provided
                            let custom_palette = controller
                                .palette_handle
                                .as_ref()
                                .and_then(|handle| ro_palettes.get(handle));

                            // Convert sprite frame to RGBA using shared utility
                            let rgba_data = convert_sprite_frame_to_rgba(
                                sprite_frame,
                                sprite.palette.as_ref(),
                                custom_palette,
                            );

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

                            // Update frame index for tracking
                            controller.frame_index = sprite_index;
                        }
                    }
                }
            }
        }
    }
}
