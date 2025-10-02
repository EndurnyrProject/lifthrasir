use super::kinds::{CharacterRoot, SpriteLayer};
use crate::domain::entities::character::components::{
    equipment::EquipmentSlot,
    visual::{EffectType, RoSpriteLayer, SpriteLayerType},
    CharacterAppearance,
};
use crate::domain::entities::components::RoAnimationController;
use crate::infrastructure::assets::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
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
#[derive(Resource, Default)]
pub struct SpriteHierarchyConfig {
    pub default_z_spacing: f32,
    pub effect_z_offset: f32,
    pub shadow_z_offset: f32,
}

// System to spawn character sprite hierarchies using moonshine-object patterns
pub fn spawn_character_sprite_hierarchy(
    mut commands: Commands,
    mut spawn_events: EventReader<SpawnCharacterSpriteEvent>,
    config: Res<SpriteHierarchyConfig>,
    character_query: Query<Entity>,
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

        info!(
            "Spawning sprite hierarchy for character entity: {:?}",
            event.character_entity
        );

        // Create root character object
        let root_entity = commands
            .spawn((
                CharacterRoot,
                Name::new("CharacterRoot"),
                Transform::from_translation(event.spawn_position),
                GlobalTransform::default(),
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

            let layer_entity = commands
                .spawn((
                    SpriteLayer,
                    Name::new(layer_name.to_string()),
                    Transform::from_xyz(0.0, 0.0, z_offset),
                    GlobalTransform::default(),
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
        let effects_container = commands
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
                info!(
                    "Created sprite hierarchy for character: root={:?}",
                    root_entity
                );
            } else {
                warn!(
                    "Character entity {:?} no longer exists when inserting CharacterObjectTree, cleaning up root {:?}",
                    character_entity, root_entity
                );
                // Clean up the orphaned root entity we just created
                if let Ok(mut root) = world.get_entity_mut(root_entity) {
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
) {
    for event in equipment_events.read() {
        if let Ok(object_tree) = characters.get(event.character) {
            info!("Handling equipment change for slot: {:?}", event.slot);

            // Get the character object using moonshine-object
            if let Ok(character_obj) = character_objects.get(object_tree.root) {
                let equipment_path = format!("Equipment/{:?}", event.slot);

                // Remove old equipment layer if it exists using path traversal
                if let Some(old_equipment) = character_obj.find_by_path(&equipment_path) {
                    commands.entity(old_equipment.entity()).despawn();
                }

                // Add new equipment layer if item is equipped
                if let Some(item_id) = event.new_item_id {
                    let z_offset = event.slot.z_order() * config.default_z_spacing;

                    // Spawn the equipment sprite entity with proper naming for moonshine-object
                    let equipment_entity = commands
                        .spawn((
                            SpriteLayer,
                            Name::new(format!("Equipment/{:?}", event.slot)),
                            Transform::from_xyz(0.0, 0.0, z_offset),
                            GlobalTransform::default(),
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
                        .insert(ChildOf(object_tree.get_root_entity()))
                        .id();

                    info!(
                        "Added equipment layer for slot {:?}: {:?}",
                        event.slot, equipment_entity
                    );
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

                        let effect_entity = commands
                            .spawn((
                                EffectLayer,
                                Name::new(format!("{:?}", event.effect_type)),
                                Transform::from_xyz(0.0, 0.0, z_offset),
                                GlobalTransform::default(),
                                Visibility::default(),
                                InheritedVisibility::default(),
                                ViewVisibility::default(),
                                CharacterSpriteHierarchy {
                                    character_entity: event.character,
                                    layer_type: SpriteLayerType::Effect(event.effect_type),
                                },
                            ))
                            .insert(ChildOf(effects_container.entity()))
                            .id();

                        info!(
                            "Added status effect visual {:?}: {:?}",
                            event.effect_type, effect_entity
                        );
                    }
                } else {
                    // Remove status effect visual using path traversal
                    if let Some(effect_entity) = character_obj.find_by_path(&effect_path) {
                        commands.entity(effect_entity.entity()).despawn();
                        info!("Removed status effect visual {:?}", event.effect_type);
                    }
                }
            }
        }
    }
}

// System to update sprite layer positions and textures based on animation
pub fn update_sprite_layer_transforms(
    mut commands: Commands,
    mut sprite_layers: Query<(
        Entity,
        &mut Transform,
        &CharacterSpriteHierarchy,
        &RoSpriteLayer,
        Option<&mut Sprite>,
    )>,
    characters: Query<&crate::domain::entities::character::components::visual::CharacterSprite>,
    spr_assets: Res<Assets<crate::infrastructure::assets::loaders::RoSpriteAsset>>,
    act_assets: Res<Assets<crate::infrastructure::assets::loaders::RoActAsset>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, mut transform, hierarchy, ro_sprite_layer, sprite_component) in
        sprite_layers.iter_mut()
    {
        if let Ok(character_sprite) = characters.get(hierarchy.character_entity) {
            // Get the sprite and action assets
            if let (Some(spr_asset), Some(act_asset)) = (
                spr_assets.get(&ro_sprite_layer.sprite_handle),
                act_assets.get(&ro_sprite_layer.action_handle),
            ) {
                let current_action = character_sprite.current_action as usize;
                let current_frame = character_sprite.current_frame as usize;

                // Ensure action index is valid
                if current_action >= act_asset.action.actions.len() {
                    continue;
                }

                let action_sequence = &act_asset.action.actions[current_action];

                // Ensure frame index is valid
                if current_frame >= action_sequence.animations.len() {
                    continue;
                }

                let animation = &action_sequence.animations[current_frame];

                // Process the first layer (main sprite layer)
                if let Some(layer) = animation.layers.first() {
                    let sprite_index = layer.sprite_index;

                    // Ensure sprite index is valid
                    if sprite_index >= 0 && (sprite_index as usize) < spr_asset.sprite.frames.len()
                    {
                        let sprite_frame = &spr_asset.sprite.frames[sprite_index as usize];

                        // Convert SPR frame to Bevy Image
                        let bevy_image =
                            convert_sprite_frame_to_image(sprite_frame, &spr_asset.sprite.palette);

                        // Create new image handle
                        let image_handle = images.add(bevy_image);

                        // Apply ACT positioning offset from layer data
                        let offset_x = layer.pos[0] as f32;
                        let offset_y = -layer.pos[1] as f32; // Flip Y for Bevy coordinate system
                        transform.translation.x = offset_x;
                        transform.translation.y = offset_y;

                        // Apply ACT scale
                        transform.scale.x = layer.scale[0];
                        transform.scale.y = layer.scale[1];

                        // Apply ACT rotation
                        if layer.angle != 0 {
                            transform.rotation = Quat::from_rotation_z(
                                layer.angle as f32 * std::f32::consts::PI / 180.0,
                            );
                        }

                        // Create or update sprite component
                        if sprite_component.is_none() {
                            commands.entity(entity).insert((Sprite {
                                flip_x: layer.is_mirror,
                                color: Color::srgba(
                                    layer.color[0],
                                    layer.color[1],
                                    layer.color[2],
                                    layer.color[3],
                                ),
                                image: image_handle,
                                ..default()
                            },));
                        } else if let Some(mut sprite) = sprite_component {
                            // Update existing sprite
                            sprite.flip_x = layer.is_mirror;
                            sprite.color = Color::srgba(
                                layer.color[0],
                                layer.color[1],
                                layer.color[2],
                                layer.color[3],
                            );
                        }
                    }
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
    sprite_layers: Query<&RoSpriteLayer, With<CharacterSpriteHierarchy>>,
) {
    for mut character_sprite in character_sprites.iter_mut() {
        // Tick the animation timer
        character_sprite.animation_timer.tick(time.delta());

        if character_sprite.animation_timer.finished() {
            // Get ACT asset to determine timing and frame count
            let mut act_asset_handle = None;

            // Find an ACT asset handle from the sprite layers
            for sprite_layer in sprite_layers.iter() {
                if !sprite_layer.action_handle.is_weak() {
                    act_asset_handle = Some(&sprite_layer.action_handle);
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
            info!(
                "Updated animation for {:?}: {:?}",
                event.character_entity, event.action_type
            );
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
            info!("Cleaning up orphaned sprite hierarchy: {:?}", root_entity);
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
) {
    for (entity, hierarchy, layer_info) in sprite_layers.iter() {
        // Get character data and appearance to determine which assets to load
        let Ok((char_data, appearance)) = characters.get(hierarchy.character_entity) else {
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
            _ => continue, // Skip other layer types for now
        };

        // Load assets via AssetServer
        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
        let palette_handle = palette_path
            .as_ref()
            .map(|path| asset_server.load::<RoPaletteAsset>(path));

        // Create RoAnimationController
        let mut controller = RoAnimationController::new(sprite_handle.clone(), act_handle.clone())
            .with_action(0) // Idle action
            .looping(true)
            .paused(true); // Paused for static display in character selection

        if let Some(palette) = palette_handle {
            controller = controller.with_palette(palette);
        }

        // Add controller and Sprite component to the entity
        commands
            .entity(entity)
            .insert((controller, Sprite::default()));

        info!(
            "Loaded assets for sprite layer {:?}: {}",
            layer_info.layer_type, sprite_path
        );
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

            info!(
                "Updated sprite layer assets for {:?}: gender={:?}, hair_style={}, hair_color={}",
                layer_info.layer_type,
                appearance.gender,
                appearance.hair_style,
                appearance.hair_color
            );
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
