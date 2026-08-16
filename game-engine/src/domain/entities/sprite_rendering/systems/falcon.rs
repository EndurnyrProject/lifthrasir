use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{SkillEffectShown, UnitEntered, UnitStateChanged};

use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
use crate::domain::entities::character::systems::OPTION_FALCON;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::asset_bank::SpriteAssetBank;
use crate::domain::entities::sprite_rendering::components::{
    FalconLayer, PlayerSprite, RenderLayer,
};
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::domain::sprite::tags::{
    LAYER_FALCON, PIXELS_PER_METRE, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER, layer_depth_bias,
    layer_order,
};
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::paths;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FalconFlightState {
    Perched,
    Outbound { origin: Vec3, position: Vec3 },
    Strike { position: Vec3 },
    Returning { origin: Vec3, position: Vec3 },
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FalconFlight {
    pub state: FalconFlightState,
    pub target: Option<Entity>,
    pub elapsed: f32,
}

impl Default for FalconFlight {
    fn default() -> Self {
        Self {
            state: FalconFlightState::Perched,
            target: None,
            elapsed: 0.0,
        }
    }
}

impl FalconFlight {
    fn position(&self, rest: Vec3) -> Vec3 {
        match self.state {
            FalconFlightState::Perched => rest,
            FalconFlightState::Outbound { position, .. }
            | FalconFlightState::Strike { position }
            | FalconFlightState::Returning { position, .. } => position,
        }
    }
}

const FALCON_REST_OFFSET: Vec3 = Vec3::new(
    PIXELS_PER_METRE * 2.0,
    SPRITE_BASE_Y_OFFSET - PIXELS_PER_METRE * 2.0,
    0.0,
);
const FALCON_FOLLOW_RATE: f32 = 6.0;
const FALCON_OUTBOUND_DURATION: f32 = 0.3;
const FALCON_STRIKE_DURATION: f32 = 0.15;
const FALCON_RETURN_DURATION: f32 = 0.35;
const HT_BLITZBEAT: u32 = 129;
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
    config(in_set = SpriteRenderingSystems::AnimationEvents)
)]
pub fn trigger_falcon_swoop(
    mut effects: MessageReader<SkillEffectShown>,
    registry: Res<EntityRegistry>,
    mut flights: Query<&mut FalconFlight>,
    follows: Query<(&ChildOf, &FalconFollow)>,
) {
    for effect in effects
        .read()
        .filter(|effect| effect.skill_id == HT_BLITZBEAT)
    {
        let (Some(owner), Some(target)) = (
            registry.get_entity(effect.src_id),
            registry.get_entity(effect.target_id),
        ) else {
            continue;
        };
        let Ok(mut flight) = flights.get_mut(owner) else {
            continue;
        };
        // A perched falcon renders at rest + lag, not at rest, so seeding the
        // swoop from the bare rest offset would snap it by the whole trail the
        // instant a moving Hunter casts. Lag is owner-uniform (see
        // `FalconFollow`), so any one quad's value is the falcon's. Mid-flight
        // lag is already zero, so only the perched case needs it.
        let current = match flight.state {
            FalconFlightState::Perched => {
                let lag = follows
                    .iter()
                    .find(|(child_of, _)| child_of.parent() == owner)
                    .map(|(_, follow)| follow.lag)
                    .unwrap_or(Vec3::ZERO);
                FALCON_REST_OFFSET + lag
            }
            _ => flight.position(FALCON_REST_OFFSET),
        };
        flight.state = FalconFlightState::Outbound {
            origin: current,
            position: current,
        };
        flight.target = Some(target);
        flight.elapsed = 0.0;
    }
}

