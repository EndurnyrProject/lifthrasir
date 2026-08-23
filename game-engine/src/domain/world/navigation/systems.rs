use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::{NavigationEnded, NavigationFailed, RouteUpdated};
use net_contract::state::ZoneSessionGeneration;

use crate::core::GameState;

use super::{ActiveRoute, NavigationSystems, Route};

#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::domain::world::WorldDomainPlugin)]
pub(super) struct RouteSessionGate(ZoneSessionGeneration);

#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::domain::world::WorldDomainPlugin)]
pub(super) struct RouteGeneration(u64);

#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    config(
        in_set = NavigationSystems::Apply,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn apply_route_lifecycle(
    mut updates: MessageReader<RouteUpdated>,
    mut ended: MessageReader<NavigationEnded>,
    mut failed: MessageReader<NavigationFailed>,
    mut active_route: ResMut<ActiveRoute>,
    mut route_generation: ResMut<RouteGeneration>,
) {
    for update in updates.read() {
        route_generation.0 = route_generation
            .0
            .checked_add(1)
            .expect("route generation exhausted");
        active_route.0 = Some(Route {
            generation: route_generation.0,
            legs: update.legs.clone(),
            current: update.current,
            destination: update.destination.clone(),
            hide_window: update.hide_window,
        });
    }

    let ended = ended.read().count() != 0;
    let failed = failed.read().count() != 0;
    if ended || failed {
        active_route.0 = None;
    }
}

