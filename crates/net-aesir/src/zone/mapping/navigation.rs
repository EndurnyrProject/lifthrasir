use bevy::prelude::warn;
use net_contract::dto::{NavigationEnd, NavigationFailure, RouteDestination, RouteLeg};
use net_contract::events::RouteUpdated;

use crate::proto::aesir::net;

pub fn navigate_to(m: net::NavigateTo) -> Option<RouteUpdated> {
    let Some(destination) = m.destination else {
        warn!("NavigateTo without destination; skipping");
        return None;
    };
    let current = m
        .legs
        .iter()
        .position(|leg| !leg.cells.is_empty())
        .unwrap_or(0) as u32;

    Some(RouteUpdated {
        legs: m
            .legs
            .into_iter()
            .map(|leg| RouteLeg {
                map: leg.map,
                cells: leg
                    .cells
                    .into_iter()
                    .map(|cell| (cell.x as u16, cell.y as u16))
                    .collect(),
                exit_portal: (!leg.exit_portal.is_empty()).then_some(leg.exit_portal),
                next_map: (!leg.next_map.is_empty()).then_some(leg.next_map),
            })
            .collect(),
        current,
        destination: RouteDestination {
            map: destination.map,
            x: destination.x as u16,
            y: destination.y as u16,
        },
        hide_window: m.hide_window,
    })
}

pub fn navigation_failed(
    m: net::NavigationFailed,
) -> Option<net_contract::events::NavigationFailed> {
    let reason = match net::NavigationFailureReason::try_from(m.reason) {
        Ok(net::NavigationFailureReason::Unresolved) => NavigationFailure::Unresolved,
        Ok(net::NavigationFailureReason::Unreachable) => NavigationFailure::Unreachable,
        Ok(net::NavigationFailureReason::AlreadyThere) => NavigationFailure::AlreadyThere,
        Ok(net::NavigationFailureReason::Excluded) => NavigationFailure::Excluded,
        Ok(reason) => {
            warn!(
                "unsupported NavigationFailureReason {}; skipping",
                reason.as_str_name()
            );
            return None;
        }
        Err(_) => {
            warn!("unknown NavigationFailureReason {}; skipping", m.reason);
            return None;
        }
    };

    Some(net_contract::events::NavigationFailed { reason })
}

