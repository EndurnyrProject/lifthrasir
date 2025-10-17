use super::kinds::{CharacterRoot, SpriteLayer};
use crate::domain::assets::patterns::{
    body_action_path, body_sprite_path, hair_palette_path, head_action_path, head_sprite_path,
};
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::{
    equipment::EquipmentSlot,
    visual::{EffectType, RoSpriteLayer, SpriteLayerType},
    CharacterAppearance,
};
use crate::domain::entities::components::RoAnimationController;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::{
    RoActAsset, RoAltitudeAsset, RoPaletteAsset, RoSpriteAsset,
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

/// Context struct holding validated animation data for sprite layer updates
/// Reduces repeated unwrapping and simplifies function signatures
struct AnimationContext<'a> {
    spr_asset: &'a RoSpriteAsset,
    sprite_frame: &'a crate::infrastructure::ro_formats::sprite::SpriteFrame,
    layer: &'a crate::infrastructure::ro_formats::act::Layer,
}

/// Query type for sprite layers that need asset population
/// Queries sprite layers that don't yet have animation controllers attached
type SpriteLayersNeedingAssets<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CharacterSpriteHierarchy,
        &'static RoSpriteLayer,
    ),
    (Without<RoAnimationController>, With<SpriteLayer>),
>;

/// Shared bundle components for sprite layers
#[derive(Bundle)]
struct SpriteLayerBaseBundle {
    name: Name,
    mesh: Mesh3d,
    material: MeshMaterial3d<StandardMaterial>,
    transform: Transform,
    global_transform: GlobalTransform,
    visibility: Visibility,
    inherited_visibility: InheritedVisibility,
    view_visibility: ViewVisibility,
    ro_sprite_layer: RoSpriteLayer,
    hierarchy: CharacterSpriteHierarchy,
}

impl SpriteLayerBaseBundle {
    fn new(
        name: String,
        material: Handle<StandardMaterial>,
        mesh: Handle<Mesh>,
        layer_type: SpriteLayerType,
        character_entity: Entity,
        z_offset: f32,
    ) -> Self {
        Self {
            name: Name::new(name),
            mesh: Mesh3d(mesh),
            material: MeshMaterial3d(material),
            transform: Transform::from_xyz(0.0, 0.0, z_offset),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            inherited_visibility: InheritedVisibility::default(),
            view_visibility: ViewVisibility::default(),
            ro_sprite_layer: RoSpriteLayer {
                layer_type: layer_type.clone(),
                z_offset,
                ..default()
            },
            hierarchy: CharacterSpriteHierarchy {
                character_entity,
                layer_type,
            },
        }
    }
}

/// Helper to spawn equipment layer
fn create_equipment_bundle(
    slot: EquipmentSlot,
    character_entity: Entity,
    material: Handle<StandardMaterial>,
    mesh: Handle<Mesh>,
    config: &SpriteHierarchyConfig,
) -> (SpriteLayer, SpriteLayerBaseBundle) {
    let z_offset = slot.z_order() * config.default_z_spacing;
    let layer_type = SpriteLayerType::Equipment(slot);
    (
        SpriteLayer,
        SpriteLayerBaseBundle::new(
            format!("Equipment/{:?}", slot),
            material,
            mesh,
            layer_type,
            character_entity,
            z_offset,
        ),
    )
}

/// Helper to spawn effect layer
fn create_effect_bundle(
    effect_type: EffectType,
    character_entity: Entity,
    material: Handle<StandardMaterial>,
    mesh: Handle<Mesh>,
    z_offset: f32,
) -> (EffectLayer, SpriteLayerBaseBundle) {
    let layer_type = SpriteLayerType::Effect(effect_type);
    (
        EffectLayer,
        SpriteLayerBaseBundle::new(
            format!("{:?}", effect_type),
            material,
            mesh,
            layer_type,
            character_entity,
            z_offset,
        ),
    )
}

