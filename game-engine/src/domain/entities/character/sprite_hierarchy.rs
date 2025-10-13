use super::kinds::{CharacterRoot, SpriteLayer};
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::{
    equipment::EquipmentSlot,
    visual::{EffectType, RoSpriteLayer, SpriteLayerType},
    CharacterAppearance,
};
use crate::domain::entities::components::RoAnimationController;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::{
    RoActAsset, RoGroundAsset, RoPaletteAsset, RoSpriteAsset,
};
use crate::utils::constants::SPRITE_WORLD_SCALE;
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use moonshine_object::prelude::*;
use std::collections::HashMap;

// Additional component marker for effect layers
#[derive(Component, Debug)]
pub struct EffectLayer;

// Character sprite hierarchy - stores only root entity, uses moonshine-object for queries
#[derive(Component, Debug)]
pub struct CharacterObjectTree {
    pub root: Entity,
}

// Events for sprite hierarchy management
#[derive(Event)]
pub struct SpawnCharacterSpriteEvent {
    pub character_entity: Entity,
    pub spawn_position: Vec3,
}

#[derive(Event)]
pub struct EquipmentChangeEvent {
    pub character: Entity,
    pub slot: EquipmentSlot,
    pub new_item_id: Option<u32>,
}

#[derive(Event)]
pub struct StatusEffectVisualEvent {
    pub character: Entity,
    pub effect_type: EffectType,
    pub add: bool, // true to add, false to remove
}

#[derive(Event)]
pub struct SpriteAnimationChangeEvent {
    pub character_entity: Entity,
    pub action_type: crate::domain::entities::character::components::visual::ActionType,
}

// Component to mark entities as part of character sprite hierarchy
#[derive(Component)]
pub struct CharacterSpriteHierarchy {
    pub character_entity: Entity,
    pub layer_type: SpriteLayerType,
}

// Resource for sprite hierarchy configuration
#[derive(Resource)]
pub struct SpriteHierarchyConfig {
    pub default_z_spacing: f32,
    pub effect_z_offset: f32,
    pub shadow_z_offset: f32,
}

impl Default for SpriteHierarchyConfig {
    fn default() -> Self {
        Self {
            default_z_spacing: 0.01,
            effect_z_offset: 0.1,
            shadow_z_offset: -0.05,
        }
    }
}

