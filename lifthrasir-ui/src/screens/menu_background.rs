use bevy::prelude::*;
use game_engine::core::state::GameState;

/// Loaded through the `ro://` composite source. Paths are joined onto the data
/// folder root (`assets/data`), so the bare filename maps to `assets/data/main_bg.png`.
const BACKGROUND_IMAGE: &str = "ro://main_bg.png";

pub struct MenuBackgroundPlugin;

impl Plugin for MenuBackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_menu_background);
        app.add_systems(Update, toggle_on_transition);
    }
}

/// Full-screen image behind the extended_ui menu screens. The menu roots are
/// transparent (see the `ui/*.css` screen selectors) so this shows through.
#[derive(Component)]
struct MenuBackground;

fn is_menu_state(state: &GameState) -> bool {
    matches!(
        state,
        GameState::Login
            | GameState::ServerSelection
            | GameState::CharacterSelection
            | GameState::CharacterCreation
    )
}

fn spawn_menu_background(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode::new(asset_server.load(BACKGROUND_IMAGE)),
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
        Pickable::IGNORE,
        MenuBackground,
    ));
}

fn toggle_on_transition(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut background: Single<&mut Visibility, With<MenuBackground>>,
) {
    let Some(entered) = transitions.read().last().and_then(|event| event.entered.as_ref()) else {
        return;
    };
    **background = if is_menu_state(entered) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
