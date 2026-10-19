//! Protocol-neutral navigation route types.

/// One positional route leg. `cells` is empty for topology-only future legs.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteLeg {
    pub map: String,
    pub cells: Vec<(u16, u16)>,
    /// Portal to leave by; `None` on the final leg.
    pub exit_portal: Option<String>,
    /// Map that portal leads to; `None` on the final leg.
    pub next_map: Option<String>,
}

/// The map cell where a route ends.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteDestination {
    pub map: String,
    pub x: u16,
    pub y: u16,
}

/// A target the server can plan a route to.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationTarget {
    Coord { map: String, x: u16, y: u16 },
    Map(String),
    Npc(String),
    Monster(u32),
}

/// Why the server could not start navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationFailure {
    Unresolved,
    Unreachable,
    AlreadyThere,
    Excluded,
}

/// Why the server ended navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationEnd {
    Arrived,
    Cancelled,
}
