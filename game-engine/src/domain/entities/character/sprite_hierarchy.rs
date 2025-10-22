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
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
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
#[derive(Message)]
pub struct SpawnCharacterSpriteEvent {
    pub character_entity: Entity,
    pub spawn_position: Vec3,
}

#[derive(Message)]
pub struct EquipmentChangeEvent {
    pub character: Entity,
    pub slot: EquipmentSlot,
    pub new_item_id: Option<u32>,
}

#[derive(Message)]
pub struct StatusEffectVisualEvent {
    pub character: Entity,
    pub effect_type: EffectType,
    pub add: bool, // true to add, false to remove
}

#[derive(Message)]
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
    mut spawn_events: MessageReader<SpawnCharacterSpriteEvent>,
    character_query: Query<Entity>,
    mut rendering: RenderingResources,
    terrain: TerrainResources,
) {
    for event in spawn_events.read() {
        info!(
            "🎭 spawn_character_sprite_hierarchy: Received event for character entity {:?} at {:?}",
            event.character_entity, event.spawn_position
        );

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
            "Head",
            "Equipment/HeadBottom",
            "Equipment/HeadMid",
            "Equipment/HeadTop",
        ];

        let mut body_entity = Entity::PLACEHOLDER;
        let mut head_entity = Entity::PLACEHOLDER;

        for (i, layer_name) in layer_names.iter().enumerate() {
            let z_offset = i as f32 * rendering.config.default_z_spacing;

            // Create material for this layer
            let layer_material = create_sprite_material(&mut rendering.materials);

            let layer_entity = commands
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

            match *layer_name {
                "Body" => {
                    body_entity = layer_entity;
                    info!("🎭 Spawned Body layer: entity {:?}", layer_entity);
                }
                "Head" => {
                    head_entity = layer_entity;
                    info!("🎭 Spawned Head layer: entity {:?}", layer_entity);
                }
                _ => {}
            }
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

        // Add the object tree to the character entity and update CharacterSprite
        // Use a command closure to check entity existence at command-application time
        // This prevents panic if the entity was despawned between the check and the insert
        let character_entity = event.character_entity;
        commands.queue(move |world: &mut World| {
            if let Ok(mut entity_mut) = world.get_entity_mut(character_entity) {
                entity_mut.insert(object_tree);

                // Update CharacterSprite with body and head entities
                if let Some(mut character_sprite) = entity_mut.get_mut::<crate::domain::entities::character::components::visual::CharacterSprite>() {
                    character_sprite.body_sprite = body_entity;
                    character_sprite.head_sprite = head_entity;
                }
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
    mut equipment_events: MessageReader<EquipmentChangeEvent>,
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
    mut effect_events: MessageReader<StatusEffectVisualEvent>,
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
///
/// # Doridori Animation Structure
///
/// Head equipment ACT files contain directional idle animations with 3 head tilt variants:
/// - Total frames: 8 directions × 3 headDir variants = 24 frames
/// - Frame layout:
///   - Frames 0-7: headDir 0 (forward), directions South to SouthEast
///   - Frames 8-15: headDir 1 (looking up), directions South to SouthEast
///   - Frames 16-23: headDir 2 (looking down), directions South to SouthEast
///
/// # With Directional Body Animations
///
/// When body uses directional action indices (e.g., action 4 for Idle-North),
/// we extract the direction from the action index and map it to the head frame
/// using only headDir 0 to match the body's direction.
///
/// Formula: `direction % 8` gives us frame 0-7 (headDir 0 range)
fn calculate_layer_frame_index(
    layer_type: &SpriteLayerType,
    current_action: usize,
    base_frame: usize,
    animation_count: usize,
) -> usize {
    match layer_type {
        // Head equipment: doridori structure (24 frames = 8 directions × 3 tilts)
        // Equipment ACT files contain idle animations with 3 head tilt variants:
        // - Frames 0-7: headDir 0 (forward), directions South to SouthEast
        // - Frames 8-15: headDir 1 (looking up), directions South to SouthEast
        // - Frames 16-23: headDir 2 (looking down), directions South to SouthEast
        SpriteLayerType::Equipment(
            EquipmentSlot::HeadBottom | EquipmentSlot::HeadMid | EquipmentSlot::HeadTop,
        ) if animation_count > 0 => {
            let direction = current_action % 8;
            let frames_per_variant = animation_count / 3;
            if frames_per_variant >= 8 {
                // Use headDir 0 range (frames 0-7) to match body direction
                direction
            } else {
                base_frame
            }
        }

        // All other layers (body, head, shadow, effects) use base_frame from animation timer
        // Head and Body ACT structure: 103 actions with multiple frames per action
        // The sprite_index in each frame tells us which sprite to use
        _ => base_frame,
    }
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
    let character_sprite = characters.get(hierarchy.character_entity).ok()?;
    let spr_asset = spr_assets.get(&controller.sprite_handle)?;
    let act_asset = act_assets.get(&controller.action_handle)?;

    let current_action = character_sprite.current_action as usize;

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

    // Clamp frame to available frames (like reference code does)
    // This prevents child sprites (head) from stopping when they have fewer frames than parent (body)
    let current_frame = if current_frame >= action_sequence.animations.len() {
        action_sequence.animations.len().saturating_sub(1)
    } else {
        current_frame
    };

    let animation = &action_sequence.animations[current_frame];

    trace!("🎬 Animation for {:?}: action={}, frame={}, layer_count={}",
          hierarchy.layer_type, current_action, current_frame, animation.layers.len());

    // Find the first layer with a valid sprite_index (>= 0)
    // Layer 0 might be a dummy/anchor layer with sprite_index=-1
    let layer = animation.layers.iter()
        .find(|l| l.sprite_index >= 0)
        .or_else(|| animation.layers.first())?;

    trace!("   └─ Using layer: pos=[{}, {}], sprite_index={}",
          layer.pos[0], layer.pos[1], layer.sprite_index);

    let sprite_index = match &hierarchy.layer_type {
        SpriteLayerType::Head => {
            // Head ACT files have sprite_index=0 for all frames (confirmed by logs)
            // Direction is NOT encoded in ACT sprite_index
            // Must manually extract direction from action and map to sprite index
            let direction = current_action % 8;
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
        _ => {
            // Body and other layers use sprite_index from ACT file
            if layer.sprite_index < 0 {
                0
            } else {
                layer.sprite_index as usize
            }
        }
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
/// Uses the is_mirror flag from ACT data to determine sprite flipping
/// Optional anchor_x_offset is added to offset_x for child sprites (e.g., head positioning)
fn apply_layer_transform(
    transform: &mut Transform,
    layer: &crate::infrastructure::ro_formats::act::Layer,
    sprite_frame: &crate::infrastructure::ro_formats::sprite::SpriteFrame,
    layer_type: &SpriteLayerType,
    anchor_x_offset: Option<f32>,
) {
    trace!("📐 apply_layer_transform for {:?}: raw layer.pos=[{}, {}]",
          layer_type, layer.pos[0], layer.pos[1]);

    let base_offset_x = layer.pos[0] as f32 * SPRITE_WORLD_SCALE;
    let offset_x = base_offset_x + anchor_x_offset.unwrap_or(0.0);

    // Use ACT dimensions if specified (ACT v2.5+), otherwise fall back to SPR dimensions
    // ACT width/height provide the intended rendering dimensions for consistent positioning
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
    let offset_y = -layer.pos[1] as f32 * SPRITE_WORLD_SCALE;
    let scale_y = layer.scale[1] * sprite_height * SPRITE_WORLD_SCALE;

    if layer.is_mirror {
        scale_x = -scale_x;
    }

    trace!("   └─ Calculated offsets: base_offset_x={:.6}, anchor_offset={:.6}, offset_y={:.6}",
          base_offset_x, anchor_x_offset.unwrap_or(0.0), offset_y);

    transform.translation.x = offset_x;
    transform.translation.y = offset_y;
    transform.scale = Vec3::new(scale_x, scale_y, 1.0);

    trace!("   └─ Final transform: pos=({:.6}, {:.6}), scale=({:.3}, {:.3})",
          transform.translation.x, transform.translation.y, scale_x, scale_y);

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

/// Helper function to get the current animation from an AnimationContext
/// Returns the full animation object which includes anchor point data
fn get_current_animation<'a>(
    hierarchy: &CharacterSpriteHierarchy,
    _controller: &RoAnimationController,
    character_sprite: &crate::domain::entities::character::components::visual::CharacterSprite,
    act_asset: &'a RoActAsset,
) -> Option<&'a crate::infrastructure::ro_formats::act::Animation> {
    let current_action = character_sprite.current_action as usize;

    if current_action >= act_asset.action.actions.len() {
        return None;
    }

    let action_sequence = &act_asset.action.actions[current_action];

    let current_frame = calculate_layer_frame_index(
        &hierarchy.layer_type,
        current_action,
        character_sprite.current_frame as usize,
        action_sequence.animations.len(),
    );

    let current_frame = if current_frame >= action_sequence.animations.len() {
        action_sequence.animations.len().saturating_sub(1)
    } else {
        current_frame
    };

    action_sequence.animations.get(current_frame)
}

/// Helper function to process a sprite layer with validated animation context
/// Applies transforms and creates materials
fn process_sprite_layer(
    ctx: &AnimationContext,
    layer: &crate::infrastructure::ro_formats::act::Layer,
    transform: &mut Transform,
    entity: Entity,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    layer_type: &SpriteLayerType,
    anchor_x_offset: Option<f32>,
) {
    apply_layer_transform(transform, layer, ctx.sprite_frame, layer_type, anchor_x_offset);

    let material = create_layer_material(
        images,
        materials,
        ctx.sprite_frame,
        &ctx.spr_asset.sprite.palette,
        layer,
    );

    commands.entity(entity).insert(MeshMaterial3d(material));
}

// System to update sprite layer positions and textures based on animation
// Performance: O(n) complexity through character grouping strategy
// Groups sprite layers by character_entity using HashMap, calculates anchor offset ONCE per character,
// then processes all layers for that character in a single pass
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
    // Strategy: Group sprite layers by character entity to reduce O(n²) to O(n)
    // Step 1: Build character groups and calculate anchor offsets ONCE per character
    // Step 2: Process all layers using pre-calculated anchor offsets

    let mut character_anchor_offsets: HashMap<Entity, Option<f32>> = HashMap::new();

    // First pass: Group layers and calculate anchor offsets per character
    let mut character_groups: HashMap<Entity, Vec<Entity>> = HashMap::new();

    for (entity, _, hierarchy, _) in sprite_layers.iter() {
        character_groups
            .entry(hierarchy.character_entity)
            .or_default()
            .push(entity);
    }

    // Calculate anchor offsets for each character (O(n) total, not O(n²))
    for (character_entity, layer_entities) in character_groups.iter() {
        let mut body_anchor_x: Option<i32> = None;
        let mut head_anchor_x: Option<i32> = None;

        let Ok(character_sprite) = characters.get(*character_entity) else {
            continue;
        };

        // Single iteration through this character's layers to find both anchors
        for &layer_entity in layer_entities.iter() {
            let Ok((_, _, hierarchy, controller)) = sprite_layers.get(layer_entity) else {
                continue;
            };

            let Some(act_asset) = act_assets.get(&controller.action_handle) else {
                continue;
            };

            let Some(animation) = get_current_animation(hierarchy, controller, character_sprite, act_asset) else {
                continue;
            };

            let Some(anchor) = animation.positions.first() else {
                continue;
            };

            match &hierarchy.layer_type {
                SpriteLayerType::Body => {
                    body_anchor_x = Some(anchor.x);
                }
                SpriteLayerType::Head => {
                    head_anchor_x = Some(anchor.x);
                }
                _ => {}
            }

            // Early exit if we found both anchors
            if body_anchor_x.is_some() && head_anchor_x.is_some() {
                break;
            }
        }

        let anchor_offset = if let (Some(body_x), Some(head_x)) = (body_anchor_x, head_anchor_x) {
            Some((body_x - head_x) as f32 * SPRITE_WORLD_SCALE)
        } else {
            None
        };

        character_anchor_offsets.insert(*character_entity, anchor_offset);
    }

    // Second pass: Process all sprite layers with pre-calculated anchor offsets
    for (entity, mut transform, hierarchy, controller) in sprite_layers.iter_mut() {
        let Some(ctx) = get_animation_context(
            hierarchy,
            controller,
            &characters,
            &spr_assets,
            &act_assets,
        ) else {
            warn!(
                "❌ update_sprite_layer_transforms: Failed to get animation context for entity {:?}, type {:?}",
                entity, hierarchy.layer_type
            );
            continue;
        };

        // Determine if this layer uses anchor offset (only Head sprites)
        let layer_anchor_offset = if matches!(hierarchy.layer_type, SpriteLayerType::Head) {
            character_anchor_offsets
                .get(&hierarchy.character_entity)
                .and_then(|&offset| offset)
        } else {
            None
        };

        process_sprite_layer(
            &ctx,
            ctx.layer,
            &mut transform,
            entity,
            &mut commands,
            &mut images,
            &mut materials,
            &hierarchy.layer_type,
            layer_anchor_offset,
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
        rgba_data.extend_from_slice(&frame.data);
    } else if let Some(palette) = palette {
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
        .find(|controller| controller.action_handle.is_strong())
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
/// During idle animations (actions 0-7), keeps frame locked at 0 to prevent doridori head nodding
fn advance_animation_frame(
    character_sprite: &mut crate::domain::entities::character::components::visual::CharacterSprite,
    action_sequence: &crate::infrastructure::ro_formats::act::ActionSequence,
    current_action: usize,
) {
    let frame_count = action_sequence.animations.len();

    if frame_count == 0 {
        return;
    }

    // Actions 0-7 are directional idle animations
    // During idle, keep frame at 0 (headDir 0 - forward facing)
    // This prevents doridori head nodding animation from auto-playing
    // Reference: In RO, HeadFacing is a state (Center/Right/Left), not an auto-cycling animation
    let is_idle = current_action < 8;

    if is_idle {
        character_sprite.current_frame = 0;
    } else {
        // Normal frame advancement for non-idle animations
        character_sprite.current_frame = (character_sprite.current_frame + 1) % (frame_count as u8);
    }

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
    mut character_sprites: Query<(
        Entity,
        &mut crate::domain::entities::character::components::visual::CharacterSprite,
    )>,
    act_assets: Res<Assets<crate::infrastructure::assets::loaders::RoActAsset>>,
    sprite_layers: Query<&RoAnimationController, With<CharacterSpriteHierarchy>>,
) {
    let Some(act_handle) = find_act_handle(&sprite_layers) else {
        return;
    };

    let Some(act_asset) = act_assets.get(act_handle) else {
        return;
    };

    for (entity, mut character_sprite) in character_sprites.iter_mut() {
        character_sprite.animation_timer.tick(time.delta());

        if !character_sprite.animation_timer.is_finished() {
            continue;
        }

        let current_action = character_sprite.current_action as usize;
        let Some(action_sequence) = get_action_sequence(act_asset, current_action) else {
            warn!(
                "⚠️ advance_character_animations: Invalid action index {} for entity {:?}",
                current_action, entity
            );
            continue;
        };

        advance_animation_frame(&mut character_sprite, action_sequence, current_action);
    }
}

// System to handle sprite animation changes from state machine
pub fn handle_sprite_animation_changes(
    mut animation_events: MessageReader<SpriteAnimationChangeEvent>,
    mut character_sprites: Query<(
        &mut crate::domain::entities::character::components::visual::CharacterSprite,
        &crate::domain::entities::character::components::visual::CharacterDirection,
    )>,
) {
    for event in animation_events.read() {
        if let Ok((mut sprite, direction)) = character_sprites.get_mut(event.character_entity) {
            sprite.play_action(event.action_type, direction.facing);
        } else {
            warn!(
                "   └─ ❌ Failed to get CharacterSprite for entity {:?}",
                event.character_entity
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
        if characters.get(hierarchy.character_entity).is_err() {
            commands.entity(root_entity).despawn();
        }
    }
}

// Helper function to spawn a complete character sprite hierarchy
pub fn spawn_complete_character_sprite(
    spawn_events: &mut MessageWriter<SpawnCharacterSpriteEvent>,
    equipment_events: &mut MessageWriter<EquipmentChangeEvent>,
    character_entity: Entity,
    position: Vec3,
    equipment_slots: &HashMap<EquipmentSlot, u32>,
) {
    spawn_events.write(SpawnCharacterSpriteEvent {
        character_entity,
        spawn_position: position,
    });

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
            SpriteLayerType::Head => {
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
                SpriteLayerType::Head => {
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
            .add_message::<SpawnCharacterSpriteEvent>()
            .add_message::<EquipmentChangeEvent>()
            .add_message::<StatusEffectVisualEvent>()
            .add_message::<SpriteAnimationChangeEvent>()
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
