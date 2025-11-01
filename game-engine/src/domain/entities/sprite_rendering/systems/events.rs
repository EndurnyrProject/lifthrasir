use super::super::components::{EffectType, SpriteHierarchy, SpriteLayerType, SpriteObjectTree};
use super::super::kinds::{EffectLayer, SpriteLayer, SpriteRoot};
use crate::domain::entities::billboard::SharedSpriteQuad;
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use bevy::prelude::*;
use moonshine_object::prelude::*;

use super::super::components::SpriteHierarchyConfig;

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
    pub add: bool,
}

#[derive(Message)]
pub struct SpriteAnimationChangeEvent {
    pub character_entity: Entity,
    pub action_type: crate::domain::entities::character::components::visual::ActionType,
}

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
    ro_sprite_layer: crate::domain::entities::sprite_rendering::components::RoSpriteLayer,
    hierarchy: SpriteHierarchy,
}

impl SpriteLayerBaseBundle {
    fn new(
        name: String,
        material: Handle<StandardMaterial>,
        mesh: Handle<Mesh>,
        layer_type: SpriteLayerType,
        parent_entity: Entity,
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
            ro_sprite_layer: crate::domain::entities::sprite_rendering::components::RoSpriteLayer {
                layer_type: layer_type.clone(),
                z_offset,
                sprite_handle: Handle::default(),
                action_handle: Handle::default(),
            },
            hierarchy: SpriteHierarchy {
                parent_entity,
                layer_type,
            },
        }
    }
}

struct RenderingContext<'a> {
    config: &'a SpriteHierarchyConfig,
    materials: &'a mut Assets<StandardMaterial>,
    shared_quad: &'a SharedSpriteQuad,
}

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

fn equipment_layer_path(slot: EquipmentSlot) -> String {
    format!("Equipment/{:?}", slot)
}

fn create_equipment_bundle(
    slot: EquipmentSlot,
    parent_entity: Entity,
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
            parent_entity,
            z_offset,
        ),
    )
}

fn create_effect_bundle(
    effect_type: EffectType,
    parent_entity: Entity,
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
            parent_entity,
            z_offset,
        ),
    )
}

pub fn handle_equipment_changes(
    mut commands: Commands,
    entities: Query<&SpriteObjectTree>,
    sprite_objects: Objects<SpriteRoot>,
    mut equipment_events: MessageReader<EquipmentChangeEvent>,
    config: Res<SpriteHierarchyConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
) {
    for event in equipment_events.read() {
        let Ok(object_tree) = entities.get(event.character) else {
            warn!(
                "Entity {:?} not found for equipment change",
                event.character
            );
            continue;
        };

        let Ok(sprite_obj) = sprite_objects.get(object_tree.root) else {
            warn!("Sprite object root {:?} not found", object_tree.root);
            continue;
        };

        let equipment_path = equipment_layer_path(event.slot);

        remove_old_equipment(&mut commands, &sprite_obj, &equipment_path);

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

fn remove_old_equipment(
    commands: &mut Commands,
    sprite_obj: &Object<SpriteRoot>,
    equipment_path: &str,
) {
    if let Some(old_equipment) = sprite_obj.find_by_path(equipment_path) {
        commands.entity(old_equipment.entity()).despawn();
    }
}

fn spawn_equipment_layer(
    commands: &mut Commands,
    slot: EquipmentSlot,
    parent_entity: Entity,
    root_entity: Entity,
    rendering: &mut RenderingContext,
) {
    let material = create_sprite_material(rendering.materials);
    let (marker, bundle) = create_equipment_bundle(
        slot,
        parent_entity,
        material,
        rendering.shared_quad.mesh.clone(),
        rendering.config,
    );

    commands
        .spawn((marker, bundle))
        .insert(ChildOf(root_entity));
}

pub fn handle_status_effect_visuals(
    mut commands: Commands,
    entities: Query<&SpriteObjectTree>,
    sprite_objects: Objects<SpriteRoot>,
    mut effect_events: MessageReader<StatusEffectVisualEvent>,
    config: Res<SpriteHierarchyConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
) {
    for event in effect_events.read() {
        let Ok(object_tree) = entities.get(event.character) else {
            warn!("Entity {:?} not found for effect visual", event.character);
            continue;
        };

        let Ok(sprite_obj) = sprite_objects.get(object_tree.root) else {
            warn!("Sprite object root {:?} not found", object_tree.root);
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
                &sprite_obj,
                &effect_path,
                event.effect_type,
                event.character,
                &mut rendering,
            );
        } else {
            remove_status_effect(&mut commands, &sprite_obj, &effect_path);
        }
    }
}

fn add_status_effect(
    commands: &mut Commands,
    sprite_obj: &Object<SpriteRoot>,
    effect_path: &str,
    effect_type: EffectType,
    parent_entity: Entity,
    rendering: &mut RenderingContext,
) {
    if sprite_obj.find_by_path(effect_path).is_some() {
        return;
    }

    let Some(effects_container) = sprite_obj.find_by_path("Effects") else {
        warn!("Effects container not found");
        return;
    };

    let effect_count = effects_container.children().count();
    let z_offset = effect_count as f32 * rendering.config.default_z_spacing;

    let material = create_sprite_material(rendering.materials);
    let (marker, bundle) = create_effect_bundle(
        effect_type,
        parent_entity,
        material,
        rendering.shared_quad.mesh.clone(),
        z_offset,
    );

    commands
        .spawn((marker, bundle))
        .insert(ChildOf(effects_container.entity()));
}

fn remove_status_effect(
    commands: &mut Commands,
    sprite_obj: &Object<SpriteRoot>,
    effect_path: &str,
) {
    if let Some(effect_entity) = sprite_obj.find_by_path(effect_path) {
        commands.entity(effect_entity.entity()).despawn();
    }
}

pub fn handle_sprite_animation_changes(
    mut animation_events: MessageReader<SpriteAnimationChangeEvent>,
    mut character_sprites: Query<
        (
            &mut crate::domain::entities::character::components::visual::CharacterSprite,
            &crate::domain::entities::character::components::visual::CharacterDirection,
        ),
        With<crate::domain::entities::character::components::CharacterAppearance>,
    >,
) {
    for event in animation_events.read() {
        if let Ok((mut sprite, direction)) = character_sprites.get_mut(event.character_entity) {
            sprite.play_action(event.action_type, direction.facing);
        }
    }
}
