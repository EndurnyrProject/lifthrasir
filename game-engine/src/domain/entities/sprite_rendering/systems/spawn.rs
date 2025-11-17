use super::super::components::{RoSpriteLayer, SpriteHierarchy, SpriteLayerType, SpriteObjectTree};
use super::super::events::SpawnSpriteEvent;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::{
    equipment::EquipmentSlot, CharacterAppearance,
};
use crate::domain::entities::character::kinds::{CharacterRoot, SpriteLayer};
use crate::domain::entities::sprite_rendering::components::SpriteHierarchyConfig;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// SystemParam bundle for rendering resources
#[derive(bevy::ecs::system::SystemParam)]
pub struct RenderingResources<'w> {
    materials: ResMut<'w, Assets<StandardMaterial>>,
    shared_quad: Res<'w, SharedSpriteQuad>,
    _config: Res<'w, SpriteHierarchyConfig>,
}

/// SystemParam bundle for terrain-related resources
#[derive(bevy::ecs::system::SystemParam)]
pub struct TerrainResources<'w, 's> {
    map_loader_query: Query<'w, 's, &'static MapLoader>,
    altitude_assets: Res<'w, Assets<RoAltitudeAsset>>,
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

/// Helper function to create a material for a sprite layer
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

/// Helper function to spawn a single layer
#[allow(clippy::too_many_arguments)]
fn spawn_layer(
    commands: &mut Commands,
    name: &str,
    layer_type: SpriteLayerType,
    root_entity: Entity,
    parent_entity: Entity,
    z_offset: f32,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let layer_material = create_sprite_material(materials);

    commands
        .spawn((
            SpriteLayer,
            Name::new(name.to_string()),
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(layer_material),
            Transform::from_xyz(0.0, 0.0, z_offset),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            RoSpriteLayer {
                layer_type: layer_type.clone(),
                z_offset,
                sprite_handle: Handle::default(),
                action_handle: Handle::default(),
            },
            SpriteHierarchy {
                parent_entity,
                layer_type,
            },
        ))
        .insert(ChildOf(root_entity))
        .id()
}

/// Helper function to spawn the sprite root entity
fn spawn_sprite_root(
    commands: &mut Commands,
    entity: Entity,
    world_position: Vec3,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) -> Entity {
    commands
        .spawn((
            CharacterRoot,
            Name::new("SpriteRoot"),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(world_position),
            GlobalTransform::default(),
            Billboard,
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            SpriteHierarchy {
                parent_entity: entity,
                layer_type: SpriteLayerType::Body,
            },
        ))
        .id()
}

/// Helper function to spawn PC layers (body, head, equipment, effects)
fn spawn_pc_layers(
    commands: &mut Commands,
    root_entity: Entity,
    entity: Entity,
    rendering: &mut RenderingResources,
) {
    debug!("Spawning PC layers for entity {:?}", entity);

    // Body layer (z=0.0)
    spawn_layer(
        commands,
        "Body",
        SpriteLayerType::Body,
        root_entity,
        entity,
        0.0,
        &rendering.shared_quad,
        &mut rendering.materials,
    );

    // Head layer
    spawn_layer(
        commands,
        "Head",
        SpriteLayerType::Head,
        root_entity,
        entity,
        rendering._config.default_z_spacing,
        &rendering.shared_quad,
        &mut rendering.materials,
    );

    // Equipment layers using dynamic z-offset calculation
    let equipment_slots = [
        EquipmentSlot::HeadBottom,
        EquipmentSlot::HeadMid,
        EquipmentSlot::HeadTop,
        EquipmentSlot::Garment,
        EquipmentSlot::Weapon,
        EquipmentSlot::Shield,
    ];

    for slot in equipment_slots {
        let z_offset = slot.z_order() * rendering._config.default_z_spacing;
        spawn_layer(
            commands,
            &format!("Equipment/{:?}", slot),
            SpriteLayerType::Equipment(slot),
            root_entity,
            entity,
            z_offset,
            &rendering.shared_quad,
            &mut rendering.materials,
        );
    }

    // Effects container
    commands
        .spawn((
            Name::new("Effects"),
            Transform::from_xyz(0.0, 0.0, rendering._config.effect_z_offset),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .insert(ChildOf(root_entity));

    info!("🎭 PC layers created for entity {:?}", entity);
}

/// Helper function to spawn simple entity layers (single body layer for mobs/NPCs)
fn spawn_simple_entity_layers(
    commands: &mut Commands,
    root_entity: Entity,
    entity: Entity,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    spawn_layer(
        commands,
        "Body",
        SpriteLayerType::Body,
        root_entity,
        entity,
        0.0,
        shared_quad,
        materials,
    );
}

/// System to spawn generic sprite hierarchies
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update
)]
pub fn spawn_sprite_hierarchy(
    mut commands: Commands,
    mut spawn_events: MessageReader<SpawnSpriteEvent>,
    entity_query: Query<(Entity, Option<&CharacterAppearance>)>,
    mut rendering: RenderingResources,
    terrain: TerrainResources,
) {
    for event in spawn_events.read() {
        debug!(
            "spawn_sprite_hierarchy: Received event for entity {:?} at {:?}, type: {:?}",
            event.entity, event.position, event.sprite_info.sprite_data
        );

        let Ok((entity, appearance_opt)) = entity_query.get(event.entity) else {
            warn!(
                "Entity {:?} no longer exists, skipping sprite spawn",
                event.entity
            );
            continue;
        };

        let world_position = calculate_terrain_height(event.position, &terrain);
        let root_material = create_sprite_material(&mut rendering.materials);

        let root_entity = spawn_sprite_root(
            &mut commands,
            entity,
            world_position,
            rendering.shared_quad.mesh.clone(),
            root_material,
        );

        // Check if this is a PC (has CharacterAppearance)
        if appearance_opt.is_some() {
            spawn_pc_layers(&mut commands, root_entity, entity, &mut rendering);
        } else {
            spawn_simple_entity_layers(
                &mut commands,
                root_entity,
                entity,
                &rendering.shared_quad,
                &mut rendering.materials,
            );
        }

        let object_tree = SpriteObjectTree { root: root_entity };
        let sprite_info = event.sprite_info.clone();

        // Insert immediately instead of queuing to ensure populate_sprite_assets can access it in the same frame
        commands.entity(entity).insert((object_tree, sprite_info));
    }
}
