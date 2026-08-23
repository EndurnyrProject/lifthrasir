use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::dto::{NavigationEnd, NavigationFailure, RouteDestination, RouteLeg};

/// A complete route snapshot. Wholly replaces any previous route — never a delta.
#[derive(Message, Debug, Clone, PartialEq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct RouteUpdated {
    pub legs: Vec<RouteLeg>,
    pub current: u32,
    pub destination: RouteDestination,
    pub hide_window: bool,
}

/// The server could not start navigation.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct NavigationFailed {
    pub reason: NavigationFailure,
}

/// The server ended navigation.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct NavigationEnded {
    pub reason: NavigationEnd,
}
