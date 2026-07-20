use bevy::prelude::*;
use game_engine::core::state::GameState;
use iyes_progress::prelude::ProgressTracker;

use crate::theme;

pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), show_loading_screen)
            .add_systems(
                Update,
                update_loading_bar.run_if(in_state(GameState::Loading)),
            );
    }
}

#[derive(Component)]
struct LoadingBarFill;

fn show_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(24.0),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(0x0c, 0x14, 0x11)),
        DespawnOnExit(GameState::Loading),
        children![
            (
                Text::new("LIFTHRASIR"),
                TextFont {
                    font: asset_server.load(theme::FONT_TITLE).into(),
                    font_size: 48.0.into(),
                    ..default()
                },
                TextColor(theme::EMERALD),
            ),
            (
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(0x1a, 0x2a, 0x22)),
                children![(
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(theme::EMERALD),
                    LoadingBarFill,
                )],
            ),
        ],
    ));
}

fn update_loading_bar(
    tracker: Res<ProgressTracker<GameState>>,
    mut fills: Query<&mut Node, With<LoadingBarFill>>,
) {
    let progress = tracker.get_global_progress();
    if progress.total == 0 {
        return;
    }

    let percent = progress.done as f32 / progress.total as f32 * 100.0;
    for mut node in &mut fills {
        node.width = Val::Percent(percent);
    }
}
