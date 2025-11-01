use super::super::components::{RoSpriteLayer, SpriteHierarchy, SpriteLayerType};
use crate::domain::assets::patterns::{
    body_action_path, body_sprite_path, head_action_path, head_sprite_path,
};
use crate::domain::character::JobClass;
use crate::domain::entities::animation::frame_cache::{FrameCacheKey, RoFrameCache};
use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::diagnostics::AnimationDiagnostics;
use crate::utils::constants::SPRITE_WORLD_SCALE;
use bevy::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    pbr::MeshMaterial3d,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// Query type for sprite layers that need asset population
type SpriteLayersNeedingAssets<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SpriteHierarchy, &'static RoSpriteLayer),
    Without<RoAnimationController>,
>;

/// Query type for character sprites with animation state
type CharacterSpriteQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static crate::domain::entities::character::components::visual::CharacterSprite,
        &'static super::super::components::SpriteObjectTree,
    ),
    (
        With<crate::domain::entities::character::components::CharacterAppearance>,
        Changed<crate::domain::entities::character::components::visual::CharacterSprite>,
    ),
>;

/// Query type for entities with changed direction (non-PC)
type ChangedDirectionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static crate::domain::entities::character::components::visual::CharacterDirection,
        &'static super::super::components::SpriteObjectTree,
    ),
    (
        Changed<crate::domain::entities::character::components::visual::CharacterDirection>,
        Without<crate::domain::entities::character::components::CharacterAppearance>,
    ),
>;