/// SystemParam bundle for rendering-related resources
#[derive(bevy::ecs::system::SystemParam)]
pub struct RenderingResources<'w> {
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    pub shared_quad: Res<'w, SharedSpriteQuad>,
    pub config: Res<'w, SpriteHierarchyConfig>,
}

/// SystemParam bundle for terrain-related resources
#[derive(bevy::ecs::system::SystemParam)]
pub struct TerrainResources<'w, 's> {
    pub map_loader_query: Query<'w, 's, &'static MapLoader>,
    pub altitude_assets: Res<'w, Assets<RoAltitudeAsset>>,
}

/// Context struct for rendering resources passed to helper functions
/// Groups config, materials, and shared quad to reduce parameter count
struct RenderingContext<'a> {
    config: &'a SpriteHierarchyConfig,
    materials: &'a mut Assets<StandardMaterial>,
    shared_quad: &'a SharedSpriteQuad,
}

/// Helper function to calculate terrain height at a given position
/// Returns the adjusted position with terrain height applied (if available)
fn calculate_terrain_height(position: Vec3, terrain: &TerrainResources) -> Vec3 {
    let mut world_position = position;

    if let Some(terrain_height) = terrain
        .map_loader_query
        .single()
        .ok()
        .and_then(|loader| loader.altitude.as_ref())
        .and_then(|handle| terrain.altitude_assets.get(handle))
        .and_then(|asset| {
            asset
                .altitude
                .get_terrain_height_at_position(world_position)
        })
    {
        world_position.y = terrain_height;
    }

    world_position
}

