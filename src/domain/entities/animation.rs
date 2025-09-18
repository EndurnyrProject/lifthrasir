use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::{
    RoActAsset, RoPaletteAsset, RoSpriteAsset, calculate_sprite_scale,
    convert_sprite_frame_to_rgba, create_bevy_image,
};
use bevy::prelude::*;
use bevy_lunex::UiLayout;

/// Animation system for RO sprites - supports palettes and configurable looping
pub fn animate_sprites(
    mut commands: Commands,
    mut query: Query<(Entity, &mut RoAnimationController, &Transform, Option<&UiLayout>)>,
    ro_sprites: Res<Assets<RoSpriteAsset>>,
    ro_actions: Res<Assets<RoActAsset>>,
    ro_palettes: Res<Assets<RoPaletteAsset>>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    for (entity, mut controller, _transform, ui_layout) in query.iter_mut() {
        let sprite_asset = ro_sprites.get(&controller.sprite_handle);
        let action_asset = ro_actions.get(&controller.action_handle);

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
                    // Advance animation index
                    controller.animation_index += 1;
                    if controller.animation_index >= action_seq.animations.len() {
                        if controller.loop_animation {
                            controller.animation_index = 0;
                        } else {
                            // Stop at last frame if not looping
                            controller.animation_index = action_seq.animations.len() - 1;
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

                            // For UI sprites (with UiLayout), apply ACT layer offset to Transform
                            // RO uses Y-negative = up, Bevy uses Y-positive = up, so negate Y
                            if ui_layout.is_some() {
                                // UI sprite: apply ACT offset with Y negation
                                let layer_offset = Vec3::new(
                                    first_layer.pos[0] as f32,
                                    -first_layer.pos[1] as f32, // Negate Y for Bevy coordinate system
                                    _transform.translation.z,   // Preserve Z for layering
                                );

                                commands.entity(entity).insert((
                                    Sprite::from_image(image_handle),
                                    Transform::from_translation(layer_offset),
                                ));
                            } else {
                                // World sprite: update both sprite and transform with ACT offset
                                let scale = calculate_sprite_scale(
                                    sprite_frame.width as u32,
                                    sprite_frame.height as u32,
                                );

                                let layer_offset = Vec3::new(
                                    first_layer.pos[0] as f32,
                                    first_layer.pos[1] as f32,
                                    0.0,
                                );

                                let new_transform =
                                    Transform::from_translation(_transform.translation + layer_offset)
                                        .with_scale(Vec3::splat(scale));

                                commands
                                    .entity(entity)
                                    .insert((Sprite::from_image(image_handle), new_transform));
                            }

                            // Update frame index for tracking
                            controller.frame_index = sprite_index;
                        }
                    }
                }
            }
        }
    }
}