/// Helper function to convert sprite frame to Bevy image
fn convert_sprite_frame_to_image(
    sprite_frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
    sprite_palette: &Option<crate::infrastructure::ro_formats::sprite::Palette>,
) -> Image {
    let mut rgba_data =
        Vec::with_capacity((sprite_frame.width as usize) * (sprite_frame.height as usize) * 4);

    if sprite_frame.is_rgba {
        rgba_data.extend_from_slice(&sprite_frame.data);
    } else if let Some(palette) = sprite_palette {
        for &index in &sprite_frame.data {
            if (index as usize) < palette.colors.len() {
                let color = palette.colors[index as usize];
                rgba_data.extend_from_slice(&color);
            } else {
                rgba_data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    } else {
        for &index in &sprite_frame.data {
            rgba_data.extend_from_slice(&[index, index, index, 255]);
        }
    }

    Image::new(
        Extent3d {
            width: sprite_frame.width as u32,
            height: sprite_frame.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Helper function to create sprite texture image for current animation frame
fn create_sprite_texture_image(
    sprite_frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
    sprite_palette: &Option<crate::infrastructure::ro_formats::sprite::Palette>,
    _layer: &crate::infrastructure::ro_formats::act::Layer,
) -> Image {
    convert_sprite_frame_to_image(sprite_frame, sprite_palette)
}

/// Helper function to calculate correct sprite index for head layers
/// Head sprites use 8-directional indexing (0-7), unlike body which uses action indices
fn calculate_head_sprite_index(action_index: usize) -> usize {
    // Extract direction from action index
    let direction = action_index % 8;
    match direction {
        0 => 0, // South
        1 => 1, // SouthWest
        2 => 2, // West
        3 => 3, // NorthWest
        4 => 4, // North
        5 => 3, // NorthEast → use NorthWest sprite (flipped by transform)
        6 => 2, // East → use West sprite (flipped by transform)
        7 => 1, // SouthEast → use SouthWest sprite (flipped by transform)
        _ => 0,
    }
}

/// System to update sprite layer transforms based on animation
/// Phase 3: Enhanced with PC-specific logic (head direction, anchor offsets) and frame caching
#[allow(clippy::too_many_arguments)]
pub fn update_sprite_transforms(
    mut sprite_layers: Query<(
        Entity,
        &mut Transform,
        &SpriteHierarchy,
        &RoAnimationController,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    spr_assets: Res<Assets<RoSpriteAsset>>,
    act_assets: Res<Assets<RoActAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut frame_cache: ResMut<RoFrameCache>,
    mut diagnostics: ResMut<AnimationDiagnostics>,
) {
    for (_entity, mut transform, hierarchy, controller, material_handle) in sprite_layers.iter_mut()
    {
        let Some(spr_asset) = spr_assets.get(&controller.sprite_handle) else {
            continue;
        };

        let Some(act_asset) = act_assets.get(&controller.action_handle) else {
            continue;
        };

        let current_action = controller.action_index;
        if current_action >= act_asset.action.actions.len() {
            continue;
        }

        let action_sequence = &act_asset.action.actions[current_action];
        let current_frame = controller.animation_index % action_sequence.animations.len().max(1);
        let animation = &action_sequence.animations[current_frame];

        if animation.layers.is_empty() {
            continue;
        }

        let layer = &animation.layers[0];

        // For head layers, calculate sprite index based on direction
        // Head ACT files have sprite_index=0 for all frames, so we must derive it from action
        let sprite_index = match &hierarchy.layer_type {
            SpriteLayerType::Head => calculate_head_sprite_index(current_action),
            _ => {
                // Body and other layers use sprite_index from ACT file
                if layer.sprite_index < 0 {
                    0
                } else {
                    layer.sprite_index as usize
                }
            }
        };

        if sprite_index >= spr_asset.sprite.frames.len() {
            warn!(
                "Invalid sprite_index for {:?}: index={} (must be < {})",
                hierarchy.layer_type,
                sprite_index,
                spr_asset.sprite.frames.len()
            );
            continue;
        }

        let sprite_frame = &spr_asset.sprite.frames[sprite_index];
        let sprite_palette = &spr_asset.sprite.palette;

        let offset_x = layer.pos[0] as f32 * SPRITE_WORLD_SCALE;
        let offset_y = -layer.pos[1] as f32 * SPRITE_WORLD_SCALE;

        let sprite_width = if layer.width > 0 {
            layer.width as f32
        } else {
            sprite_frame.width as f32
        };

        let sprite_height = if layer.height > 0 {
            layer.height as f32
        } else {
            sprite_frame.height as f32
        };

        let mut scale_x = layer.scale[0] * sprite_width * SPRITE_WORLD_SCALE;
        let scale_y = layer.scale[1] * sprite_height * SPRITE_WORLD_SCALE;

        if layer.is_mirror {
            scale_x = -scale_x;
        }

        transform.translation.x = offset_x;
        transform.translation.y = offset_y;
        transform.scale = Vec3::new(scale_x, scale_y, 1.0);

        if layer.angle != 0 {
            transform.rotation =
                Quat::from_rotation_z(layer.angle as f32 * std::f32::consts::PI / 180.0);
        }

        // Update existing material's texture instead of creating a new one
        if let Some(material) = materials.get_mut(&material_handle.0) {
            // Create cache key for this frame
            let cache_key = FrameCacheKey::new(
                &controller.sprite_handle,
                sprite_index,
                controller.palette_handle.as_ref(),
            );

            // Check cache before converting
            let texture_handle = if let Some(cached_handle) = frame_cache.get(&cache_key) {
                // Cache HIT - reuse existing texture
                diagnostics.record_cache_hit();
                cached_handle
            } else {
                // Cache MISS - create and cache the texture
                diagnostics.record_cache_miss();
                diagnostics.record_conversion();

                let image = create_sprite_texture_image(sprite_frame, sprite_palette, layer);
                let handle = images.add(image);
                frame_cache.insert(cache_key, handle.clone());
                handle
            };

            // Update the material's texture and color
            material.base_color_texture = Some(texture_handle);
            material.base_color = Color::srgba(
                layer.color[0],
                layer.color[1],
                layer.color[2],
                layer.color[3],
            );
        }
    }
}

/// System to advance animations for generic sprites (mobs, NPCs, etc.)
pub fn advance_animations(
    time: Res<Time>,
    mut controllers: Query<(&mut RoAnimationController, &SpriteHierarchy)>,
    act_assets: Res<Assets<RoActAsset>>,
    parents: Query<&crate::domain::entities::character::components::CharacterAppearance>,
) {
    for (mut controller, hierarchy) in controllers.iter_mut() {
        if controller.paused {
            continue;
        }

        let Some(act_asset) = act_assets.get(&controller.action_handle) else {
            continue;
        };

        // Advance timer (in milliseconds to match RO convention)
        controller.timer += time.delta().as_millis() as f32;

        // Check if current delay has elapsed
        if controller.timer < controller.current_delay {
            continue;
        }

        // Get current action sequence
        let Some(action_sequence) = act_asset.action.actions.get(controller.action_index) else {
            continue;
        };

        let frame_count = action_sequence.animations.len();
        if frame_count == 0 {
            continue;
        }

        // Actions 0-7 are directional idle animations
        let is_idle = controller.action_index < 8;
        // Check if this sprite belongs to a PC (has CharacterAppearance)
        let is_pc_sprite = parents.get(hierarchy.parent_entity).is_ok();

        if is_idle && is_pc_sprite {
            // PCs: Lock idle at frame 0 to prevent doridori
            controller.animation_index = 0;
        } else {
            // Mobs and PC non-idle: Normal frame advancement
            controller.animation_index = (controller.animation_index + 1) % frame_count;
        }

        // Loop or stop at end
        if controller.animation_index == 0 && !controller.loop_animation {
            controller.paused = true;
        }

        // Update delay from action sequence (keep in milliseconds)
        controller.current_delay = action_sequence.delay.max(100.0);
        controller.timer = 0.0;
    }
}

/// System to sync CharacterSprite animation state to RoAnimationController components
/// This bridges the gap between the old CharacterSprite API and the new generic sprite system
/// Only affects PC characters; mobs/NPCs use RoAnimationController directly
pub fn sync_character_animations_to_controllers(
    characters: CharacterSpriteQuery,
    mut sprite_layers: Query<(&mut RoAnimationController, &SpriteHierarchy)>,
) {
    for (character_entity, character_sprite, _object_tree) in characters.iter() {
        let desired_action = character_sprite.current_action as usize;
        let desired_frame = character_sprite.current_frame as usize;

        // Find all sprite layers belonging to this character and sync their controllers
        for (mut controller, hierarchy) in sprite_layers.iter_mut() {
            if hierarchy.parent_entity != character_entity {
                continue;
            }

            // Only update if action changed to avoid unnecessary work
            if controller.action_index != desired_action {
                debug!(
                    "🎬 Syncing animation for {:?} layer {:?}: action {} -> {}",
                    character_entity, hierarchy.layer_type, controller.action_index, desired_action
                );

                controller.action_index = desired_action;
                controller.animation_index = desired_frame;
                controller.timer = 0.0;
            }
        }
    }
}

/// System to update RoAnimationController action index for non-PC entities based on direction changes
/// PCs use CharacterSprite and sync_character_animations_to_controllers instead
pub fn update_generic_sprite_direction(
    changed_entities: ChangedDirectionQuery,
    mut sprite_layers: Query<(&mut RoAnimationController, &SpriteHierarchy)>,
) {
    for (entity, direction, _object_tree) in changed_entities.iter() {
        // Find all sprite layers for this entity
        for (mut controller, hierarchy) in sprite_layers.iter_mut() {
            if hierarchy.parent_entity != entity {
                continue;
            }

            // Calculate new action index: (current_action / 8) * 8 + direction
            // This preserves the action type (idle, walk, etc.) while updating direction
            let base_action = controller.action_index / 8;
            let new_action_index = base_action * 8 + (direction.facing as usize);

            if controller.action_index != new_action_index {
                debug!(
                    "🔄 Updating direction for entity {:?}: action {} -> {} (direction={:?})",
                    entity, controller.action_index, new_action_index, direction.facing
                );

                controller.action_index = new_action_index;
                // Don't reset animation_index - preserve current frame for smooth transition
            }
        }
    }
}

/// System to populate sprite layers with assets
/// Phase 3: Enhanced to support PC body/head layers in addition to simple entities
pub fn populate_sprite_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_layers: SpriteLayersNeedingAssets,
    sprite_info_query: Query<&super::super::components::EntitySpriteInfo>,
) {
    let layer_count = sprite_layers.iter().count();

    if layer_count == 0 {
        return;
    }

    for (entity, hierarchy, layer_info) in sprite_layers.iter() {
        // Get EntitySpriteInfo from the parent entity
        let Ok(sprite_info) = sprite_info_query.get(hierarchy.parent_entity) else {
            warn!(
                "🔍 populate_sprite_assets: Parent entity {:?} missing EntitySpriteInfo (layer: {:?})",
                hierarchy.parent_entity, entity
            );
            continue;
        };

        // Determine asset paths based on EntitySpriteData and layer type
        let (sprite_path, act_path) = match &sprite_info.sprite_data {
            super::super::components::EntitySpriteData::Character {
                job_class,
                gender,
                head,
            } => {
                // Character: Load different assets based on layer type
                match &layer_info.layer_type {
                    SpriteLayerType::Body => {
                        // Load body sprite
                        let job = JobClass::from(*job_class);
                        let job_name = job.to_sprite_name();
                        (
                            body_sprite_path(*gender, job_name),
                            body_action_path(*gender, job_name),
                        )
                    }
                    SpriteLayerType::Head => {
                        // Load head sprite
                        (
                            head_sprite_path(*gender, *head),
                            head_action_path(*gender, *head),
                        )
                    }
                    SpriteLayerType::Equipment(_slot) => {
                        // Equipment layers start empty, will be populated by EquipmentChangeEvent
                        // Insert empty controller to stop matching Without<RoAnimationController> query
                        let empty_controller = RoAnimationController {
                            action_index: 0,
                            animation_index: 0,
                            frame_index: 0,
                            timer: 0.0,
                            current_delay: 0.15,
                            sprite_handle: Handle::default(),
                            action_handle: Handle::default(),
                            palette_handle: None,
                            loop_animation: true,
                            paused: false,
                            previous_frame_index: None,
                        };
                        commands.entity(entity).insert(empty_controller);
                        continue; // Skip asset loading, but controller is inserted
                    }
                    _ => {
                        warn!(
                            "populate_sprite_assets: Unsupported layer type {:?} for character",
                            layer_info.layer_type
                        );
                        continue;
                    }
                }
            }
            super::super::components::EntitySpriteData::Mob { sprite_name } => {
                use crate::domain::assets::patterns::{mob_action_path, mob_sprite_path};
                (mob_sprite_path(sprite_name), mob_action_path(sprite_name))
            }
            super::super::components::EntitySpriteData::Npc { sprite_name } => {
                use crate::domain::assets::patterns::{npc_action_path, npc_sprite_path};
                (npc_sprite_path(sprite_name), npc_action_path(sprite_name))
            }
        };

        debug!(
            "🔍 Loading assets for layer {:?} (parent {:?}): sprite={}, action={}",
            entity, hierarchy.parent_entity, sprite_path, act_path
        );

        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);

        // Determine initial action and pause state
        let initial_action = 0; // Idle action
        let is_paused = false; // Start playing immediately

        let controller = RoAnimationController::new(sprite_handle, act_handle)
            .with_action(initial_action)
            .looping(true)
            .paused(is_paused);

        debug!(
            "✅ Loaded assets for layer {:?} - RoAnimationController created",
            entity
        );
        commands.entity(entity).insert(controller);
    }
}

/// System to cleanup orphaned sprite objects
pub fn cleanup_orphaned_sprites(
    mut commands: Commands,
    sprite_roots: Query<(Entity, &SpriteHierarchy), Without<RoAnimationController>>,
    entities: Query<Entity>,
) {
    for (root_entity, hierarchy) in sprite_roots.iter() {
        if entities.get(hierarchy.parent_entity).is_err() {
            commands.entity(root_entity).despawn();
        }
    }
}