// System to spawn character sprite hierarchies using moonshine-object patterns
pub fn spawn_character_sprite_hierarchy(
    mut commands: Commands,
    mut spawn_events: EventReader<SpawnCharacterSpriteEvent>,
    character_query: Query<Entity>,
    mut rendering: RenderingResources,
    terrain: TerrainResources,
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
        let world_position = calculate_terrain_height(event.spawn_position, &terrain);

        // Create root material - will be updated by animation system
        let root_material = create_sprite_material(&mut rendering.materials);

        // Create root character object with 3D billboard components
        // CRITICAL: Only the root gets Billboard component!
        let root_entity = commands
            .spawn((
                CharacterRoot,
                Name::new("CharacterRoot"),
                Mesh3d(rendering.shared_quad.mesh.clone()),
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
            let z_offset = i as f32 * rendering.config.default_z_spacing;

            // Create material for this layer
            let layer_material = create_sprite_material(&mut rendering.materials);

            let _layer_entity = commands
                .spawn((
                    SpriteLayer,
                    Name::new(layer_name.to_string()),
                    Mesh3d(rendering.shared_quad.mesh.clone()),
                    MeshMaterial3d(layer_material),
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
        let _effects_container = commands
            .spawn((
                Name::new("Effects"),
                Transform::from_xyz(0.0, 0.0, rendering.config.effect_z_offset),
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
        let Ok(object_tree) = characters.get(event.character) else {
            warn!(
                "Character entity {:?} not found for equipment change",
                event.character
            );
            continue;
        };

        let Ok(character_obj) = character_objects.get(object_tree.root) else {
            warn!("Character object root {:?} not found", object_tree.root);
            continue;
        };

        let equipment_path = equipment_layer_path(event.slot);

        remove_old_equipment(&mut commands, &character_obj, &equipment_path);

        if event.new_item_id.is_some() {
            let mut rendering = RenderingContext {
                config: &config,
                materials: &mut materials,
                shared_quad: &shared_quad,
            };
            spawn_equipment_layer(
                &mut commands,
                event.slot,
                event.character,
                object_tree.root,
                &mut rendering,
            );
        }
    }
}

/// Removes an existing equipment layer from the character
fn remove_old_equipment(
    commands: &mut Commands,
    character_obj: &Object<CharacterRoot>,
    equipment_path: &str,
) {
    if let Some(old_equipment) = character_obj.find_by_path(equipment_path) {
        commands.entity(old_equipment.entity()).despawn();
    }
}

/// Spawns a new equipment layer as a child of the character root
fn spawn_equipment_layer(
    commands: &mut Commands,
    slot: EquipmentSlot,
    character_entity: Entity,
    root_entity: Entity,
    rendering: &mut RenderingContext,
) {
    let material = create_sprite_material(rendering.materials);
    let (marker, bundle) = create_equipment_bundle(
        slot,
        character_entity,
        material,
        rendering.shared_quad.mesh.clone(),
        rendering.config,
    );

    commands
        .spawn((marker, bundle))
        .insert(ChildOf(root_entity));
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
        let Ok(object_tree) = characters.get(event.character) else {
            warn!(
                "Character entity {:?} not found for effect visual",
                event.character
            );
            continue;
        };

        let Ok(character_obj) = character_objects.get(object_tree.root) else {
            warn!("Character object root {:?} not found", object_tree.root);
            continue;
        };

        let effect_path = format!("Effects/{:?}", event.effect_type);

        if event.add {
            let mut rendering = RenderingContext {
                config: &config,
                materials: &mut materials,
                shared_quad: &shared_quad,
            };
            add_status_effect(
                &mut commands,
                &character_obj,
                &effect_path,
                event.effect_type,
                event.character,
                &mut rendering,
            );
        } else {
            remove_status_effect(&mut commands, &character_obj, &effect_path);
        }
    }
}

/// Adds a status effect visual to the character
fn add_status_effect(
    commands: &mut Commands,
    character_obj: &Object<CharacterRoot>,
    effect_path: &str,
    effect_type: EffectType,
    character_entity: Entity,
    rendering: &mut RenderingContext,
) {
    // Check if effect already exists
    if character_obj.find_by_path(effect_path).is_some() {
        return;
    }

    // Find Effects container
    let Some(effects_container) = character_obj.find_by_path("Effects") else {
        warn!("Effects container not found for character");
        return;
    };

    // Count existing effects for z-offset
    let effect_count = effects_container.children().count();
    let z_offset = effect_count as f32 * rendering.config.default_z_spacing;

    let material = create_sprite_material(rendering.materials);
    let (marker, bundle) = create_effect_bundle(
        effect_type,
        character_entity,
        material,
        rendering.shared_quad.mesh.clone(),
        z_offset,
    );

    commands
        .spawn((marker, bundle))
        .insert(ChildOf(effects_container.entity()));
}

/// Removes a status effect visual from the character
fn remove_status_effect(
    commands: &mut Commands,
    character_obj: &Object<CharacterRoot>,
    effect_path: &str,
) {
    if let Some(effect_entity) = character_obj.find_by_path(effect_path) {
        commands.entity(effect_entity.entity()).despawn();
    }
}

/// Helper function to calculate the correct frame index for a sprite layer
/// Handles special case for head equipment doridori animation
fn calculate_layer_frame_index(
    layer_type: &SpriteLayerType,
    current_action: usize,
    base_frame: usize,
    animation_count: usize,
) -> usize {
    // Special handling for head equipment doridori animation
    // Head ACT files have 3x frames (8 directions × 3 headDir variants for doridori)
    // We only want headDir 0 (looking forward) to match body animation
    if matches!(
        layer_type,
        SpriteLayerType::Equipment(
            EquipmentSlot::HeadBottom | EquipmentSlot::HeadMid | EquipmentSlot::HeadTop
        )
    ) && current_action == 0
    {
        // Divide by 3 to get frames per doridori variant
        let frames_per_variant = animation_count / 3;
        // Clamp to first variant (headDir 0)
        if frames_per_variant > 0 {
            return base_frame % frames_per_variant;
        }
    }
    base_frame
}

/// Helper function to get and validate animation context
/// Returns None if any validation fails, with appropriate warnings
fn get_animation_context<'a>(
    hierarchy: &CharacterSpriteHierarchy,
    controller: &RoAnimationController,
    characters: &Query<&crate::domain::entities::character::components::visual::CharacterSprite>,
    spr_assets: &'a Assets<RoSpriteAsset>,
    act_assets: &'a Assets<RoActAsset>,
) -> Option<AnimationContext<'a>> {
    // Get character sprite component
    let character_sprite = characters.get(hierarchy.character_entity).ok()?;

    // Get sprite and action assets
    let spr_asset = spr_assets.get(&controller.sprite_handle)?;
    let act_asset = act_assets.get(&controller.action_handle)?;

    let current_action = character_sprite.current_action as usize;

    // Validate action index
    if current_action >= act_asset.action.actions.len() {
        warn!(
            "Action index out of bounds for {:?}: action {} >= total_actions {}",
            hierarchy.layer_type,
            current_action,
            act_asset.action.actions.len()
        );
        return None;
    }

    let action_sequence = &act_asset.action.actions[current_action];

    // Calculate the correct frame index (handles doridori animation)
    let current_frame = calculate_layer_frame_index(
        &hierarchy.layer_type,
        current_action,
        character_sprite.current_frame as usize,
        action_sequence.animations.len(),
    );

    // Validate frame index
    if current_frame >= action_sequence.animations.len() {
        warn!(
            "Frame index out of bounds for {:?}: frame {} >= animation_count {}",
            hierarchy.layer_type,
            current_frame,
            action_sequence.animations.len()
        );
        return None;
    }

    let animation = &action_sequence.animations[current_frame];

    // Get the first layer (main sprite layer)
    let layer = animation.layers.first()?;

    // Handle negative sprite indices (use index 0 as fallback)
    // RO uses -1 to indicate "no sprite" or invisible layers
    let sprite_index = if layer.sprite_index < 0 {
        0
    } else {
        layer.sprite_index as usize
    };

    // Validate sprite index
    if sprite_index >= spr_asset.sprite.frames.len() {
        warn!(
            "Invalid sprite_index for {:?}: index={} (must be < {})",
            hierarchy.layer_type,
            sprite_index,
            spr_asset.sprite.frames.len()
        );
        return None;
    }

    let sprite_frame = &spr_asset.sprite.frames[sprite_index];

    Some(AnimationContext {
        spr_asset,
        sprite_frame,
        layer,
    })
}

/// Helper function to apply ACT layer transformations to entity transform
/// Handles position, scale, and rotation based on ACT data
fn apply_layer_transform(
    transform: &mut Transform,
    layer: &crate::infrastructure::ro_formats::act::Layer,
    sprite_frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
) {
    // Apply ACT positioning offset from layer data
    // ACT offsets are in pixel coordinates, scale to world units
    let offset_x = layer.pos[0] as f32 * SPRITE_WORLD_SCALE;
    let offset_y = -layer.pos[1] as f32 * SPRITE_WORLD_SCALE; // Flip Y for Bevy coordinate system
    transform.translation.x = offset_x;
    transform.translation.y = offset_y;
    // Keep z-offset from RoSpriteLayer (already set by spawn)

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
        transform.rotation =
            Quat::from_rotation_z(layer.angle as f32 * std::f32::consts::PI / 180.0);
    }
}

/// Helper function to create a material for a sprite layer
/// Converts sprite frame to image and creates StandardMaterial with proper settings
fn create_layer_material(
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    sprite_frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
    sprite_palette: &Option<crate::infrastructure::ro_formats::sprite::Palette>,
    layer: &crate::infrastructure::ro_formats::act::Layer,
) -> Handle<StandardMaterial> {
    // Convert SPR frame to Bevy Image
    let bevy_image = convert_sprite_frame_to_image(sprite_frame, sprite_palette);

    // Create new image handle
    let image_handle = images.add(bevy_image);

    // Create new material with the updated texture
    // This replaces the entire material to force Bevy's render system to update GPU bindings
    materials.add(StandardMaterial {
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
    })
}

/// Helper function to process a sprite layer with validated animation context
/// Applies transforms and creates materials
fn process_sprite_layer(
    ctx: &AnimationContext,
    transform: &mut Transform,
    entity: Entity,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Apply layer transformations (position, scale, rotation)
    apply_layer_transform(transform, ctx.layer, ctx.sprite_frame);

    // Create material with sprite texture
    let material = create_layer_material(
        images,
        materials,
        ctx.sprite_frame,
        &ctx.spr_asset.sprite.palette,
        ctx.layer,
    );

    // Replace the material component to trigger Bevy's change detection
    commands.entity(entity).insert(MeshMaterial3d(material));
}

// System to update sprite layer positions and textures based on animation
pub fn update_sprite_layer_transforms(
    mut commands: Commands,
    mut sprite_layers: Query<
        (
            Entity,
            &mut Transform,
            &CharacterSpriteHierarchy,
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
    for (entity, mut transform, hierarchy, controller) in sprite_layers.iter_mut() {
        // Get and validate animation context
        let Some(ctx) =
            get_animation_context(hierarchy, controller, &characters, &spr_assets, &act_assets)
        else {
            continue;
        };

        // Process the sprite layer with the validated context
        process_sprite_layer(
            &ctx,
            &mut transform,
            entity,
            &mut commands,
            &mut images,
            &mut materials,
        );
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

/// Helper function to find the first valid ACT asset handle from sprite layers
/// Returns None if no valid (non-weak) handle is found
fn find_act_handle<'a>(
    sprite_layers: &'a Query<&RoAnimationController, With<CharacterSpriteHierarchy>>,
) -> Option<&'a Handle<RoActAsset>> {
    sprite_layers
        .iter()
        .find(|controller| !controller.action_handle.is_weak())
        .map(|controller| &controller.action_handle)
}

/// Helper function to get the action sequence for a given action index
/// Returns None if the index is out of bounds
fn get_action_sequence(
    act_asset: &RoActAsset,
    action_index: usize,
) -> Option<&crate::infrastructure::ro_formats::act::ActionSequence> {
    act_asset.action.actions.get(action_index)
}

/// Helper function to advance the animation frame and update timer
/// Handles frame wraparound and minimum delay validation
fn advance_animation_frame(
    character_sprite: &mut crate::domain::entities::character::components::visual::CharacterSprite,
    action_sequence: &crate::infrastructure::ro_formats::act::ActionSequence,
) {
    let frame_count = action_sequence.animations.len();

    // Guard: Skip if no frames available
    if frame_count == 0 {
        return;
    }

    // Advance to next frame with wraparound
    character_sprite.current_frame = (character_sprite.current_frame + 1) % (frame_count as u8);

    // Update timer duration from ACT delay (convert from milliseconds to seconds)
    // Apply minimum delay of 0.1s to prevent infinite loops
    let delay_seconds = (action_sequence.delay / 1000.0).max(0.1);
    character_sprite
        .animation_timer
        .set_duration(std::time::Duration::from_secs_f32(delay_seconds));
    character_sprite.animation_timer.reset();
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
    // Optimization: Find ACT handle once outside the character loop (O(m) instead of O(n*m))
    // This works because all characters share the same ACT file format structure
    let Some(act_handle) = find_act_handle(&sprite_layers) else {
        // No valid ACT handle found - skip all character animations this frame
        return;
    };

    // Get the ACT asset once for all characters
    let Some(act_asset) = act_assets.get(act_handle) else {
        // ACT asset not yet loaded - skip this frame
        return;
    };

    // Process each character's animation state
    for mut character_sprite in character_sprites.iter_mut() {
        // Tick the animation timer
        character_sprite.animation_timer.tick(time.delta());

        // Skip if timer hasn't finished yet
        if !character_sprite.animation_timer.finished() {
            continue;
        }

        // Get the action sequence for the current action
        let current_action = character_sprite.current_action as usize;
        let Some(action_sequence) = get_action_sequence(act_asset, current_action) else {
            // Invalid action index - skip this character
            continue;
        };

        // Advance the animation frame and reset timer
        advance_animation_frame(&mut character_sprite, action_sequence);
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
pub fn populate_sprite_layers_with_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_layers: SpriteLayersNeedingAssets,
    characters: Query<(
        &super::components::core::CharacterData,
        &CharacterAppearance,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
    map_spawn_context: Option<Res<crate::domain::world::spawn_context::MapSpawnContext>>,
) {
    for (entity, hierarchy, layer_info) in sprite_layers.iter() {
        let Ok((char_data, appearance)) = characters.get(hierarchy.character_entity) else {
            warn!(
                "Failed to get character data for entity {:?}",
                hierarchy.character_entity
            );
            continue;
        };

        let (sprite_path, act_path, palette_path) = match &layer_info.layer_type {
            SpriteLayerType::Body => {
                let job_class = crate::domain::character::JobClass::from(char_data.job_id);
                let job_name = job_class.to_sprite_name();

                let sprite = body_sprite_path(appearance.gender, job_name);
                let act = body_action_path(appearance.gender, job_name);
                (sprite, act, None)
            }
            SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
            | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
            | SpriteLayerType::Equipment(EquipmentSlot::HeadTop) => {
                // For now, use head sprites for all head layers
                let sprite = head_sprite_path(appearance.gender, appearance.hair_style);
                let act = head_action_path(appearance.gender, appearance.hair_style);

                let palette = if appearance.hair_color > 0 {
                    Some(hair_palette_path(
                        appearance.hair_style,
                        appearance.gender,
                        appearance.hair_color,
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

        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
        let palette_handle = palette_path
            .as_ref()
            .map(|path| asset_server.load::<RoPaletteAsset>(path));

        // Determine if we should pause based on context
        // We're in InGame when MapSpawnContext exists and character is being spawned for gameplay
        // Default to NOT paused for in-game rendering
        let should_pause = map_spawn_context.is_none();

        let mut controller = RoAnimationController::new(sprite_handle.clone(), act_handle.clone())
            .with_action(0) // Idle action
            .looping(true)
            .paused(should_pause); // Not paused for in-game rendering

        if let Some(palette) = palette_handle {
            controller = controller.with_palette(palette);
        }

        let material = materials.add(StandardMaterial {
            base_color_texture: None, // Will be set by animation system
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

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
        for (hierarchy, layer_info, mut controller) in sprite_layers.iter_mut() {
            if hierarchy.character_entity != character_entity {
                continue;
            }

            let (sprite_path, act_path, palette_path) = match &layer_info.layer_type {
                SpriteLayerType::Body => {
                    let job_class = crate::domain::character::JobClass::from(char_data.job_id);
                    let job_name = job_class.to_sprite_name();

                    let sprite = body_sprite_path(appearance.gender, job_name);
                    let act = body_action_path(appearance.gender, job_name);
                    (sprite, act, None)
                }
                SpriteLayerType::Equipment(EquipmentSlot::HeadBottom)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadMid)
                | SpriteLayerType::Equipment(EquipmentSlot::HeadTop) => {
                    let sprite = head_sprite_path(appearance.gender, appearance.hair_style);
                    let act = head_action_path(appearance.gender, appearance.hair_style);

                    let palette = if appearance.hair_color > 0 {
                        Some(hair_palette_path(
                            appearance.hair_style,
                            appearance.gender,
                            appearance.hair_color,
                        ))
                    } else {
                        None
                    };

                    (sprite, act, palette)
                }
                _ => continue, // Skip other layer types
            };

            let new_sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
            let new_act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
            let palette_handle = palette_path
                .as_ref()
                .map(|path| asset_server.load::<RoPaletteAsset>(path));

            controller.sprite_handle = new_sprite_handle;
            controller.action_handle = new_act_handle;
            controller.palette_handle = palette_handle;
            controller.reset(); // Reset animation to frame 0
        }
    }
}

/// Creates a standard sprite material for character layers
fn create_sprite_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color_texture: None,
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

/// Generates the moonshine-object path for an equipment slot
fn equipment_layer_path(slot: EquipmentSlot) -> String {
    format!("Equipment/{:?}", slot)
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
                    populate_sprite_layers_with_assets,
                    update_sprite_layers_on_appearance_change,
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
