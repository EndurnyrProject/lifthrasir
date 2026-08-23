use super::VfxSystems;
use crate::core::state::GameState;
use crate::domain::world::components::CurrentMapAltitude;
use crate::domain::world::map::MapData;
use crate::domain::world::navigation::{ActiveRoute, NavigationSystems, densify};
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

const QUAD_SIZE: f32 = 4.2;
const PATH_LIFT: f32 = -0.1;

#[derive(Component)]
struct NavigationPathQuad;

#[derive(Resource)]
struct NavigationPathAssets {
    quad: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl FromWorld for NavigationPathAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world.resource_mut::<Assets<Mesh>>().add(
            Mesh::from(Rectangle::new(QUAD_SIZE, QUAD_SIZE).mesh())
                .rotated_by(Quat::from_rotation_x(FRAC_PI_2)),
        );
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgba(0.2, 0.65, 1.0, 0.55),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            });
        Self { quad, material }
    }
}

#[derive(Resource, Default)]
struct NavigationPathKey(Option<(String, u64)>);

#[allow(clippy::too_many_arguments)]
fn update_navigation_path(
    active_route: Res<ActiveRoute>,
    maps: Query<&MapData>,
    assets: Res<NavigationPathAssets>,
    map_altitude: Option<Res<CurrentMapAltitude>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    existing: Query<Entity, With<NavigationPathQuad>>,
    mut key: ResMut<NavigationPathKey>,
    mut commands: Commands,
) {
    let Some(route) = active_route.0.as_ref() else {
        despawn_navigation_path(&existing, &mut commands);
        key.0 = None;
        return;
    };
    let Ok(map) = maps.single() else {
        despawn_navigation_path(&existing, &mut commands);
        key.0 = None;
        return;
    };
    let Some(leg) = route.current_leg() else {
        despawn_navigation_path(&existing, &mut commands);
        key.0 = None;
        return;
    };
    if leg.map != map.name {
        despawn_navigation_path(&existing, &mut commands);
        key.0 = None;
        return;
    }

    let desired = (map.name.clone(), route.generation);
    if key.0.as_ref() == Some(&desired) {
        return;
    }

    despawn_navigation_path(&existing, &mut commands);

    let Some(altitude) = map_altitude
        .as_ref()
        .and_then(|map_altitude| altitude_assets.get(&map_altitude.0))
    else {
        key.0 = None;
        return;
    };

    for (cx, cy) in densify(&leg.cells) {
        let world = spawn_coords_to_world_position(cx, cy);
        let Some(height) = altitude.altitude.get_terrain_height_at_position(world) else {
            continue;
        };
        commands.spawn((
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_xyz(world.x, height + PATH_LIFT, world.z),
            NotShadowCaster,
            NavigationPathQuad,
        ));
    }

    key.0 = Some(desired);
}

fn despawn_navigation_path(
    existing: &Query<Entity, With<NavigationPathQuad>>,
    commands: &mut Commands,
) {
    for entity in existing {
        commands.entity(entity).despawn();
    }
}

fn reset_navigation_path(
    existing: Query<Entity, With<NavigationPathQuad>>,
    mut key: ResMut<NavigationPathKey>,
    mut commands: Commands,
) {
    despawn_navigation_path(&existing, &mut commands);
    key.0 = None;
}

pub struct NavigationPathPlugin;

