//! The Skills window frame: a draggable titlebar over the body the skill canvas is
//! projected into.
//!
//! NOTE: this is the legacy chrome the window had as a Character window tab. Port it to
//! the RO-dark primitives with the Skills artboard of the Lifthrasir Widgets design.

use bevy::prelude::*;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::PlayerAction;
use lifthrasir_ui::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};
use lifthrasir_ui::widgets::chrome::{body_container, titlebar};

const WINDOW_LEFT: f32 = 300.0;
const WINDOW_TOP: f32 = 100.0;
const WINDOW_WIDTH: f32 = 780.0;

#[derive(Component, Default, Clone)]
pub struct SkillsWindowRoot;

/// The title bar, which is the drag handle.
#[derive(Component, Default, Clone)]
pub struct SkillsTitlebar;

/// The region the skill canvas is rebuilt into.
#[derive(Component, Default, Clone)]
pub struct SkillsBody;

/// Spawn the window, hidden, under `parent`.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        SkillsWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [
            @titlebar::<SkillsTitlebar, SkillsWindowRoot>("skillp", "Skills") --
            @body_container::<SkillsBody>(UiRect::axes(px(14), px(10)))
        ]
    }
}

/// The Skills chord shows or hides the window.
pub(super) fn toggle_skills_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut root: Query<&mut Visibility, With<SkillsWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Skills) {
        return;
    }
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}
