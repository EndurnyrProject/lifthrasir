use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{UnitEntered, UnitStateChanged};

use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::systems::CART_MASK;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::{CartLayer, PlayerSprite, RenderLayer};
use crate::domain::sprite::tags::{
    layer_order, LAYER_CART, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER,
};
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

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
    mut pending_animations: ResMut<PendingAnimations>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cart_layers: CartOwnerQuery,
) {
    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            continue;
        };
        apply_cart_state(
            entity,
            event.effect_state,
            &cart_layers,
            &mut commands,
            &asset_server,
            &mut pending_animations,
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
            &mut pending_animations,
            &shared_quad,
            &mut materials,
        );
    }
}

/// Reconciles one unit's cart layer with its `effect_state`: spawn when the bit
/// sets and no cart child exists yet, despawn when it clears and one does.
#[allow(clippy::too_many_arguments)]
fn apply_cart_state(
    entity: Entity,
    effect_state: u32,
    cart_layers: &CartOwnerQuery,
    commands: &mut Commands,
    asset_server: &AssetServer,
    pending_animations: &mut PendingAnimations,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let mounted = effect_state & CART_MASK != 0;
    // NOTE: the spawn/despawn are deferred commands, so two cart-mount events
    // for the same unit in a single frame would both see `existing == None`
    // and double-spawn. aesir emits discrete per-change state broadcasts, so
    // this cannot happen in practice; add a per-run dedup set if it ever does.
    let existing = cart_layers
        .iter()
        .find(|(_, child_of)| child_of.parent() == entity)
        .map(|(child, _)| child);

    match (mounted, existing) {
        (true, None) => spawn_cart_layer(
            commands,
            entity,
            asset_server,
            pending_animations,
            shared_quad,
            materials,
        ),
        (false, Some(child)) => {
            commands.entity(child).despawn();
        }
        _ => {}
    }
}

/// Spawns the cart child now (so the mount is observable immediately) with an
/// empty animation; `finalize_cart_layer` fills it once the SPR/ACT load. The
/// child starts hidden to avoid a blank quad flashing before its first texture.
fn spawn_cart_layer(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    pending_animations: &mut PendingAnimations,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let z_offset = layer_order(LAYER_CART) as f32 * Z_OFFSET_PER_LAYER;

    let material = materials.add(StandardMaterial {
        base_color_texture: None,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    let child = commands
        .spawn((
            Mesh3d(shared_quad.mesh.clone()),
            MeshMaterial3d(material),
            Billboard,
            RenderLayer::body(Handle::default(), LAYER_CART, Vec::new()),
            CartLayer,
            Transform::from_translation(Vec3::new(0.0, SPRITE_BASE_Y_OFFSET, z_offset)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            ChildOf(parent),
        ))
        .id();

    let spr = asset_server.load(patterns::cart_sprite_path());
    let act = asset_server.load(patterns::cart_action_path());
    pending_animations.request(spr, act, LAYER_CART, Some(child));
}

/// Fills the cart child's animation handle + textures once the SPR/ACT finish
/// loading. Uses the selective drain so it never steals body/head/equipment
/// completions from their own finalizers.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(
        in_set = SpriteRenderingSystems::AnimationEvents,
        before = crate::domain::entities::sprite_rendering::systems::events::finalize_equipment_layers
    )
)]
pub fn finalize_cart_layer(
    mut pending_animations: ResMut<PendingAnimations>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut cart_layers: Query<&mut RenderLayer, With<CartLayer>>,
) {
    for (pending, handle) in pending_animations.take_completed_for_layer(LAYER_CART) {
        let Some(child) = pending.callback_entity else {
            continue;
        };

        let Ok(mut render_layer) = cart_layers.get_mut(child) else {
            continue;
        };

        let Some(animation) = animations.get(&handle) else {
            continue;
        };

        render_layer.textures = animation.textures.clone();
        render_layer.animation = handle;
    }
}

type CartLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    With<CartLayer>,
>;

/// Drives the cart per frame off its parent's `PlayerSprite`, exactly as the
/// body layer drives itself: the cart ACT is authored for the same
/// action/direction layout, so the parent's frame index selects the matching
/// cart pose. Positions like the body (raw layer position, world up is -Y),
/// leaving the child's initial z-offset intact so it stays behind the body.
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

    for (layer, child_of, material_handle, mut transform, mut visibility) in cart_query.iter_mut() {
        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        let Some(frame) = ro_sprite.get_frame(animation, game_time_ms) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(part) = frame.parts.first() else {
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

        transform.scale = Vec3::new(scale_x, scale_y, 1.0);
        transform.translation.x = part.position.x * SPRITE_WORLD_SCALE;
        transform.translation.y = -part.position.y * SPRITE_WORLD_SCALE;

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
            .init_resource::<PendingAnimations>()
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
    fn cart_bit_spawns_exactly_one_cart_layer() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);

        assert_eq!(cart_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn repeat_bit_does_not_respawn_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);
        emit(&mut app, OPTION_CART1 | 0x02);

        assert_eq!(cart_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn clearing_bit_despawns_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit(&mut app, OPTION_CART1);
        assert_eq!(cart_children(&mut app, unit).len(), 1);

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

        assert_eq!(cart_children(&mut app, unit).len(), 1);
    }

    #[test]
    fn unit_entered_then_redundant_state_change_does_not_respawn() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        emit_entered(&mut app, 7, OPTION_CART1);
        emit(&mut app, OPTION_CART1);

        assert_eq!(cart_children(&mut app, unit).len(), 1);
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
