use bevy::prelude::*;
use game_engine::core::state::GameState;

use crate::theme;

pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), show_loading_screen);
    }
}

fn show_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb_u8(0x0c, 0x14, 0x11)),
        DespawnOnExit(GameState::Loading),
        children![(
            Text::new("LIFTHRASIR"),
            TextFont {
                font: asset_server.load(theme::FONT_TITLE),
                font_size: 48.0,
                ..default()
            },
            TextColor(theme::EMERALD),
        )],
    ));
}
