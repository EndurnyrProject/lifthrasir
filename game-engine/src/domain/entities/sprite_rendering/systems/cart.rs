use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_persistent::prelude::Persistent;
use net_contract::events::{UnitEntered, UnitStateChanged};

use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::visual::{ActionType, Direction};
use crate::domain::entities::character::systems::CART_MASK;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::{CartLayer, PlayerSprite, RenderLayer};
use crate::domain::settings::resources::Settings;
use crate::domain::sprite::tags::{
    layer_order, LAYER_CART, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER,
};
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

/// SPR/ACT handles still loading for a cart child. Kept on the child itself so
/// the cart never touches the shared `PendingAnimations` queue (whose whole-queue
/// drainers raced it); `finalize_cart_layer` polls these and removes the marker.
#[derive(Component)]
pub struct CartAnimationPending {
    spr: Handle<RoSpriteAsset>,
    act: Handle<RoActAsset>,
}

/// Query of the parent -> cart child relationship, keyed by the `CartLayer`
/// marker so the child's presence *is* the parent's mount state.
type CartOwnerQuery<'w, 's> = Query<'w, 's, (Entity, &'static ChildOf), With<CartLayer>>;

/// Spawns/despawns the pushcart layer from a unit's `effect_state` cart bits.
///
/// The cart is not equipment, so it rides neither the equipment nor the
/// body/head finalizer. It is a `LAYER_CART` attachment layer (behind the body)
/// spawned as a child the moment the bit sets and despawned when it clears.
///
/// Consumes **both** channels that carry `effect_state`, so every path is
/// covered uniformly: `UnitStateChanged` for live mount/unmount toggles, and
/// `UnitEntered` for units already mounted when they enter view (there is no
/// follow-up `UnitStateChanged` in that case). Ordered `after` entity spawning
/// so the `UnitEntered` unit is already registered when we resolve it.
///
/// The presence of a `CartLayer` child is the parent's mount state, so a repeat
/// event that still has the bit set does not respawn it.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(
        in_set = SpriteRenderingSystems::AnimationEvents,
        after = EntityLifecycleSystems::Spawning
    )
)]
#[allow(clippy::too_many_arguments)]
pub fn apply_cart_mount(
    mut state_changes: MessageReader<UnitStateChanged>,
    mut entered: MessageReader<UnitEntered>,
    registry: Res<EntityRegistry>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cart_layers: CartOwnerQuery,
) {
    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            debug!(
                "cart: UnitStateChanged for unresolved unit {} (effect_state={:#x}) dropped",
                event.unit_id, event.effect_state
            );
            continue;
        };
        apply_cart_state(
            entity,
            event.effect_state,
            &cart_layers,
            &mut commands,
            &asset_server,
            &shared_quad,
            &mut materials,
        );
    }

    for event in entered.read() {
        let Some(entity) = registry.get_entity(event.gid) else {
            continue;
        };
        apply_cart_state(
            entity,
            event.effect_state,
            &cart_layers,
            &mut commands,
            &asset_server,
            &shared_quad,
            &mut materials,
        );
    }
}

/// Reconciles one unit's cart layer with its `effect_state`: spawn when the bit
/// sets and no cart child exists yet, despawn when it clears and one does.
fn apply_cart_state(
    entity: Entity,
    effect_state: u32,
    cart_layers: &CartOwnerQuery,
    commands: &mut Commands,
    asset_server: &AssetServer,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let mounted = effect_state & CART_MASK != 0;
    // NOTE: the spawn/despawn are deferred commands, so two cart-mount events
    // for the same unit in a single frame would both see no existing children
    // and double-spawn. aesir emits discrete per-change state broadcasts, so
    // this cannot happen in practice; add a per-run dedup set if it ever does.
    let existing: Vec<Entity> = cart_layers
        .iter()
        .filter(|(_, child_of)| child_of.parent() == entity)
        .map(|(child, _)| child)
        .collect();

    match (mounted, existing.is_empty()) {
        (true, true) => spawn_cart_layer(commands, entity, asset_server, shared_quad, materials),
        (false, false) => {
            for child in existing {
                commands.entity(child).despawn();
            }
        }
        _ => {}
    }
}

/// Number of ACT layers per cart frame. Every action/frame of 손수레.act
/// composes exactly two: the wheel piece then the cart body on top.
const CART_ACT_PARTS: usize = 2;

