use std::collections::HashSet;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{UnitEntered, UnitStateChanged};

use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::systems::OPTION_FALCON;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::{
    FalconLayer, PlayerSprite, RenderLayer,
};
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::domain::settings::GraphicsSettings;
use crate::domain::sprite::tags::{
    LAYER_FALCON, PIXELS_PER_METRE, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER, layer_depth_bias,
    layer_order,
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

/// Trailing state, held per quad.
///
/// Every quad of the falcon integrates the *same* lag from the same owner
/// motion, so they stay locked together. That only holds while every early
/// return in `sync_falcon_layer` before the lag update is owner-uniform - one
/// that skips some quads but not others would let them drift apart and visibly
/// shear the sprite. Keep per-part bail-outs after the lag update.
#[derive(Component, Default)]
pub struct FalconFollow {
    previous_owner_position: Option<Vec3>,
    lag: Vec3,
}

const FALCON_REST_OFFSET: Vec3 = Vec3::new(
    PIXELS_PER_METRE * 2.0,
    SPRITE_BASE_Y_OFFSET - PIXELS_PER_METRE * 2.0,
    0.0,
);
const FALCON_FOLLOW_RATE: f32 = 6.0;
/// Largest trail the falcon may accumulate, in world units.
///
/// Ordinary walking never approaches this. It exists for a same-map teleport
/// (Warp Portal), where the owner's single-frame delta would otherwise fling
/// the falcon the entire warp distance and glide it back over a second. A
/// map change despawns the falcon outright, so only the same-map case matters.
const FALCON_MAX_LAG: f32 = PIXELS_PER_METRE * 8.0;

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
                FalconFollow::default(),
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

type FalconLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static FalconLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut FalconFollow,
        &'static mut Transform,
        &'static mut Visibility,
    ),
>;