#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    config(in_set = NavigationSystems::SessionReset)
)]
pub fn clear_route_on_session_change(
    generation: Res<ZoneSessionGeneration>,
    mut gate: ResMut<RouteSessionGate>,
    mut active_route: ResMut<ActiveRoute>,
    mut route_generation: ResMut<RouteGeneration>,
) {
    if gate.0 == *generation {
        return;
    }
    gate.0 = *generation;
    active_route.0 = None;
    route_generation.0 = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use net_contract::dto::{NavigationEnd, NavigationFailure, RouteDestination, RouteLeg};
    use net_contract::state::ZoneSession;

    #[derive(Resource, Default)]
    struct ObservedRoute(Option<String>);

    #[derive(Resource, Default)]
    struct LifecyclePairWritten(bool);

    fn write_lifecycle_pair_once(
        mut updates: MessageWriter<RouteUpdated>,
        mut ended: MessageWriter<NavigationEnded>,
        mut written: ResMut<LifecyclePairWritten>,
    ) {
        if written.0 {
            return;
        }
        updates.write(updated("prontera", 10, false));
        ended.write(NavigationEnded {
            reason: NavigationEnd::Arrived,
        });
        written.0 = true;
    }

    fn observe_route(active_route: Res<ActiveRoute>, mut observed: ResMut<ObservedRoute>) {
        observed.0 = active_route
            .0
            .as_ref()
            .and_then(Route::current_leg)
            .map(|leg| leg.map.clone());
    }

    fn updated(map: &str, x: u16, hide_window: bool) -> RouteUpdated {
        RouteUpdated {
            legs: vec![RouteLeg {
                map: map.into(),
                cells: vec![(x, 20)],
                exit_portal: None,
                next_map: None,
            }],
            current: 0,
            destination: RouteDestination {
                map: map.into(),
                x,
                y: 20,
            },
            hide_window,
        }
    }

    #[test]
    fn later_route_update_wholly_replaces_the_previous_snapshot() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteGeneration>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationEnded>()
            .add_message::<NavigationFailed>()
            .add_systems(Update, apply_route_lifecycle);
        app.world_mut()
            .write_message(updated("prontera", 10, false));
        app.world_mut().write_message(updated("geffen", 30, true));

        app.update();

        assert_eq!(
            app.world().resource::<ActiveRoute>().0,
            Some(Route {
                generation: 2,
                legs: vec![RouteLeg {
                    map: "geffen".into(),
                    cells: vec![(30, 20)],
                    exit_portal: None,
                    next_map: None,
                }],
                current: 0,
                destination: RouteDestination {
                    map: "geffen".into(),
                    x: 30,
                    y: 20,
                },
                hide_window: true,
            })
        );
    }

    #[test]
    fn identical_route_updates_receive_distinct_generations() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteGeneration>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationEnded>()
            .add_message::<NavigationFailed>()
            .add_systems(Update, apply_route_lifecycle);
        let update = updated("prontera", 10, false);

        app.world_mut().write_message(update.clone());
        app.update();
        let first = app
            .world()
            .resource::<ActiveRoute>()
            .0
            .as_ref()
            .unwrap()
            .generation;

        app.world_mut().write_message(update);
        app.update();
        let second = app
            .world()
            .resource::<ActiveRoute>()
            .0
            .as_ref()
            .unwrap()
            .generation;

        assert_eq!((first, second), (1, 2));
    }

    #[test]
    fn view_sync_set_observes_route_updates_in_the_same_frame() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteGeneration>()
            .init_resource::<ObservedRoute>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationEnded>()
            .add_message::<NavigationFailed>()
            .configure_sets(
                Update,
                (
                    NavigationSystems::SessionReset,
                    NavigationSystems::Apply,
                    NavigationSystems::ViewSync,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    apply_route_lifecycle.in_set(NavigationSystems::Apply),
                    observe_route.in_set(NavigationSystems::ViewSync),
                ),
            );
        app.world_mut().write_message(updated("geffen", 30, true));

        app.update();

        assert_eq!(
            app.world().resource::<ObservedRoute>().0.as_deref(),
            Some("geffen")
        );
    }

    #[test]
    fn route_update_then_end_in_one_frame_leaves_no_active_route() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteGeneration>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationEnded>()
            .add_message::<NavigationFailed>()
            .add_systems(Update, apply_route_lifecycle);
        app.world_mut()
            .write_message(updated("prontera", 10, false));
        app.world_mut().write_message(NavigationEnded {
            reason: NavigationEnd::Arrived,
        });

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_none());
    }

    #[test]
    fn interleaved_lifecycle_pair_cannot_leave_an_orphaned_route() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteGeneration>()
            .init_resource::<LifecyclePairWritten>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationEnded>()
            .add_message::<NavigationFailed>()
            .configure_sets(
                Update,
                (NavigationSystems::Apply, NavigationSystems::ViewSync).chain(),
            )
            .add_systems(
                Update,
                (
                    apply_route_lifecycle.in_set(NavigationSystems::Apply),
                    write_lifecycle_pair_once
                        .after(NavigationSystems::Apply)
                        .before(NavigationSystems::ViewSync),
                ),
            );

        app.update();
        assert!(app.world().resource::<ActiveRoute>().0.is_none());

        app.update();
        assert!(app.world().resource::<ActiveRoute>().0.is_none());
    }

    #[test]
    fn navigation_ended_clears_the_active_route() {
        let mut app = App::new();
        app.insert_resource(ActiveRoute(Some(Route {
            generation: 0,
            legs: updated("prontera", 10, false).legs,
            current: 0,
            destination: RouteDestination {
                map: "prontera".into(),
                x: 10,
                y: 20,
            },
            hide_window: false,
        })))
        .init_resource::<RouteGeneration>()
        .add_message::<RouteUpdated>()
        .add_message::<NavigationEnded>()
        .add_message::<NavigationFailed>()
        .add_systems(Update, apply_route_lifecycle);
        app.world_mut().write_message(NavigationEnded {
            reason: NavigationEnd::Arrived,
        });

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_none());
    }

    #[test]
    fn navigation_failed_clears_the_route_the_server_already_dropped() {
        let mut app = App::new();
        app.insert_resource(ActiveRoute(Some(Route {
            generation: 0,
            legs: updated("prontera", 10, false).legs,
            current: 0,
            destination: RouteDestination {
                map: "prontera".into(),
                x: 10,
                y: 20,
            },
            hide_window: false,
        })))
        .init_resource::<RouteGeneration>()
        .add_message::<RouteUpdated>()
        .add_message::<NavigationEnded>()
        .add_message::<NavigationFailed>()
        .add_systems(Update, apply_route_lifecycle);
        app.world_mut().write_message(NavigationFailed {
            reason: NavigationFailure::Unreachable,
        });

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_none());
    }

    #[test]
    fn zone_session_generation_change_clears_the_active_route_outside_ingame() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .init_resource::<ActiveRoute>()
            .init_resource::<RouteSessionGate>()
            .init_resource::<RouteGeneration>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_systems(Update, clear_route_on_session_change);
        app.update();
        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Bootstrapping
        );
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(Route {
            generation: 0,
            legs: updated("prontera", 10, false).legs,
            current: 0,
            destination: RouteDestination {
                map: "prontera".into(),
                x: 10,
                y: 20,
            },
            hide_window: false,
        });
        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_none());
    }

    #[test]
    fn rewriting_the_same_generation_does_not_clear_the_active_route() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteSessionGate>()
            .init_resource::<RouteGeneration>()
            .insert_resource(ZoneSessionGeneration(1))
            .add_systems(Update, clear_route_on_session_change);
        app.update();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(Route {
            generation: 0,
            legs: updated("prontera", 10, false).legs,
            current: 0,
            destination: RouteDestination {
                map: "prontera".into(),
                x: 10,
                y: 20,
            },
            hide_window: false,
        });
        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(1);

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_some());
    }

    #[test]
    fn map_change_does_not_clear_the_active_route() {
        let mut app = App::new();
        app.init_resource::<ActiveRoute>()
            .init_resource::<RouteSessionGate>()
            .init_resource::<RouteGeneration>()
            .insert_resource(ZoneSessionGeneration(1))
            .insert_resource(ZoneSession {
                map_name: "prontera".into(),
                ..default()
            })
            .add_systems(Update, clear_route_on_session_change);
        app.update();
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(Route {
            generation: 0,
            legs: updated("prontera", 10, false).legs,
            current: 0,
            destination: RouteDestination {
                map: "geffen".into(),
                x: 30,
                y: 40,
            },
            hide_window: false,
        });
        app.world_mut().resource_mut::<ZoneSession>().map_name = "geffen".into();

        app.update();

        assert!(app.world().resource::<ActiveRoute>().0.is_some());
    }
}
