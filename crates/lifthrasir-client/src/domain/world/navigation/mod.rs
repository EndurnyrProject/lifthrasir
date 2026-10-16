use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::dto::{RouteDestination, RouteLeg};

mod systems;

/// Ordered route lifecycle stages. Systems reading [`ActiveRoute`] belong to [`Self::ViewSync`].
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    chain
)]
pub enum NavigationSystems {
    SessionReset,
    Apply,
    ViewSync,
}

#[derive(Resource, Debug, Default)]
#[auto_init_resource(plugin = crate::domain::world::WorldDomainPlugin)]
pub struct ActiveRoute(pub Option<Route>);

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub generation: u64,
    pub legs: Vec<RouteLeg>,
    pub current: u32,
    pub destination: RouteDestination,
    pub hide_window: bool,
}

impl Route {
    /// The only leg carrying cells. `None` if `current` is out of range.
    pub fn current_leg(&self) -> Option<&RouteLeg> {
        self.legs.get(self.current as usize)
    }

    /// `(current + 1, total)`, 1-based for display.
    pub fn leg_progress(&self) -> (u32, u32) {
        (self.current + 1, self.legs.len() as u32)
    }

    /// Cells to draw, but only when the leg belongs to `map_name`.
    pub fn cells_for_map(&self, map_name: &str) -> &[(u16, u16)] {
        self.current_leg()
            .filter(|leg| leg.map == map_name)
            .map_or(&[], |leg| leg.cells.as_slice())
    }
}

/// Expand simplified waypoints into every intermediate cell, including each endpoint once.
/// Chebyshev stepping matches the server's eight-way movement.
pub fn densify(waypoints: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let Some(&first) = waypoints.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for pair in waypoints.windows(2) {
        let (ax, ay) = (i64::from(pair[0].0), i64::from(pair[0].1));
        let (dx, dy) = (i64::from(pair[1].0) - ax, i64::from(pair[1].1) - ay);
        let steps = dx.abs().max(dy.abs());
        for step in 1..=steps {
            let interpolate = |origin, delta| {
                let numerator = delta * step;
                let rounded = if numerator >= 0 {
                    numerator + steps / 2
                } else {
                    numerator - steps / 2
                };
                (origin + rounded / steps) as u16
            };
            out.push((interpolate(ax, dx), interpolate(ay, dy)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn route(current: u32) -> Route {
        Route {
            generation: 0,
            legs: vec![RouteLeg {
                map: "prontera".into(),
                cells: vec![(10, 20)],
                exit_portal: None,
                next_map: None,
            }],
            current,
            destination: RouteDestination {
                map: "prontera".into(),
                x: 10,
                y: 20,
            },
            hide_window: false,
        }
    }

    #[test]
    fn current_leg_returns_the_indexed_leg() {
        assert_eq!(route(0).current_leg().unwrap().map, "prontera");
    }

    #[test]
    fn current_leg_returns_none_when_the_index_is_out_of_range() {
        assert!(route(1).current_leg().is_none());
    }

    #[test]
    fn cells_for_map_returns_the_current_legs_cells() {
        assert_eq!(route(0).cells_for_map("prontera"), [(10, 20)]);
    }

    #[test]
    fn cells_for_map_is_empty_when_the_current_leg_belongs_to_another_map() {
        assert!(route(0).cells_for_map("geffen").is_empty());
    }

    #[test]
    fn leg_progress_is_one_based_and_includes_the_total() {
        let mut route = route(1);
        route.legs.push(RouteLeg {
            map: "geffen".into(),
            cells: vec![],
            exit_portal: None,
            next_map: None,
        });

        assert_eq!(route.leg_progress(), (2, 2));
    }

    #[test]
    fn densify_empty_waypoints_is_empty() {
        assert!(densify(&[]).is_empty());
    }

    #[test]
    fn densify_single_waypoint_returns_it_once() {
        assert_eq!(densify(&[(7, 9)]), [(7, 9)]);
    }

    #[test]
    fn densify_horizontal_span_fills_every_cell() {
        assert_eq!(densify(&[(2, 5), (5, 5)]), [(2, 5), (3, 5), (4, 5), (5, 5)]);
    }

    #[test]
    fn densify_diagonal_span_advances_one_cell_per_step() {
        assert_eq!(densify(&[(2, 5), (5, 8)]), [(2, 5), (3, 6), (4, 7), (5, 8)]);
    }

    #[test]
    fn densify_supports_the_full_u16_coordinate_span() {
        let cells = densify(&[(0, 0), (u16::MAX, u16::MAX)]);

        assert_eq!(cells.len(), usize::from(u16::MAX) + 1);
        assert_eq!(cells.first(), Some(&(0, 0)));
        assert_eq!(cells.last(), Some(&(u16::MAX, u16::MAX)));
    }

    #[test]
    fn densify_negative_unequal_slope_reaches_the_endpoint_without_repeats() {
        let cells = densify(&[(60_000, 10), (5, 40_000)]);
        let unique: HashSet<_> = cells.iter().copied().collect();

        assert_eq!(cells.last(), Some(&(5, 40_000)));
        assert_eq!(unique.len(), cells.len());
    }

    #[test]
    fn densify_duplicate_waypoints_do_not_duplicate_segment_seams() {
        assert_eq!(
            densify(&[(1, 1), (3, 3), (3, 3), (5, 3)]),
            [(1, 1), (2, 2), (3, 3), (4, 3), (5, 3)]
        );
    }
}