/// Drives the falcon's idle flap and keeps it hovering beside its owner. Owner
/// movement first displaces the child by the inverse delta, then exponential
/// damping returns it to its resting offset without depending on frame rate.
#[auto_add_system(
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_falcon_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    owner_query: Query<(&PlayerSprite, &Transform), Without<FalconLayer>>,
    mut falcon_query: FalconLayerQuery,
) {
    let game_time_ms = time.elapsed_secs() * 1000.0;
    let damping = (-FALCON_FOLLOW_RATE * time.delta_secs()).exp();

    for (layer, falcon, child_of, material_handle, mut follow, mut transform, mut visibility) in
        &mut falcon_query
    {
        let Ok((owner_sprite, owner_transform)) = owner_query.get(child_of.parent()) else {
            continue;
        };
        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        let Some(action) = animation.actions.get(owner_sprite.direction as usize) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if action.frames.is_empty() {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        if let Some(previous) = follow.previous_owner_position {
            follow.lag -= owner_transform.translation - previous;
            follow.lag = follow.lag.clamp_length_max(FALCON_MAX_LAG);
        }
        follow.previous_owner_position = Some(owner_transform.translation);
        follow.lag *= damping;

        let delay = action.delay_ms.max(1.0);
        let frame_index = (game_time_ms / delay) as usize % action.frames.len();
        let Some(part) = action.frames[frame_index].parts.get(falcon.part) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        if let Some(texture) = animation.textures.get(part.texture_index) {
            set_layer_texture(&mut materials, &material_handle.0, texture);
        }

        let scale_x = if part.mirror {
            -part.scale.x
        } else {
            part.scale.x
        } * part.texture_size.x
            * SPRITE_WORLD_SCALE;
        let part_z =
            layer_order(LAYER_FALCON) as f32 * Z_OFFSET_PER_LAYER + falcon.part as f32 * 0.001;
        let current = *transform;
        transform.set_if_neq(Transform {
            translation: Vec3::new(
                part.position.x * SPRITE_WORLD_SCALE + FALCON_REST_OFFSET.x + follow.lag.x,
                -part.position.y * SPRITE_WORLD_SCALE + FALCON_REST_OFFSET.y + follow.lag.y,
                part_z + follow.lag.z,
            ),
            scale: Vec3::new(
                scale_x,
                part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE,
                1.0,
            ),
            ..current
        });
        visibility.set_if_neq(Visibility::Inherited);
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
    use std::time::Duration;

    use bevy::prelude::*;
    use net_contract::events::{UnitEntered, UnitStateChanged};

    use super::*;
    use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
    use crate::domain::entities::character::systems::OPTION_FALCON;
    use crate::domain::entities::registry::EntityRegistry;
    use crate::domain::entities::sprite_rendering::components::FalconLayer;
    use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
    use crate::infrastructure::assets::ro_animation_asset::{ActionData, FrameData, FramePart};

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

    fn sync_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAnimationAsset>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(100),
            ))
            .add_systems(Update, sync_falcon_layer);
        app
    }

    fn part(position: Vec2, texture_index: usize) -> FramePart {
        FramePart {
            texture_index,
            transform: Mat4::IDENTITY,
            position,
            scale: Vec2::ONE,
            texture_size: Vec2::ONE,
            color: Color::WHITE,
            mirror: false,
        }
    }

    fn spawn_synced_falcon(app: &mut App, actions: Vec<ActionData>) -> (Entity, Entity) {
        let textures = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            vec![images.add(Image::default()), images.add(Image::default())]
        };
        let animation = app
            .world_mut()
            .resource_mut::<Assets<RoAnimationAsset>>()
            .add(RoAnimationAsset {
                textures: textures.clone(),
                actions,
                layer: LAYER_FALCON,
                sounds: Vec::new(),
            });
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let owner = app
            .world_mut()
            .spawn((PlayerSprite::default(), Transform::default()))
            .id();
        let falcon = app
            .world_mut()
            .spawn((
                RenderLayer::body(animation, LAYER_FALCON, textures),
                FalconLayer { part: 0 },
                FalconFollow::default(),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::Hidden,
                ChildOf(owner),
            ))
            .id();
        (owner, falcon)
    }

    fn idle_actions(frame_positions: &[Vec2]) -> Vec<ActionData> {
        (0..8)
            .map(|_| ActionData {
                frames: frame_positions
                    .iter()
                    .enumerate()
                    .map(|(index, position)| FrameData {
                        parts: vec![part(*position, index % 2)],
                        ..default()
                    })
                    .collect(),
                delay_ms: 100.0,
            })
            .collect()
    }

    #[test]
    fn stationary_falcon_settles_at_resting_offset() {
        let mut app = sync_app();
        let (_, falcon) = spawn_synced_falcon(&mut app, idle_actions(&[Vec2::ZERO]));

        app.update();

        let translation = app.world().get::<Transform>(falcon).unwrap().translation;
        assert_eq!(translation.x, FALCON_REST_OFFSET.x);
        assert_eq!(translation.y, FALCON_REST_OFFSET.y);
        assert!(translation.y < 0.0, "world-up hover must use negative Y");
    }

    #[test]
    fn falcon_lags_after_owner_moves_then_converges() {
        let mut app = sync_app();
        let (owner, falcon) = spawn_synced_falcon(&mut app, idle_actions(&[Vec2::ZERO]));
        app.update();

        app.world_mut()
            .get_mut::<Transform>(owner)
            .unwrap()
            .translation
            .x = 10.0;
        app.update();
        let lagged_x = app.world().get::<Transform>(falcon).unwrap().translation.x;
        assert!(lagged_x < FALCON_REST_OFFSET.x);

        for _ in 0..20 {
            app.update();
        }
        let settled_x = app.world().get::<Transform>(falcon).unwrap().translation.x;
        assert!((settled_x - FALCON_REST_OFFSET.x).abs() < 0.001);
    }

    #[test]
    fn falcon_idle_action_advances_frames() {
        let mut app = sync_app();
        let (_, falcon) =
            spawn_synced_falcon(&mut app, idle_actions(&[Vec2::ZERO, Vec2::new(5.0, 0.0)]));

        app.update();
        let first = app.world().get::<Transform>(falcon).unwrap().translation.x;
        app.update();
        let second = app.world().get::<Transform>(falcon).unwrap().translation.x;

        assert_ne!(first, second);
    }

    #[test]
    fn falcon_uses_owners_camera_relative_facing() {
        let mut app = sync_app();
        let mut actions = idle_actions(&[Vec2::ZERO]);
        actions[Direction::West as usize].frames[0].parts[0]
            .position
            .x = 7.0;
        let (owner, falcon) = spawn_synced_falcon(&mut app, actions);
        app.world_mut()
            .entity_mut(owner)
            .insert(CharacterDirection {
                facing: Direction::South,
            });
        app.world_mut()
            .get_mut::<PlayerSprite>(owner)
            .unwrap()
            .direction = Direction::West;

        app.update();

        let x = app.world().get::<Transform>(falcon).unwrap().translation.x;
        assert_eq!(x, FALCON_REST_OFFSET.x + 7.0 * SPRITE_WORLD_SCALE);
    }
}