/// Spawns the cart children now (one quad per ACT part, so the mount is
/// observable immediately) with empty animations; `finalize_cart_layer` fills
/// them once the SPR/ACT load. The children start hidden to avoid blank quads
/// flashing before their first texture.
fn spawn_cart_layer(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let z_offset = layer_order(LAYER_CART) as f32 * Z_OFFSET_PER_LAYER;

    for part in 0..CART_ACT_PARTS {
        let material = materials.add(StandardMaterial {
            base_color_texture: None,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        // Later ACT layers draw on top, so give each part a tiny z step.
        let part_z = z_offset + part as f32 * 0.001;

        commands.spawn((
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(material),
            Billboard,
            RenderLayer::body(Handle::default(), LAYER_CART, Vec::new()),
            CartLayer { part },
            CartAnimationPending {
                spr: asset_server.load(patterns::cart_sprite_path()),
                act: asset_server.load(patterns::cart_action_path()),
            },
            Transform::from_translation(Vec3::new(0.0, SPRITE_BASE_Y_OFFSET, part_z)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            ChildOf(parent),
        ));
    }
}

/// Fills the cart child's animation handle + textures once its SPR/ACT finish
/// loading, polling the handles carried by [`CartAnimationPending`]. The cart
/// deliberately bypasses `PendingAnimations`: that queue has whole-queue
/// drainers (`finalize_render_layers`, `finalize_equipment_layers`) that
/// consumed the cart's completion, leaving the layer permanently hidden.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AssetPopulation)
)]
pub fn finalize_cart_layer(
    mut commands: Commands,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<Persistent<Settings>>,
    mut cart_layers: Query<(Entity, &CartAnimationPending, &mut RenderLayer), With<CartLayer>>,
) {
    for (entity, pending, mut render_layer) in &mut cart_layers {
        let (Some(sprite), Some(action)) = (sprites.get(&pending.spr), actions.get(&pending.act))
        else {
            continue;
        };

        let animation = RoAnimationProcessor::process(
            &sprite.sprite,
            &action.action,
            LAYER_CART,
            &mut images,
            settings.graphics.upscaling,
        );

        render_layer.textures = animation.textures.clone();
        render_layer.animation = animations.add(animation);
        commands.entity(entity).remove::<CartAnimationPending>();
        debug!("cart: animation finalized for {entity:?}");
    }
}

type CartLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static CartLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
>;

/// How far behind the character the cart trails, in world units on the ground
/// plane (one GAT cell = 5.0). The cart ACT carries no anchor positions, so the
/// pull-behind placement is ours; the offset is opposite the facing direction,
/// which also gives correct depth order for free (behind the body when facing
/// the camera, in front when facing away).
const CART_BACK_DISTANCE: f32 = 7.0;

/// Ground-plane (x, z) offset pointing behind a unit facing `direction`.
/// World axes: South = -Z, East = +X. Diagonals keep full per-axis magnitude
/// (a grid-diagonal step, not a normalized vector) so the cart lands on the
/// visually adjacent back cell.
fn cart_behind_offset(direction: Direction) -> Vec2 {
    match direction {
        Direction::South => Vec2::new(0.0, 1.0),
        Direction::SouthWest => Vec2::new(1.0, 1.0),
        Direction::West => Vec2::new(1.0, 0.0),
        Direction::NorthWest => Vec2::new(1.0, -1.0),
        Direction::North => Vec2::new(0.0, -1.0),
        Direction::NorthEast => Vec2::new(-1.0, -1.0),
        Direction::East => Vec2::new(-1.0, 0.0),
        Direction::SouthEast => Vec2::new(-1.0, 1.0),
    }
}

/// Drives each cart quad per frame off its parent's `PlayerSprite`: the cart
/// ACT is direction-only (8 actions), so the parent's facing picks the action
/// and the wheel frames animate on the cart's own delay while walking.
/// Positions like the body (raw layer position, world up is -Y), then pulls
/// the quad behind the character on the ground plane.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_cart_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<&PlayerSprite>,
    mut cart_query: CartLayerQuery,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for (layer, cart, child_of, material_handle, mut transform, mut visibility) in
        cart_query.iter_mut()
    {
        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // The cart ACT is direction-only (one action per facing), unlike the
        // body's action-type x direction grid, so index it by facing directly.
        // The frames roll the wheels: animate them on the cart's own delay
        // while walking, hold the first frame otherwise.
        let action_index = ro_sprite.direction as usize;
        let Some(action_data) = animation.actions.get(action_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if action_data.frames.is_empty() {
            *visibility = Visibility::Hidden;
            continue;
        }

        let frame_index = if ro_sprite.action_type == ActionType::Walk {
            let delay = action_data.delay_ms.max(1.0);
            (game_time_ms as f32 / delay) as usize % action_data.frames.len()
        } else {
            0
        };

        let Some(frame) = action_data.frames.get(frame_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(part) = frame.parts.get(cart.part) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if let Some(texture) = animation.textures.get(part.texture_index) {
            if let Some(mut material) = materials.get_mut(&material_handle.0) {
                material.base_color_texture = Some(texture.clone());
            }
        }

        let mut scale_x = part.scale.x * part.texture_size.x * SPRITE_WORLD_SCALE;
        let scale_y = part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE;

        if part.mirror {
            scale_x = -scale_x;
        }

        let behind = cart_behind_offset(ro_sprite.direction) * CART_BACK_DISTANCE;
        let part_z = layer_order(LAYER_CART) as f32 * Z_OFFSET_PER_LAYER + cart.part as f32 * 0.001;

        transform.scale = Vec3::new(scale_x, scale_y, 1.0);
        transform.translation.x = part.position.x * SPRITE_WORLD_SCALE + behind.x;
        transform.translation.y = -part.position.y * SPRITE_WORLD_SCALE;
        transform.translation.z = part_z + behind.y;

        *visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::billboard::create_sprite_quad_mesh;
    use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};

    const OPTION_CART1: u32 = 0x08;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<Mesh>()
            .init_asset::<RoSpriteAsset>()
            .init_asset::<RoActAsset>()
            .add_message::<UnitStateChanged>()
            .add_message::<UnitEntered>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, apply_cart_mount);

        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(create_sprite_quad_mesh());
        app.insert_resource(SharedSpriteQuad { mesh });
        app
    }

    fn register(app: &mut App, gid: u32, entity: Entity) {
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, entity);
    }

    fn emit(app: &mut App, effect_state: u32) {
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

    fn emit_entered(app: &mut App, gid: u32, effect_state: u32) {
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
                gid,
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
                sex: 0,
                is_boss: false,
                name: String::new(),
                moving: false,
                dst_x: 0,
                dst_y: 0,
                move_start_time: 0,
            });
        app.update();
    }

    fn cart_children(app: &mut App, parent: Entity) -> Vec<Entity> {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &ChildOf), With<CartLayer>>();
        query
            .iter(app.world())
            .filter(|(_, child_of)| child_of.parent() == parent)
            .map(|(entity, _)| entity)
            .collect()
    }

    #[test]
    fn cart_bit_spawns_one_quad_per_part() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);

        assert_eq!(cart_children(&mut app, unit).len(), CART_ACT_PARTS);
    }

    #[test]
    fn repeat_bit_does_not_respawn_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);
        emit(&mut app, OPTION_CART1 | 0x02);

        assert_eq!(cart_children(&mut app, unit).len(), CART_ACT_PARTS);
    }

    #[test]
    fn clearing_bit_despawns_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);
        assert_eq!(cart_children(&mut app, unit).len(), CART_ACT_PARTS);

        emit(&mut app, 0);
        assert!(cart_children(&mut app, unit).is_empty());
    }

    // Building the schedule panics with B0001 if `sync_cart_layer`'s `&mut Transform`
    // aliases another query; it shares archetypes with the weapon/headgear layers.
    #[test]
    fn sync_cart_layer_has_no_query_conflict() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<RoAnimationAsset>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Update, sync_cart_layer);
        app.update();
    }

    #[test]
    fn already_mounted_unit_entered_spawns_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, 7, OPTION_CART1);

        assert_eq!(cart_children(&mut app, unit).len(), CART_ACT_PARTS);
    }

    #[test]
    fn unit_entered_then_redundant_state_change_does_not_respawn() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, 7, OPTION_CART1);
        emit(&mut app, OPTION_CART1);

        assert_eq!(cart_children(&mut app, unit).len(), CART_ACT_PARTS);
    }

    #[test]
    fn no_cart_bit_spawns_nothing() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, 0x02);

        assert!(cart_children(&mut app, unit).is_empty());
    }
}