#[auto_add_system(
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AssetPopulation)
)]
pub fn finalize_falcon_layer(
    mut commands: Commands,
    mut bank: SpriteAssetBank,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shared_quad: Res<SharedSpriteQuad>,
    pending_layers: Query<(Entity, &FalconAnimationPending, &ChildOf), With<FalconLayer>>,
) {
    for (entity, pending, child_of) in &pending_layers {
        let (Some(sprite), Some(action)) = (
            bank.sprites.get(&pending.spr),
            bank.actions.get(&pending.act),
        ) else {
            continue;
        };

        let animation = RoAnimationProcessor::process(
            &sprite.sprite,
            &action.action,
            None,
            LAYER_FALCON,
            &mut bank.images,
            bank.settings.upscaling,
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
        let animation = bank.animations.add(animation);
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

fn advance_falcon_flight(
    flight: &mut FalconFlight,
    dt: f32,
    owner: &GlobalTransform,
    target: Option<&GlobalTransform>,
    rest: Vec3,
) -> Vec3 {
    flight.elapsed += dt;
    flight.state = match flight.state {
        FalconFlightState::Perched => FalconFlightState::Perched,
        FalconFlightState::Outbound { origin, position } => {
            let Some(target) = target else {
                flight.elapsed = 0.0;
                flight.target = None;
                return set_returning(flight, position);
            };
            let destination = owner
                .affine()
                .inverse()
                .transform_point3(target.translation());
            let t = (flight.elapsed / FALCON_OUTBOUND_DURATION).min(1.0);
            let position = origin.lerp(destination, t);
            if t >= 1.0 {
                flight.elapsed = 0.0;
                FalconFlightState::Strike { position }
            } else {
                FalconFlightState::Outbound { origin, position }
            }
        }
        FalconFlightState::Strike { position } => {
            if target.is_none() || flight.elapsed >= FALCON_STRIKE_DURATION {
                flight.elapsed = 0.0;
                flight.target = None;
                FalconFlightState::Returning {
                    origin: position,
                    position,
                }
            } else {
                FalconFlightState::Strike { position }
            }
        }
        FalconFlightState::Returning { origin, .. } => {
            let t = (flight.elapsed / FALCON_RETURN_DURATION).min(1.0);
            let position = origin.lerp(rest, t);
            if t >= 1.0 {
                flight.elapsed = 0.0;
                FalconFlightState::Perched
            } else {
                FalconFlightState::Returning { origin, position }
            }
        }
    };
    flight.position(rest)
}

fn set_returning(flight: &mut FalconFlight, position: Vec3) -> Vec3 {
    flight.state = FalconFlightState::Returning {
        origin: position,
        position,
    };
    position
}

fn flight_direction(
    from: Vec3,
    to: Vec3,
    owner_world_facing: Direction,
    owner_display_facing: Direction,
) -> Option<Direction> {
    let movement = to - from;
    if movement.x.abs() < 0.01 && movement.z.abs() < 0.01 {
        return None;
    }
    let world = Direction::from_movement_vector(movement.x, movement.z);
    let camera_octant =
        (owner_display_facing as i16 - owner_world_facing as i16).rem_euclid(8) as u8;
    Some(Direction::from_u8((world as u8 + camera_octant) % 8))
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
    owner_query: Query<(&PlayerSprite, &CharacterDirection, &Transform), Without<FalconLayer>>,
    mut flight_query: Query<(Entity, &GlobalTransform, &mut FalconFlight)>,
    target_query: Query<&GlobalTransform>,
    mut falcon_query: FalconLayerQuery,
) {
    let game_time_ms = time.elapsed_secs() * 1000.0;
    let damping = (-FALCON_FOLLOW_RATE * time.delta_secs()).exp();
    let mut flight_offsets = HashMap::new();
    let mut flying = HashSet::new();

    for (owner_entity, owner_global, mut flight) in &mut flight_query {
        let target = flight
            .target
            .and_then(|entity| target_query.get(entity).ok());
        let previous = flight.position(FALCON_REST_OFFSET);
        let offset = advance_falcon_flight(
            &mut flight,
            time.delta_secs(),
            owner_global,
            target,
            FALCON_REST_OFFSET,
        );
        if flight.state != FalconFlightState::Perched {
            flying.insert(owner_entity);
        }
        flight_offsets.insert(owner_entity, (previous, offset));
    }

    for (layer, falcon, child_of, material_handle, mut follow, mut transform, mut visibility) in
        &mut falcon_query
    {
        let owner_entity = child_of.parent();
        let Ok((owner_sprite, owner_direction, owner_transform)) = owner_query.get(owner_entity)
        else {
            continue;
        };
        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        let direction = flight_offsets
            .get(&owner_entity)
            .and_then(|(previous, offset)| {
                flight_direction(
                    *previous,
                    *offset,
                    owner_direction.facing,
                    owner_sprite.direction,
                )
            })
            .unwrap_or(owner_sprite.direction);
        let Some(action) = animation.actions.get(direction as usize) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if action.frames.is_empty() {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        let flight_offset = flight_offsets
            .get(&owner_entity)
            .map(|(_, offset)| *offset)
            .unwrap_or(FALCON_REST_OFFSET);
        if flying.contains(&owner_entity) {
            // Flight owns the local offset; bypass and zero follow lag so the two
            // motion models cannot fight or leave drift when the falcon perches.
            follow.lag = Vec3::ZERO;
            follow.previous_owner_position = Some(owner_transform.translation);
        } else {
            if let Some(previous) = follow.previous_owner_position {
                follow.lag -= owner_transform.translation - previous;
                follow.lag = follow.lag.clamp_length_max(FALCON_MAX_LAG);
            }
            follow.previous_owner_position = Some(owner_transform.translation);
            follow.lag *= damping;
        }

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
                part.position.x * SPRITE_WORLD_SCALE + flight_offset.x + follow.lag.x,
                -part.position.y * SPRITE_WORLD_SCALE + flight_offset.y + follow.lag.y,
                part_z + flight_offset.z + follow.lag.z,
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
            commands.entity(entity).insert(FalconFlight::default());
            commands.spawn((
                FalconLayer { part: 0 },
                FalconAnimationPending {
                    spr: asset_server.load(paths::falcon_sprite_path()),
                    act: asset_server.load(paths::falcon_action_path()),
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
            commands.entity(entity).remove::<FalconFlight>();
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
    use net_contract::events::{
        SkillDamageReceived, SkillEffectShown, UnitEntered, UnitStateChanged,
    };

    use super::*;
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
            display_size: 0,
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
        assert!(!app.world().entity(unit).contains::<FalconFlight>());
    }

    fn trigger_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SkillEffectShown>()
            .add_message::<SkillDamageReceived>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, trigger_falcon_swoop);
        app
    }

    fn emit_blitz(app: &mut App, src_id: u32, target_id: u32) {
        app.world_mut().write_message(SkillEffectShown {
            skill_id: HT_BLITZBEAT,
            level: 5,
            src_id,
            target_id,
            result: 0,
        });
        app.update();
    }

    #[test]
    fn recast_system_restarts_from_current_position() {
        let mut app = trigger_app();
        let current = Vec3::new(9.0, -3.0, 0.0);
        let owner = app
            .world_mut()
            .spawn(FalconFlight {
                state: FalconFlightState::Strike { position: current },
                target: None,
                elapsed: 0.1,
            })
            .id();
        let target = app.world_mut().spawn_empty().id();
        register(&mut app, 7, owner);
        register(&mut app, 8, target);

        emit_blitz(&mut app, 7, 8);

        let flight = app.world().get::<FalconFlight>(owner).unwrap();
        assert_eq!(
            flight.state,
            FalconFlightState::Outbound {
                origin: current,
                position: current
            }
        );
        assert_eq!(flight.elapsed, 0.0);
    }

    #[test]
    fn swoop_from_a_moving_owner_starts_where_the_falcon_is_rendered() {
        let mut app = trigger_app();
        let lag = Vec3::new(-4.0, 1.5, 0.0);
        let owner = app.world_mut().spawn(FalconFlight::default()).id();
        app.world_mut().spawn((
            ChildOf(owner),
            FalconLayer { part: 0 },
            FalconFollow {
                previous_owner_position: None,
                lag,
            },
        ));
        let target = app.world_mut().spawn_empty().id();
        register(&mut app, 7, owner);
        register(&mut app, 8, target);

        emit_blitz(&mut app, 7, 8);

        // A trailing falcon must dive from where it is drawn, not snap back to
        // the rest offset first.
        let flight = app.world().get::<FalconFlight>(owner).unwrap();
        assert_eq!(
            flight.state,
            FalconFlightState::Outbound {
                origin: FALCON_REST_OFFSET + lag,
                position: FALCON_REST_OFFSET + lag,
            }
        );
    }

    #[test]
    fn damage_packets_do_not_restart_the_cast_swoop() {
        let mut app = trigger_app();
        let owner = app.world_mut().spawn(FalconFlight::default()).id();
        let target = app.world_mut().spawn_empty().id();
        register(&mut app, 7, owner);
        register(&mut app, 8, target);
        emit_blitz(&mut app, 7, 8);
        app.world_mut()
            .get_mut::<FalconFlight>(owner)
            .unwrap()
            .elapsed = 0.2;

        for _ in 0..3 {
            app.world_mut().write_message(SkillDamageReceived {
                skill_id: HT_BLITZBEAT,
                level: 5,
                src_id: 7,
                target_id: 8,
                server_tick: 0,
                damage: 100,
                div: 5,
                type_: 0,
                src_delay: 0,
                dst_delay: 0,
            });
            app.update();
        }

        assert_eq!(app.world().get::<FalconFlight>(owner).unwrap().elapsed, 0.2);
    }

    #[test]
    fn unresolved_caster_does_not_trigger_a_swoop() {
        let mut app = trigger_app();
        let target = app.world_mut().spawn_empty().id();
        register(&mut app, 8, target);

        emit_blitz(&mut app, 7, 8);

        assert!(
            !app.world()
                .iter_entities()
                .any(|entity| entity.contains::<FalconFlight>())
        );
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
            .spawn((
                PlayerSprite::default(),
                CharacterDirection::default(),
                Transform::default(),
                GlobalTransform::default(),
            ))
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

    fn global(position: Vec3) -> GlobalTransform {
        GlobalTransform::from_translation(position)
    }

    fn outbound(target: Entity) -> FalconFlight {
        FalconFlight {
            state: FalconFlightState::Outbound {
                origin: FALCON_REST_OFFSET,
                position: FALCON_REST_OFFSET,
            },
            target: Some(target),
            elapsed: 0.0,
        }
    }

    #[test]
    fn swoop_completes_full_cycle_without_drift() {
        let target_entity = Entity::from_bits(2);
        let owner = global(Vec3::new(10.0, 0.0, 0.0));
        let target = global(Vec3::new(20.0, 0.0, 0.0));
        let mut flight = outbound(target_entity);
        let mut states = Vec::new();

        for _ in 0..9 {
            advance_falcon_flight(&mut flight, 0.1, &owner, Some(&target), FALCON_REST_OFFSET);
            states.push(flight.state);
        }

        assert!(
            states
                .iter()
                .any(|state| matches!(state, FalconFlightState::Strike { .. }))
        );
        assert!(
            states
                .iter()
                .any(|state| matches!(state, FalconFlightState::Returning { .. }))
        );
        assert_eq!(flight.state, FalconFlightState::Perched);
        assert_eq!(flight.position(FALCON_REST_OFFSET), FALCON_REST_OFFSET);
    }

    #[test]
    fn lost_target_returns_from_current_position() {
        let target_entity = Entity::from_bits(2);
        let owner = global(Vec3::ZERO);
        let target = global(Vec3::new(20.0, 0.0, 0.0));
        let mut flight = outbound(target_entity);
        let current =
            advance_falcon_flight(&mut flight, 0.1, &owner, Some(&target), FALCON_REST_OFFSET);

        let after_loss = advance_falcon_flight(&mut flight, 0.1, &owner, None, FALCON_REST_OFFSET);

        assert!(matches!(flight.state, FalconFlightState::Returning { .. }));
        assert_eq!(after_loss, current);
    }

    #[test]
    fn outbound_recomputes_moving_target_position() {
        let target_entity = Entity::from_bits(2);
        let owner = global(Vec3::ZERO);
        let mut flight = outbound(target_entity);
        advance_falcon_flight(
            &mut flight,
            0.1,
            &owner,
            Some(&global(Vec3::new(10.0, 0.0, 0.0))),
            FALCON_REST_OFFSET,
        );
        let tracked = advance_falcon_flight(
            &mut flight,
            0.1,
            &owner,
            Some(&global(Vec3::new(20.0, 0.0, 0.0))),
            FALCON_REST_OFFSET,
        );

        assert!(tracked.x > 10.0);
    }

    #[test]
    fn recast_restarts_outbound_at_current_position() {
        let target_entity = Entity::from_bits(2);
        let owner = global(Vec3::ZERO);
        let target = global(Vec3::new(20.0, 0.0, 0.0));
        let mut flight = outbound(target_entity);
        let current =
            advance_falcon_flight(&mut flight, 0.1, &owner, Some(&target), FALCON_REST_OFFSET);

        flight.state = FalconFlightState::Outbound {
            origin: current,
            position: current,
        };
        flight.elapsed = 0.0;

        assert_eq!(flight.position(FALCON_REST_OFFSET), current);
        assert!(matches!(flight.state, FalconFlightState::Outbound { .. }));
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
