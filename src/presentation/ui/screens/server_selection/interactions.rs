use bevy::prelude::*;
use bevy_lunex::prelude::*;

/// Component to mark server card in Lunex
#[derive(Component)]
pub struct LunexServerCard {
    pub server_index: usize,
    pub is_selected: bool,
}

/// Component to mark the selected server card
#[derive(Component)]
pub struct LunexSelectedServer;

/// Component to mark server list container
#[derive(Component)]
pub struct LunexServerList;

/// Component to mark connect button in server selection
#[derive(Component)]
pub struct LunexConnectButton;

/// Component to mark server text in simple list
#[derive(Component)]
pub struct LunexServerText {
    pub index: usize,
}

/// Component to track hover state for server items
#[derive(Component)]
pub struct LunexServerHovered;

/// Component to mark server glow text layer
#[derive(Component)]
pub struct LunexServerGlow {
    pub index: usize,
}