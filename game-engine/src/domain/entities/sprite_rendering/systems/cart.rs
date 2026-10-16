use crate::domain::entities::billboard::SharedSpriteQuad;
use crate::domain::entities::character::systems::CART_MASK;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::CartLayer;
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
#[cfg(test)]
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
pub use lifthrasir_runtime::actors::sprites::systems::cart::*;
use net_contract::events::{UnitEntered, UnitStateChanged};
use std::collections::HashSet;
#[cfg(test)]
const CART_ACT_PARTS: usize = 2;

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
    plugin = crate::domain::entities::sprite_rendering::plugin::SpriteRenderingDomainPlugin,
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
    mut handled: Local<HashSet<Entity>>,
) {
    // Per-run only: cleared every update so a later remount is never skipped.
    // Where both readers carry the same owner in one run the first message
    // wins, including when they disagree. A frame that both sets and clears
    // the bit has no correct answer, so this is a deliberate tie-break rather
    // than an ordering bug to "fix".
    handled.clear();

    for event in state_changes.read() {
        let Some(entity) = registry.get_entity(event.unit_id) else {
            debug!(
                "cart: UnitStateChanged for unresolved unit {} (effect_state={:#x}) dropped",
                event.unit_id, event.effect_state
            );
            continue;
        };
        if !handled.insert(entity) {
            continue;
        }
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
        if !handled.insert(entity) {
            continue;
        }
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
    fn state_change_and_unit_entered_in_same_frame_spawn_one_cart() {
        let mut app = app();
        let unit = app.world_mut().spawn_empty().id();
        register(&mut app, 7, unit);

        app.world_mut()
            .resource_mut::<Messages<UnitStateChanged>>()
            .write(UnitStateChanged {
                unit_id: 7,
                body_state: 0,
                health_state: 0,
                effect_state: OPTION_CART1,
                virtue: 0,
            });
        app.world_mut()
            .resource_mut::<Messages<UnitEntered>>()
            .write(UnitEntered {
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
                effect_state: OPTION_CART1,
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
            });
        app.update();

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
