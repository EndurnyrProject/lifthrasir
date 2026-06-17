//! In-game HUD overlay (raw `bevy_ui`).
//!
//! A full-screen, pickable-ignored root (so clicks reach the world) hosting the
//! status-frame and chat-box widgets. Built on `OnEnter(GameState::InGame)` and torn
//! down by `DespawnOnExit`; the two widget sub-plugins drive their marked elements.

use bevy::prelude::*;
use game_engine::core::state::GameState;

pub mod character_info;
pub mod chat_box;
pub mod draggable;
pub mod status_window;

pub struct InGameHudPlugin;

impl Plugin for InGameHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), show_hud);
        app.add_plugins((
            character_info::CharacterInfoPlugin,
            chat_box::ChatBoxPlugin,
            status_window::StatusWindowPlugin,
        ));
    }
}

fn show_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(GameState::InGame),
        ))
        .id();

    character_info::spawn_status_frame(&mut commands, root, &asset_server);
    chat_box::spawn_chat_box(&mut commands, root, &asset_server);
    status_window::spawn_status_window(&mut commands, root, &asset_server);
}