impl Plugin for NavigationPathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NavigationPathAssets>()
            .init_resource::<NavigationPathKey>()
            .add_systems(
                Update,
                update_navigation_path
                    .in_set(VfxSystems)
                    .in_set(NavigationSystems::ViewSync)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset_navigation_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::world::map::MapData;
    use crate::domain::world::navigation::{ActiveRoute, Route};
    use crate::infrastructure::assets::loaders::RoAltitudeAsset;
    use crate::infrastructure::ro_formats::{GatCell, GatCellType, RoAltitude};
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use net_contract::dto::{RouteDestination, RouteLeg};

    fn navigation_path_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAltitudeAsset>()
            .init_resource::<ActiveRoute>()
            .init_resource::<NavigationPathAssets>()
            .init_resource::<NavigationPathKey>()
            .add_systems(Update, update_navigation_path);
        app
    }

    fn navigation_path_plugin_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(StatesPlugin)
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<RoAltitudeAsset>()
            .init_state::<GameState>()
            .init_resource::<ActiveRoute>()
            .add_plugins(NavigationPathPlugin);
        app
    }

    fn quad_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&NavigationPathQuad>()
            .iter(app.world())
            .count()
    }

    fn route(map: &str, cells: Vec<(u16, u16)>) -> Route {
        Route {
            generation: 1,
            legs: vec![RouteLeg {
                map: map.into(),
                cells,
                exit_portal: None,
                next_map: None,
            }],
            current: 0,
            destination: RouteDestination {
                map: map.into(),
                x: 0,
                y: 0,
            },
            hide_window: false,
        }
    }

    fn flat_altitude(size: u32) -> RoAltitudeAsset {
        RoAltitudeAsset {
            altitude: RoAltitude {
                version: "1.2".into(),
                width: size,
                height: size,
                cells: (0..size * size)
                    .map(|_| GatCell {
                        height: [0.0; 4],
                        cell_type: GatCellType::from(0u32),
                    })
                    .collect(),
            },
        }
    }

    fn publish_flat_altitude(app: &mut App, size: u32) {
        let altitude = app
            .world_mut()
            .resource_mut::<Assets<RoAltitudeAsset>>()
            .add(flat_altitude(size));
        app.insert_resource(CurrentMapAltitude(altitude));
    }

    fn start_path(app: &mut App, map: &str, cells: Vec<(u16, u16)>, altitude_size: u32) {
        app.world_mut().spawn(MapData {
            name: map.into(),
            width: altitude_size,
            height: altitude_size,
        });
        publish_flat_altitude(app, altitude_size);
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route(map, cells));
    }

    fn quad_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<NavigationPathQuad>>()
            .iter(app.world())
            .collect()
    }

    fn set_loaded_map(app: &mut App, name: &str) {
        let world = app.world_mut();
        world
            .query_filtered::<&mut MapData, With<MapData>>()
            .single_mut(world)
            .expect("loaded map")
            .name = name.into();
    }

    #[test]
    fn wrong_map_route_despawns_quads_and_resets_the_key() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1), (2, 1)], 20);
        app.update();
        assert_eq!(quad_count(&mut app), 2);
        assert!(app.world().resource::<NavigationPathKey>().0.is_some());

        set_loaded_map(&mut app, "geffen");
        app.update();

        assert_eq!(quad_count(&mut app), 0);
        assert!(app.world().resource::<NavigationPathKey>().0.is_none());
    }

    #[test]
    fn clearing_the_route_despawns_quads_and_resets_the_key() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1), (2, 1)], 20);
        app.update();
        assert_eq!(quad_count(&mut app), 2);

        app.world_mut().resource_mut::<ActiveRoute>().0 = None;
        app.update();

        assert_eq!(quad_count(&mut app), 0);
        assert!(app.world().resource::<NavigationPathKey>().0.is_none());
    }

    #[test]
    fn unresolved_altitude_leaves_the_key_unset_and_retries_when_loaded() {
        let mut app = navigation_path_app();
        app.world_mut().spawn(MapData {
            name: "prontera".into(),
            width: 20,
            height: 20,
        });
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route("prontera", vec![(1, 1)]));

        app.update();

        assert_eq!(quad_count(&mut app), 0);
        assert!(app.world().resource::<NavigationPathKey>().0.is_none());

        publish_flat_altitude(&mut app, 20);
        app.update();

        assert_eq!(quad_count(&mut app), 1);
        assert_eq!(
            app.world().resource::<NavigationPathKey>().0,
            Some(("prontera".into(), 1))
        );
    }

    #[test]
    fn off_map_cells_are_skipped_while_in_bounds_neighbours_spawn() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(0, 0), (1, 0), (2, 0)], 2);

        app.update();

        assert_eq!(quad_count(&mut app), 2);
    }

    #[test]
    fn unchanged_route_keeps_its_existing_quads() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1), (2, 1)], 20);
        app.update();
        let original = quad_entities(&mut app);

        app.update();

        assert_eq!(quad_entities(&mut app), original);
    }

    #[test]
    fn changed_leg_rebuilds_the_path_on_the_same_map() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1)], 20);
        let mut route = route("prontera", vec![(1, 1)]);
        route.legs.push(RouteLeg {
            map: "prontera".into(),
            cells: vec![(3, 1)],
            exit_portal: None,
            next_map: None,
        });
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route.clone());
        app.update();
        let original = quad_entities(&mut app);

        route.current = 1;
        route.generation = 2;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route);
        app.update();

        assert_eq!(quad_count(&mut app), 1);
        assert_ne!(quad_entities(&mut app), original);
        assert_eq!(
            app.world().resource::<NavigationPathKey>().0,
            Some(("prontera".into(), 2))
        );
    }

    #[test]
    fn changed_destination_rebuilds_the_path_on_the_same_map() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1), (2, 1)], 20);
        app.update();
        let original = quad_entities(&mut app);

        let mut replacement = route("prontera", vec![(3, 1), (4, 1)]);
        replacement.generation = 2;
        replacement.destination.x = 4;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(replacement);
        app.update();

        assert_ne!(quad_entities(&mut app), original);
    }

    #[test]
    fn reroute_rebuilds_the_path_for_the_same_destination_and_leg() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1), (2, 1)], 20);
        app.update();
        let original = quad_entities(&mut app);

        let mut replacement = route("prontera", vec![(3, 1), (4, 1)]);
        replacement.generation = 2;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(replacement);
        app.update();

        assert_ne!(quad_entities(&mut app), original);
    }

    #[test]
    fn changed_map_rebuilds_the_path_with_the_same_leg_index() {
        let mut app = navigation_path_app();
        start_path(&mut app, "prontera", vec![(1, 1)], 20);
        app.update();
        let original = quad_entities(&mut app);

        set_loaded_map(&mut app, "geffen");
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route("geffen", vec![(3, 1)]));
        app.update();

        assert_eq!(quad_count(&mut app), 1);
        assert_ne!(quad_entities(&mut app), original);
        assert_eq!(
            app.world().resource::<NavigationPathKey>().0,
            Some(("geffen".into(), 1))
        );
    }

    #[test]
    fn ingame_exit_resets_the_path_and_reentry_rebuilds_it() {
        let mut app = navigation_path_plugin_app();
        start_path(&mut app, "prontera", vec![(1, 1)], 20);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(quad_count(&mut app), 1);
        assert!(app.world().resource::<NavigationPathKey>().0.is_some());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();

        assert_eq!(quad_count(&mut app), 0);
        assert!(app.world().resource::<NavigationPathKey>().0.is_none());

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        assert_eq!(quad_count(&mut app), 1);
        assert!(app.world().resource::<NavigationPathKey>().0.is_some());
    }
}