pub fn navigation_ended(m: net::NavigationEnded) -> Option<net_contract::events::NavigationEnded> {
    let reason = match net::NavigationEndReason::try_from(m.reason) {
        Ok(net::NavigationEndReason::Arrived) => NavigationEnd::Arrived,
        Ok(net::NavigationEndReason::Cancelled) => NavigationEnd::Cancelled,
        Ok(reason) => {
            warn!(
                "unsupported NavigationEndReason {}; skipping",
                reason.as_str_name()
            );
            return None;
        }
        Err(_) => {
            warn!("unknown NavigationEndReason {}; skipping", m.reason);
            return None;
        }
    };

    Some(net_contract::events::NavigationEnded { reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{RouteDestination, RouteLeg};
    use net_contract::events::RouteUpdated;

    use crate::proto::aesir::net;

    #[test]
    fn navigate_to_uses_the_position_of_the_only_leg_with_cells() {
        let route = navigate_to(net::NavigateTo {
            map: "ignored".into(),
            x: 0,
            y: 0,
            flag: 0,
            hide_window: true,
            monster_id: 0,
            legs: vec![
                net::NavigationLeg {
                    index: 9,
                    map: "prontera".into(),
                    cells: Vec::new(),
                    exit_portal: "prontera_to_geffen".into(),
                    next_map: "geffen".into(),
                    arrive: None,
                },
                net::NavigationLeg {
                    index: 4,
                    map: "geffen".into(),
                    cells: vec![net::NavigationCell { x: 30, y: 40 }],
                    exit_portal: "geffen_to_aldebaran".into(),
                    next_map: "aldebaran".into(),
                    arrive: None,
                },
                net::NavigationLeg {
                    index: 7,
                    map: "aldebaran".into(),
                    cells: Vec::new(),
                    exit_portal: String::new(),
                    next_map: String::new(),
                    arrive: Some(net::NavigationCell { x: 50, y: 60 }),
                },
            ],
            destination: Some(net::NavigationCoordinate {
                map: "aldebaran".into(),
                x: 50,
                y: 60,
            }),
        });

        assert_eq!(
            route,
            Some(RouteUpdated {
                legs: vec![
                    RouteLeg {
                        map: "prontera".into(),
                        cells: Vec::new(),
                        exit_portal: Some("prontera_to_geffen".into()),
                        next_map: Some("geffen".into()),
                    },
                    RouteLeg {
                        map: "geffen".into(),
                        cells: vec![(30, 40)],
                        exit_portal: Some("geffen_to_aldebaran".into()),
                        next_map: Some("aldebaran".into()),
                    },
                    RouteLeg {
                        map: "aldebaran".into(),
                        cells: Vec::new(),
                        exit_portal: None,
                        next_map: None,
                    },
                ],
                current: 1,
                destination: RouteDestination {
                    map: "aldebaran".into(),
                    x: 50,
                    y: 60,
                },
                hide_window: true,
            })
        );
    }

    #[test]
    fn navigate_to_returns_none_without_destination() {
        assert_eq!(
            navigate_to(net::NavigateTo {
                map: "ignored".into(),
                x: 0,
                y: 0,
                flag: 0,
                hide_window: false,
                monster_id: 0,
                legs: Vec::new(),
                destination: None,
            }),
            None
        );
    }

    #[test]
    fn navigate_to_defaults_to_first_leg_when_no_leg_has_cells() {
        let route = navigate_to(net::NavigateTo {
            map: "ignored".into(),
            x: 0,
            y: 0,
            flag: 0,
            hide_window: false,
            monster_id: 0,
            legs: vec![
                net::NavigationLeg {
                    index: 4,
                    map: "geffen".into(),
                    cells: Vec::new(),
                    exit_portal: "geffen_to_aldebaran".into(),
                    next_map: "aldebaran".into(),
                    arrive: None,
                },
                net::NavigationLeg {
                    index: 9,
                    map: "aldebaran".into(),
                    cells: Vec::new(),
                    exit_portal: String::new(),
                    next_map: String::new(),
                    arrive: None,
                },
            ],
            destination: Some(net::NavigationCoordinate {
                map: "aldebaran".into(),
                x: 50,
                y: 60,
            }),
        })
        .expect("destination is present");

        assert_eq!(route.current, 0);
        assert!(route.legs.iter().all(|leg| leg.cells.is_empty()));
    }

    #[test]
    fn navigation_failed_maps_supported_reasons() {
        let cases = [
            (
                net::NavigationFailureReason::Unresolved,
                net_contract::dto::NavigationFailure::Unresolved,
            ),
            (
                net::NavigationFailureReason::Unreachable,
                net_contract::dto::NavigationFailure::Unreachable,
            ),
            (
                net::NavigationFailureReason::AlreadyThere,
                net_contract::dto::NavigationFailure::AlreadyThere,
            ),
            (
                net::NavigationFailureReason::Excluded,
                net_contract::dto::NavigationFailure::Excluded,
            ),
        ];

        for (reason, expected) in cases {
            assert_eq!(
                navigation_failed(net::NavigationFailed {
                    reason: reason as i32
                }),
                Some(net_contract::events::NavigationFailed { reason: expected })
            );
        }
    }

    #[test]
    fn navigation_failed_returns_none_for_unsupported_reasons() {
        for reason in [net::NavigationFailureReason::Unspecified as i32, 99] {
            assert_eq!(navigation_failed(net::NavigationFailed { reason }), None);
        }
    }

    #[test]
    fn navigation_ended_maps_supported_reasons() {
        let cases = [
            (
                net::NavigationEndReason::Arrived,
                net_contract::dto::NavigationEnd::Arrived,
            ),
            (
                net::NavigationEndReason::Cancelled,
                net_contract::dto::NavigationEnd::Cancelled,
            ),
        ];

        for (reason, expected) in cases {
            assert_eq!(
                navigation_ended(net::NavigationEnded {
                    reason: reason as i32
                }),
                Some(net_contract::events::NavigationEnded { reason: expected })
            );
        }
    }

    #[test]
    fn navigation_ended_returns_none_for_unsupported_reasons() {
        for reason in [net::NavigationEndReason::Unspecified as i32, 99] {
            assert_eq!(navigation_ended(net::NavigationEnded { reason }), None);
        }
    }
}
