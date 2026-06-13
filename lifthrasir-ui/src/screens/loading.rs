use bevy::prelude::*;
use game_engine::core::state::GameState;

use crate::theme;

pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen);
    }
}

fn spawn_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        DespawnOnExit(GameState::Loading),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(theme::FORGE_SOOT),
        children![(
            Text::new("LIFTHRASIR"),
            TextFont {
                font: asset_server.load(theme::FONT_TITLE),
                font_size: 48.0,
                ..default()
            },
            TextColor(theme::ENERGETIC_GREEN),
        )],
    ));
}
