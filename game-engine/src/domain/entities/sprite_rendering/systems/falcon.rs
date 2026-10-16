use crate::domain::entities::character::systems::OPTION_FALCON;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::components::FalconLayer;
use crate::domain::system_sets::{EntityLifecycleSystems, SpriteRenderingSystems};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
pub use lifthrasir_runtime::actors::sprites::systems::falcon::*;
use net_contract::events::{SkillEffectShown, UnitEntered, UnitStateChanged};
use std::collections::HashSet;
const HT_BLITZBEAT: u32 = 129;
#[cfg(test)]
const FALCON_REST_OFFSET: Vec3 = Vec3::new(
    crate::domain::sprite::tags::PIXELS_PER_METRE * 2.0,
    crate::domain::sprite::tags::SPRITE_BASE_Y_OFFSET
        + crate::domain::sprite::tags::PIXELS_PER_METRE * 2.0,
    0.0,
);

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
        let lag = follows
            .iter()
            .find(|(child_of, _)| child_of.parent() == owner)
            .map(|(_, follow)| follow.lag)
            .unwrap_or(Vec3::ZERO);
        flight.launch(target, lag);
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
            spawn_falcon_layer(commands, entity, asset_server);
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
    use net_contract::events::{
        SkillDamageReceived, SkillEffectShown, UnitEntered, UnitStateChanged,
    };

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
        let mut follow = FalconFollow::default();
        follow.lag = lag;
        app.world_mut()
            .spawn((ChildOf(owner), FalconLayer { part: 0 }, follow));
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
}
