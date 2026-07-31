use std::collections::HashSet;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{UnitEntered, UnitStateChanged};

use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::systems::OPTION_FALCON;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::{FalconLayer, RenderLayer};
use crate::domain::settings::GraphicsSettings;
use crate::domain::sprite::tags::{
    LAYER_FALCON, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER, layer_depth_bias, layer_order,
};
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

/// SPR/ACT handles loading for a falcon staging child.
///
/// The falcon cannot use the shared `PendingAnimations` queue the way the body
/// layers do: its quad count is not known until the ACT has been processed, so
/// mount spawns a single staging child and `finalize_falcon_layer` fans it out
/// into one quad per part. Keep the handles on the child - collapsing this back
/// into the shared queue reintroduces the guess at how many parts a falcon has.
#[derive(Component)]
pub struct FalconAnimationPending {
    spr: Handle<RoSpriteAsset>,
    act: Handle<RoActAsset>,
}

type FalconOwnerQuery<'w, 's> = Query<'w, 's, (Entity, &'static ChildOf), With<FalconLayer>>;

#[auto_add_system(
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(
        in_set = SpriteRenderingSystems::AnimationEvents,
        after = EntityLifecycleSystems::Spawning
    )
)]
pub fn apply_falcon_mount(
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    falcon_layers: FalconOwnerQuery,
    mut handled: Local<HashSet<Entity>>,
) {
    handled.clear();

    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            debug!(
                "falcon: UnitStateChanged for unresolved unit {} (effect_state={:#x}) dropped",
                event.unit_id, event.effect_state
            );
            continue;
        };
        if !handled.insert(entity) {
            continue;
        }
        apply_falcon_state(
            entity,
            event.effect_state,
            &falcon_layers,
            &mut commands,
            &asset_server,
        );
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        if !handled.insert(entity) {
            continue;
        }
        apply_falcon_state(
            entity,
            event.effect_state,
            &falcon_layers,
            &mut commands,
            &asset_server,
        );
    }
}

#[auto_add_system(
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AssetPopulation)
)]
#[allow(clippy::too_many_arguments)]
pub fn finalize_falcon_layer(
    mut commands: Commands,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
    settings: Res<GraphicsSettings>,
    pending_layers: Query<(Entity, &FalconAnimationPending, &ChildOf), With<FalconLayer>>,
) {
    for (entity, pending, child_of) in &pending_layers {
        let (Some(sprite), Some(action)) = (sprites.get(&pending.spr), actions.get(&pending.act))
        else {
            continue;
        };

        let animation = RoAnimationProcessor::process(
            &sprite.sprite,
            &action.action,
            LAYER_FALCON,
            &mut images,
            settings.upscaling,
        );
        let initial_parts = animation
            .actions
            .iter()
            .flat_map(|action| &action.frames)
            .max_by_key(|frame| frame.parts.len())
            .map(|frame| frame.parts.clone())
            .unwrap_or_default();

        if initial_parts.is_empty() {
            warn!("falcon: processed animation has no visible parts for {entity:?}");
            commands.entity(entity).despawn();
            continue;
        }

        let textures = animation.textures.clone();
        let animation = animations.add(animation);
        let parent = child_of.parent();

        for (part, part_data) in initial_parts.iter().enumerate() {
            let material = materials.add(StandardMaterial {
                base_color_texture: textures.get(part_data.texture_index).cloned(),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                depth_bias: layer_depth_bias(LAYER_FALCON) + part as f32 * 0.01,
                ..default()
            });
            let scale_x = if part_data.mirror {
                -part_data.scale.x
            } else {
                part_data.scale.x
            } * part_data.texture_size.x
                * SPRITE_WORLD_SCALE;
            let scale_y = part_data.scale.y * part_data.texture_size.y * SPRITE_WORLD_SCALE;
            let components = (
                Mesh3d(shared_quad.mesh.clone()),
                MeshMaterial3d(material),
                Billboard,
                RenderLayer::body(animation.clone(), LAYER_FALCON, textures.clone()),
                FalconLayer { part },
                Transform {
                    translation: Vec3::new(
                        part_data.position.x * SPRITE_WORLD_SCALE,
                        SPRITE_BASE_Y_OFFSET - part_data.position.y * SPRITE_WORLD_SCALE,
                        layer_order(LAYER_FALCON) as f32 * Z_OFFSET_PER_LAYER + part as f32 * 0.001,
                    ),
                    scale: Vec3::new(scale_x, scale_y, 1.0),
                    ..default()
                },
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
                ChildOf(parent),
            );

            if part == 0 {
                commands
                    .entity(entity)
                    .insert(components)
                    .remove::<FalconAnimationPending>();
            } else {
                commands.spawn(components);
            }
        }
    }
}

