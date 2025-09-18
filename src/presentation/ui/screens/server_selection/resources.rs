use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ServerSelectionState {
    pub selected_server_index: Option<usize>,
    pub initialized: bool,
}

#[derive(Component)]
pub struct ServerSelectionScreen;

#[derive(Component)]
pub struct ConnectButton;