// System to spawn character sprite hierarchies using moonshine-object patterns
// Now creates 3D billboard sprites with proper world positioning
pub fn spawn_character_sprite_hierarchy(
    mut commands: Commands,
    mut spawn_events: EventReader<SpawnCharacterSpriteEvent>,
    config: Res<SpriteHierarchyConfig>,
    character_query: Query<Entity>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
    map_loader_query: Query<&MapLoader>,
    ground_assets: Res<Assets<RoGroundAsset>>,
) {
    for event in spawn_events.read() {
        // Check if the character entity still exists before spawning sprites
        if character_query.get(event.character_entity).is_err() {
            warn!(
                "Character entity {:?} no longer exists, skipping sprite spawn",
                event.character_entity
            );
            continue;
        }

        // Calculate world position with terrain height (if available)
        let mut world_position = event.spawn_position;

        // Try to get terrain height at spawn position
        if let Ok(map_loader) = map_loader_query.single() {
            if let Some(ground_asset) = ground_assets.get(&map_loader.ground) {
                if let Some(terrain_height) = ground_asset
                    .ground
                    .get_terrain_height_at_position(world_position)
                {
                    world_position.y = terrain_height;
                }
            }
        }

        // Create root material - will be updated by animation system
        let root_material = materials.add(StandardMaterial {
            base_color_texture: None,
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        // Create root character object with 3D billboard components
        // CRITICAL: Only the root gets Billboard component!
        let root_entity = commands
            .spawn((
                CharacterRoot,
                Name::new("CharacterRoot"),
                Mesh3d(shared_quad.mesh.clone()),
                MeshMaterial3d(root_material),
                Transform::from_translation(world_position),
                GlobalTransform::default(),
                Billboard, // ← ONLY on root!
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                CharacterSpriteHierarchy {
                    character_entity: event.character_entity,
                    layer_type: SpriteLayerType::Body,
                },
            ))
            .id();

        // Create named sprite layers using moonshine naming convention
        let layer_names = [
            "Body",
            "Equipment/HeadBottom",
            "Equipment/HeadMid",
            "Equipment/HeadTop",
        ];

        for (i, layer_name) in layer_names.iter().enumerate() {
            let z_offset = i as f32 * config.default_z_spacing;

            // Create material for this layer
            let layer_material = materials.add(StandardMaterial {
                base_color_texture: None,
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });

            let _layer_entity = commands
                .spawn((
                    SpriteLayer,
                    Name::new(layer_name.to_string()),
                    Mesh3d(shared_quad.mesh.clone()),
                    MeshMaterial3d(layer_material),
                    Transform::from_xyz(0.0, 0.0, z_offset),
                    GlobalTransform::default(),
                    // NO Billboard component on children!
                    Visibility::default(),
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                    RoSpriteLayer {
                        layer_type: SpriteLayerType::from_name(layer_name),
                        z_offset,
                        ..default()
                    },
                    CharacterSpriteHierarchy {
                        character_entity: event.character_entity,
                        layer_type: SpriteLayerType::from_name(layer_name),
                    },
                ))
                .insert(ChildOf(root_entity))
                .id();
        }

        // Create Effects container
        let _effects_container = commands
            .spawn((
                Name::new("Effects"),
                Transform::from_xyz(0.0, 0.0, config.effect_z_offset),
                GlobalTransform::default(),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .insert(ChildOf(root_entity))
            .id();

        // Create the character object tree with just the root entity
        let object_tree = CharacterObjectTree { root: root_entity };

        // Add the object tree to the character entity
        // Use a command closure to check entity existence at command-application time
        // This prevents panic if the entity was despawned between the check and the insert
        let character_entity = event.character_entity;
        commands.queue(move |world: &mut World| {
            if let Ok(mut entity_mut) = world.get_entity_mut(character_entity) {
                entity_mut.insert(object_tree);
            } else {
                warn!(
                    "Character entity {:?} no longer exists when inserting CharacterObjectTree, cleaning up root {:?}",
                    character_entity, root_entity
                );
                // Clean up the orphaned root entity we just created
                if let Ok(root) = world.get_entity_mut(root_entity) {
                    root.despawn();
                }
            }
        });
    }
}

// System to handle equipment changes using moonshine-object
pub fn handle_equipment_changes(
    mut commands: Commands,
    characters: Query<&CharacterObjectTree>,
    character_objects: Objects<CharacterRoot>,
    mut equipment_events: EventReader<EquipmentChangeEvent>,
    config: Res<SpriteHierarchyConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
) {
    for event in equipment_events.read() {
        if let Ok(object_tree) = characters.get(event.character) {
            // Get the character object using moonshine-object
            if let Ok(character_obj) = character_objects.get(object_tree.root) {
                let equipment_path = format!("Equipment/{:?}", event.slot);

                // Remove old equipment layer if it exists using path traversal
                if let Some(old_equipment) = character_obj.find_by_path(&equipment_path) {
                    commands.entity(old_equipment.entity()).despawn();
                }

                // Add new equipment layer if item is equipped
                if let Some(_item_id) = event.new_item_id {
                    let z_offset = event.slot.z_order() * config.default_z_spacing;

                    // Create material for this equipment layer
                    let layer_material = materials.add(StandardMaterial {
                        base_color_texture: None,
                        base_color: Color::WHITE,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        cull_mode: None,
                        ..default()
                    });

                    // Spawn the equipment sprite entity with proper naming for moonshine-object
                    commands
                        .spawn((
                            SpriteLayer,
                            Name::new(format!("Equipment/{:?}", event.slot)),
                            Mesh3d(shared_quad.mesh.clone()),
                            MeshMaterial3d(layer_material),
                            Transform::from_xyz(0.0, 0.0, z_offset),
                            GlobalTransform::default(),
                            // NO Billboard on children!
                            Visibility::default(),
                            InheritedVisibility::default(),
                            ViewVisibility::default(),
                            RoSpriteLayer {
                                layer_type: SpriteLayerType::Equipment(event.slot),
                                z_offset,
                                ..default()
                            },
                            CharacterSpriteHierarchy {
                                character_entity: event.character,
                                layer_type: SpriteLayerType::Equipment(event.slot),
                            },
                        ))
                        .insert(ChildOf(object_tree.get_root_entity()));
                }
            }
        }
    }
}

// System to handle status effect visuals using moonshine-object
pub fn handle_status_effect_visuals(
    mut commands: Commands,
    characters: Query<&CharacterObjectTree>,
    character_objects: Objects<CharacterRoot>,
    mut effect_events: EventReader<StatusEffectVisualEvent>,
    config: Res<SpriteHierarchyConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
) {
    for event in effect_events.read() {
        if let Ok(object_tree) = characters.get(event.character) {
            // Get the character object using moonshine-object
            if let Ok(character_obj) = character_objects.get(object_tree.root) {
                let effect_path = format!("Effects/{:?}", event.effect_type);

                if event.add {
                    // Check if effect already exists using path traversal
                    if character_obj.find_by_path(&effect_path).is_some() {
                        continue; // Effect already exists
                    }

                    // Find Effects container
                    if let Some(effects_container) = character_obj.find_by_path("Effects") {
                        // Count existing effects for z-offset
                        let effect_count = effects_container.children().count();
                        let z_offset = effect_count as f32 * config.default_z_spacing;

                        // Create material for effect layer
                        let effect_material = materials.add(StandardMaterial {
                            base_color_texture: None,
                            base_color: Color::WHITE,
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            cull_mode: None,
                            ..default()
                        });

                        commands
                            .spawn((
                                EffectLayer,
                                Name::new(format!("{:?}", event.effect_type)),
                                Mesh3d(shared_quad.mesh.clone()),
                                MeshMaterial3d(effect_material),
                                Transform::from_xyz(0.0, 0.0, z_offset),
                                GlobalTransform::default(),
                                // NO Billboard on children!
                                Visibility::default(),
                                InheritedVisibility::default(),
                                ViewVisibility::default(),
                                CharacterSpriteHierarchy {
                                    character_entity: event.character,
                                    layer_type: SpriteLayerType::Effect(event.effect_type),
                                },
                            ))
                            .insert(ChildOf(effects_container.entity()));
                    }
                } else {
                    // Remove status effect visual using path traversal
                    if let Some(effect_entity) = character_obj.find_by_path(&effect_path) {
                        commands.entity(effect_entity.entity()).despawn();
                    }
                }
            }
        }
    }
}

// System to update sprite layer positions and textures based on animation
// Now updates 3D materials instead of 2D Sprite components
pub fn update_sprite_layer_transforms(
    mut commands: Commands,
    mut sprite_layers: Query<
        (
            Entity,
            &mut Transform,
            &CharacterSpriteHierarchy,
            &RoSpriteLayer,
            &RoAnimationController,
        ),
        With<RoAnimationController>,
    >,
    characters: Query<&crate::domain::entities::character::components::visual::CharacterSprite>,
    spr_assets: Res<Assets<crate::infrastructure::assets::loaders::RoSpriteAsset>>,
    act_assets: Res<Assets<crate::infrastructure::assets::loaders::RoActAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut transform, hierarchy, _ro_sprite_layer, controller) in sprite_layers.iter_mut()
    {
        if let Ok(character_sprite) = characters.get(hierarchy.character_entity) {
            // Get the sprite and action assets from RoAnimationController (not RoSpriteLayer)
            if let (Some(_spr_asset), Some(act_asset)) = (
                spr_assets.get(&controller.sprite_handle),
                act_assets.get(&controller.action_handle),
            ) {
                let current_action = character_sprite.current_action as usize;

                // Ensure action index is valid
                if current_action >= act_asset.action.actions.len() {
                    warn!(
                        "Action index out of bounds for {:?}: action {} >= total_actions {}",
                        hierarchy.layer_type,
                        current_action,
                        act_asset.action.actions.len()
                    );
                    continue;
                }
            } else {
                continue;
            }

            // Re-get assets after check (needed for borrow checker)
            if let (Some(spr_asset), Some(act_asset)) = (
                spr_assets.get(&controller.sprite_handle),
                act_assets.get(&controller.action_handle),
            ) {
                let current_action = character_sprite.current_action as usize;

                let action_sequence = &act_asset.action.actions[current_action];

                // Map frame index for head layers at idle to fix doridori animation
                // Head ACT files have 3x frames (8 directions × 3 headDir variants for doridori)
                // We only want headDir 0 (looking forward) to match body animation
                let current_frame = if matches!(
                    hierarchy.layer_type,
                    SpriteLayerType::Equipment(
                        EquipmentSlot::HeadBottom | EquipmentSlot::HeadMid | EquipmentSlot::HeadTop
                    )
                ) && current_action == 0
                // Idle action
                {
                    // Divide by 3 to get frames per doridori variant
                    let head_frames_per_variant = action_sequence.animations.len() / 3;
                    // Clamp to first variant (headDir 0)
                    if head_frames_per_variant > 0 {
                        (character_sprite.current_frame as usize) % head_frames_per_variant
                    } else {
                        character_sprite.current_frame as usize
                    }
                } else {
                    character_sprite.current_frame as usize
                };

                // Ensure frame index is valid
                if current_frame >= action_sequence.animations.len() {
                    warn!(
                        "Frame index out of bounds for {:?}: frame {} >= animation_count {}",
                        hierarchy.layer_type,
                        current_frame,
                        action_sequence.animations.len()
                    );
                    continue;
                }

                let animation = &action_sequence.animations[current_frame];

                // Process the first layer (main sprite layer)
                if let Some(layer) = animation.layers.first() {
                    // Handle negative sprite indices (use index 0 as fallback)
                    // RO uses -1 to indicate "no sprite" or invisible layers
                    let sprite_index = if layer.sprite_index < 0 {
                        0
                    } else {
                        layer.sprite_index as usize
                    };

                    // Ensure sprite index is valid
                    if sprite_index < spr_asset.sprite.frames.len() {
                        let sprite_frame = &spr_asset.sprite.frames[sprite_index];

                        // Convert SPR frame to Bevy Image
                        let bevy_image =
                            convert_sprite_frame_to_image(sprite_frame, &spr_asset.sprite.palette);

                        // Create new image handle
                        let image_handle = images.add(bevy_image);

                        // Apply ACT positioning offset from layer data
                        // ACT offsets are in pixel coordinates, scale to world units
                        let offset_x = layer.pos[0] as f32 * SPRITE_WORLD_SCALE;
                        let offset_y = -layer.pos[1] as f32 * SPRITE_WORLD_SCALE; // Flip Y for Bevy coordinate system
                        transform.translation.x = offset_x;
                        transform.translation.y = offset_y;
                        // Keep z-offset from RoSpriteLayer
                        // transform.translation.z is already set by spawn

                        // Apply ACT scale
                        // The quad mesh is -0.5 to 0.5 (1 unit total)
                        // Scale by pixel dimensions using SPRITE_WORLD_SCALE for 3D world
                        transform.scale = Vec3::new(
                            layer.scale[0] * sprite_frame.width as f32 * SPRITE_WORLD_SCALE,
                            layer.scale[1] * sprite_frame.height as f32 * SPRITE_WORLD_SCALE,
                            1.0,
                        );

                        // Apply ACT rotation
                        if layer.angle != 0 {
                            transform.rotation = Quat::from_rotation_z(
                                layer.angle as f32 * std::f32::consts::PI / 180.0,
                            );
                        }

                        // Create new material with the updated texture
                        // This replaces the entire material to force Bevy's render system to update GPU bindings
                        let new_material = materials.add(StandardMaterial {
                            base_color_texture: Some(image_handle),
                            base_color: Color::srgba(
                                layer.color[0],
                                layer.color[1],
                                layer.color[2],
                                layer.color[3],
                            ),
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            cull_mode: None,
                            ..default()
                        });

                        // Replace the material component to trigger Bevy's change detection
                        commands.entity(entity).insert(MeshMaterial3d(new_material));
                    } else {
                        warn!(
                            "Invalid sprite_index for {:?}: index={} (must be < {})",
                            hierarchy.layer_type,
                            sprite_index,
                            spr_asset.sprite.frames.len()
                        );
                    }
                } else {
                    warn!("Animation has no layers for {:?}!", hierarchy.layer_type);
                }
            }
        }
    }
}

// Helper function to convert SPR frame data to Bevy Image
fn convert_sprite_frame_to_image(
    frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
    palette: &Option<crate::infrastructure::ro_formats::sprite::Palette>,
) -> Image {
    let mut rgba_data = Vec::with_capacity((frame.width as usize) * (frame.height as usize) * 4);

    if frame.is_rgba {
        // RGBA frame - data is already in RGBA format
        rgba_data.extend_from_slice(&frame.data);
    } else {
        // Indexed frame - convert using palette
        if let Some(palette) = palette {
            for &index in &frame.data {
                if (index as usize) < palette.colors.len() {
                    let color = palette.colors[index as usize];
                    rgba_data.extend_from_slice(&color);
                } else {
                    // Transparent pixel for invalid palette index
                    rgba_data.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        } else {
            // No palette available - treat as grayscale with alpha
            for &index in &frame.data {
                if index == 0 {
                    // Index 0 is typically transparent
                    rgba_data.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    // Convert index to grayscale
                    rgba_data.extend_from_slice(&[index, index, index, 255]);
                }
            }
        }
    }

    Image::new(
        Extent3d {
            width: frame.width as u32,
            height: frame.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

// System to advance character animation frames based on timing
pub fn advance_character_animations(
    time: Res<Time>,
    mut character_sprites: Query<
        &mut crate::domain::entities::character::components::visual::CharacterSprite,
    >,
    act_assets: Res<Assets<crate::infrastructure::assets::loaders::RoActAsset>>,
    sprite_layers: Query<&RoAnimationController, With<CharacterSpriteHierarchy>>,
) {
    for mut character_sprite in character_sprites.iter_mut() {
        // Tick the animation timer
        character_sprite.animation_timer.tick(time.delta());

        if character_sprite.animation_timer.finished() {
            // Get ACT asset to determine timing and frame count
            let mut act_asset_handle = None;

            // Find an ACT asset handle from the RoAnimationController components
            for controller in sprite_layers.iter() {
                if !controller.action_handle.is_weak() {
                    act_asset_handle = Some(&controller.action_handle);
                    break;
                }
            }

            if let Some(act_handle) = act_asset_handle {
                if let Some(act_asset) = act_assets.get(act_handle) {
                    let current_action = character_sprite.current_action as usize;

                    if current_action < act_asset.action.actions.len() {
                        let action_sequence = &act_asset.action.actions[current_action];
                        let frame_count = action_sequence.animations.len();

                        if frame_count > 0 {
                            // Advance to next frame
                            character_sprite.current_frame =
                                (character_sprite.current_frame + 1) % (frame_count as u8);

                            // Set timer duration from ACT delay (convert from milliseconds to seconds)
                            let delay_seconds = action_sequence.delay / 1000.0;
                            character_sprite.animation_timer.set_duration(
                                std::time::Duration::from_secs_f32(delay_seconds.max(0.1)), // Minimum 0.1s delay
                            );
                            character_sprite.animation_timer.reset();
                        }
                    }
                }
            }
        }
    }
}

// System to handle sprite animation changes from state machine
pub fn handle_sprite_animation_changes(
    mut animation_events: EventReader<SpriteAnimationChangeEvent>,
    mut character_sprites: Query<
        &mut crate::domain::entities::character::components::visual::CharacterSprite,
    >,
) {
    for event in animation_events.read() {
        if let Ok(mut sprite) = character_sprites.get_mut(event.character_entity) {
            sprite.play_action(event.action_type);
        }
    }
}

// System to cleanup orphaned sprite objects
pub fn cleanup_orphaned_sprites(
    mut commands: Commands,
    sprite_roots: Query<(Entity, &CharacterSpriteHierarchy), With<CharacterRoot>>,
    characters: Query<&CharacterObjectTree>,
) {
    for (root_entity, hierarchy) in sprite_roots.iter() {
        // Check if the parent character still exists
        if characters.get(hierarchy.character_entity).is_err() {
            // Despawn root - children are automatically despawned
            commands.entity(root_entity).despawn();
        }
    }
}

// Helper function to spawn a complete character sprite hierarchy
// Note: This function signature needs to be used with EventWriter parameters
pub fn spawn_complete_character_sprite(
    spawn_events: &mut EventWriter<SpawnCharacterSpriteEvent>,
    equipment_events: &mut EventWriter<EquipmentChangeEvent>,
    character_entity: Entity,
    position: Vec3,
    equipment_slots: &HashMap<EquipmentSlot, u32>,
) {
    // Send spawn event
    spawn_events.write(SpawnCharacterSpriteEvent {
        character_entity,
        spawn_position: position,
    });

    // Send equipment events for each equipped item
    for (&slot, &item_id) in equipment_slots {
        equipment_events.write(EquipmentChangeEvent {
            character: character_entity,
            slot,
            new_item_id: Some(item_id),
        });
    }
}

/// System to populate sprite layers with asset handles and RoAnimationController
/// This bridges the gap between the entity hierarchy and the rendering system
/// Now adds 3D mesh/material components instead of 2D Sprite
pub fn populate_sprite_layers_with_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_layers: Query<
        (Entity, &CharacterSpriteHierarchy, &RoSpriteLayer),
        (Without<RoAnimationController>, With<SpriteLayer>),
    >,
    characters: Query<(
        &super::components::core::CharacterData,
        &CharacterAppearance,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
    map_spawn_context: Option<Res<crate::domain::world::spawn_context::MapSpawnContext>>,
) {
    for (entity, hierarchy, layer_info) in sprite_layers.iter() {
        // Get character data and appearance to determine which assets to load
        let Ok((char_data, appearance)) = characters.get(hierarchy.character_entity) else {
            warn!(
                "Failed to get character data for entity {:?}",
                hierarchy.character_entity
            );
            continue;
        };

        // Generate asset paths based on layer type
        let (sprite_path, act_path, palette_path) = match &layer_info.layer_type {
            SpriteLayerType::Body => {
                let job_class = crate::domain::character::JobClass::from(char_data.job_id);
                let job_name = job_class.to_sprite_name();
                let sex_suffix = match appearance.gender {
                    crate::domain::character::Gender::Male => "남",
                    crate::domain::character::Gender::Female => "여",
                };

                let sprite = format!(
                    "ro://data/sprite/인간족/몸통/{}/{}_{}.spr",
                    sex_suffix, job_name, sex_suffix
                );
                let act = format!(
                    "ro://data/sprite/인간족/몸통/{}/{}_{}.act",
                    sex_suffix, job_name, sex_suffix
                );
                (sprite, act, None)
            }
            SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
            | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
            | SpriteLayerType::Equipment(EquipmentSlot::HeadTop) => {
                // For now, use head sprites for all head layers
                let sex_suffix = match appearance.gender {
                    crate::domain::character::Gender::Male => "남",
                    crate::domain::character::Gender::Female => "여",
                };

                let sprite = format!(
                    "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
                    sex_suffix, appearance.hair_style, sex_suffix
                );
                let act = format!(
                    "ro://data/sprite/인간족/머리통/{}/{}_{}.act",
                    sex_suffix, appearance.hair_style, sex_suffix
                );

                let palette = if appearance.hair_color > 0 {
                    Some(format!(
                        "ro://data/palette/머리/{}_{}_{}.pal",
                        appearance.hair_style, sex_suffix, appearance.hair_color
                    ))
                } else {
                    None
                };

                (sprite, act, palette)
            }
            _ => {
                continue; // Skip other layer types for now
            }
        };

        // Load assets via AssetServer
        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
        let palette_handle = palette_path
            .as_ref()
            .map(|path| asset_server.load::<RoPaletteAsset>(path));

        // Determine if we should pause based on context
        // We're in InGame when MapSpawnContext exists and character is being spawned for gameplay
        // Default to NOT paused for in-game rendering
        let should_pause = map_spawn_context.is_none(); // Paused only for character selection (no map context)

        // Create RoAnimationController
        let mut controller = RoAnimationController::new(sprite_handle.clone(), act_handle.clone())
            .with_action(0) // Idle action
            .looping(true)
            .paused(should_pause); // Not paused for in-game rendering

        if let Some(palette) = palette_handle {
            controller = controller.with_palette(palette);
        }

        // Create material for 3D rendering
        let material = materials.add(StandardMaterial {
            base_color_texture: None, // Will be set by animation system
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        // Add controller, mesh, and material components to the entity
        commands.entity(entity).insert((
            controller,
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(material),
        ));
    }
}

/// System to update sprite layer assets when character appearance changes
/// This allows hair style/color and gender changes to be reflected in the preview
pub fn update_sprite_layers_on_appearance_change(
    asset_server: Res<AssetServer>,
    mut sprite_layers: Query<(
        &CharacterSpriteHierarchy,
        &RoSpriteLayer,
        &mut RoAnimationController,
    )>,
    changed_characters: Query<
        (
            Entity,
            &super::components::core::CharacterData,
            &CharacterAppearance,
        ),
        Changed<CharacterAppearance>,
    >,
) {
    for (character_entity, char_data, appearance) in changed_characters.iter() {
        // Find all sprite layers for this character
        for (hierarchy, layer_info, mut controller) in sprite_layers.iter_mut() {
            // Only process layers that belong to this character
            if hierarchy.character_entity != character_entity {
                continue;
            }

            let sex_suffix = match appearance.gender {
                crate::domain::character::Gender::Male => "남",
                crate::domain::character::Gender::Female => "여",
            };

            // Generate new asset paths based on layer type
            let (sprite_path, act_path, palette_path) = match &layer_info.layer_type {
                SpriteLayerType::Body => {
                    // Update body sprite when gender changes
                    let job_class = crate::domain::character::JobClass::from(char_data.job_id);
                    let job_name = job_class.to_sprite_name();

                    let sprite = format!(
                        "ro://data/sprite/인간족/몸통/{}/{}_{}.spr",
                        sex_suffix, job_name, sex_suffix
                    );
                    let act = format!(
                        "ro://data/sprite/인간족/몸통/{}/{}_{}.act",
                        sex_suffix, job_name, sex_suffix
                    );
                    (sprite, act, None)
                }
                SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadTop) => {
                    // Update head sprites when hair style/color or gender changes
                    let sprite = format!(
                        "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
                        sex_suffix, appearance.hair_style, sex_suffix
                    );
                    let act = format!(
                        "ro://data/sprite/인간족/머리통/{}/{}_{}.act",
                        sex_suffix, appearance.hair_style, sex_suffix
                    );

                    let palette = if appearance.hair_color > 0 {
                        Some(format!(
                            "ro://data/palette/머리/{}_{}_{}.pal",
                            appearance.hair_style, sex_suffix, appearance.hair_color
                        ))
                    } else {
                        None
                    };

                    (sprite, act, palette)
                }
                _ => continue, // Skip other layer types
            };

            // Load new assets
            let new_sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
            let new_act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
            let palette_handle = palette_path
                .as_ref()
                .map(|path| asset_server.load::<RoPaletteAsset>(path));

            // Update the animation controller with new assets
            controller.sprite_handle = new_sprite_handle;
            controller.action_handle = new_act_handle;
            controller.palette_handle = palette_handle;
            controller.reset(); // Reset animation to frame 0
        }
    }
}

// Plugin to set up sprite hierarchy systems
pub struct CharacterSpriteHierarchyPlugin;

impl Plugin for CharacterSpriteHierarchyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpriteHierarchyConfig>()
            .add_event::<SpawnCharacterSpriteEvent>()
            .add_event::<EquipmentChangeEvent>()
            .add_event::<StatusEffectVisualEvent>()
            .add_event::<SpriteAnimationChangeEvent>()
            .add_systems(
                Update,
                (
                    spawn_character_sprite_hierarchy,
                    populate_sprite_layers_with_assets, // Load assets and add RoAnimationController
                    update_sprite_layers_on_appearance_change, // Update assets when appearance changes
                    handle_equipment_changes,
                    handle_status_effect_visuals,
                    handle_sprite_animation_changes,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    advance_character_animations,
                    update_sprite_layer_transforms,
                    cleanup_orphaned_sprites,
                )
                    .chain()
                    .after(handle_sprite_animation_changes),
            );
    }
}

impl CharacterObjectTree {
    /// Get the root entity of the character sprite hierarchy
    pub fn get_root_entity(&self) -> Entity {
        self.root
    }
}