fn apply_falcon_state(
    entity: Entity,
    effect_state: u32,
    falcon_layers: &FalconOwnerQuery,
    commands: &mut Commands,
    asset_server: &AssetServer,
) {
    let mounted = effect_state & OPTION_FALCON != 0;
    let existing: Vec<Entity> = falcon_layers
        .iter()
        .filter(|(_, child_of)| child_of.parent() == entity)
        .map(|(child, _)| child)
        .collect();

    match (mounted, existing.is_empty()) {
        (true, true) => {
            commands.spawn((
                FalconLayer { part: 0 },
                FalconAnimationPending {
                    spr: asset_server.load(patterns::falcon_sprite_path()),
                    act: asset_server.load(patterns::falcon_action_path()),
                },
                Transform::from_translation(Vec3::new(
                    0.0,
                    SPRITE_BASE_Y_OFFSET,
                    layer_order(LAYER_FALCON) as f32 * Z_OFFSET_PER_LAYER,
                )),
                ChildOf(entity),
            ));
        }
        (false, false) => {
            for child in existing {
                commands.entity(child).despawn();
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use net_contract::events::{UnitEntered, UnitStateChanged};

    use super::*;
    use crate::domain::entities::character::systems::OPTION_FALCON;
    use crate::domain::entities::registry::EntityRegistry;
    use crate::domain::entities::sprite_rendering::components::FalconLayer;
    use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<RoSpriteAsset>()
            .init_asset::<RoActAsset>()
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, apply_falcon_mount);
        app
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit_state(app: &mut App, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id: 7,
                body_state: 0,
                health_state: 0,
                effect_state,
                virtue: 0,
            });
        app.update();
    }

    fn entered(effect_state: u32) -> UnitEntered {
        UnitEntered {
            gid: 7,
            aid: 0,
            object_type: 0,
            job: 0,
            x: 0,
            y: 0,
            dir: 0,
            speed: 0,
            hp: 0,
            max_hp: 0,
            clevel: 0,
            body_state: 0,
            health_state: 0,
            effect_state,
            virtue: 0,
            spirit_sphere_count: 0,
            head: 0,
            weapon: 0,
            shield: 0,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            head_palette: 0,
            body_palette: 0,
            head_dir: 0,
            robe: 0,
            guild_id: 0,
            guild_name: String::new(),
            emblem_id: 0,
            sex: 0,
            is_boss: false,
            name: String::new(),
            moving: false,
            dst_x: 0,
            dst_y: 0,
            move_start_time: 0,
        }
    }

    fn emit_entered(app: &mut App, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(entered(effect_state));
        app.update();
    }

    fn falcon_children(app: &mut App, parent: Entity) -> Vec<Entity> {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &ChildOf), With<FalconLayer>>();
        query
            .iter(app.world())
            .filter(|(_, child_of)| child_of.parent() == parent)
            .map(|(entity, _)| entity)
            .collect()
    }

    #[test]
    fn falcon_bit_spawns_a_falcon_child() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, OPTION_FALCON);

        assert_eq!(falcon_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn unit_entered_with_falcon_bit_spawns_a_falcon_child() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, OPTION_FALCON);

        assert_eq!(falcon_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn state_change_and_unit_entered_in_one_frame_spawn_one_falcon() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id: 7,
                body_state: 0,
                health_state: 0,
                effect_state: OPTION_FALCON,
                virtue: 0,
            });
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(entered(OPTION_FALCON));
        app.update();

        assert_eq!(falcon_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn clearing_falcon_bit_despawns_all_falcon_children() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_state(&mut app, OPTION_FALCON);
        emit_state(&mut app, 0);

        assert!(falcon_children(&mut app, unit).is_empty());
    }
}
