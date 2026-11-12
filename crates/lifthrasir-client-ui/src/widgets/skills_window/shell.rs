//! The Skills window frame, authored as one `bsn!` tree: the title bar, the mounts of
//! the job tree and the footer, and the scrolling details panel. Sizes follow the
//! `Skills` artboard of the Lifthrasir Widgets design.

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::PlayerAction;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    WINDOW_SHADOW, close_button, inset, window_title_bar, window_titled,
};

const WINDOW_LEFT: f32 = 130.0;
const WINDOW_TOP: f32 = 30.0;
const WINDOW_WIDTH: f32 = 1004.0;
const TREE_COLUMN: f32 = 636.0;
const DETAILS_WIDTH: f32 = 300.0;
const DETAILS_HEIGHT: f32 = 520.0;

#[derive(Component, Default, Clone)]
pub struct SkillsWindowRoot;

/// The title bar, which is the drag handle.
#[derive(Component, Default, Clone)]
pub struct SkillsTitlebar;

/// The player's job and job level, shown in the title bar.
#[derive(Component, Default, Clone)]
pub struct SkillsSubtitle;

/// Mount of the job tabs and the tree canvas.
#[derive(Component, Default, Clone)]
pub struct SkillsBody;

/// Mount of the selected skill's details. It is the panel's scroll viewport.
#[derive(Component, Default, Clone)]
pub struct SkillsDetails;

/// Mount of the point bank and the plan buttons.
#[derive(Component, Default, Clone)]
pub struct SkillsFooter;

/// Spawn the window, hidden, under `parent`.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        SkillsWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        Pickable
        Children [
            @title_bar()
            --
            Node {
                column_gap: px(20),
                align_items: AlignItems::FlexStart,
                margin: {UiRect::top(px(10))},
            }
            @ignore_picking()
            Children [
                SkillsBody
                Node {
                    width: px(TREE_COLUMN),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
                --
                @details_panel()
            ]
            --
            SkillsFooter
            Node {
                flex_direction: FlexDirection::Column,
                margin: {UiRect::new(px(0), px(0), px(14), px(8))},
            }
            @ignore_picking()
        ]
    }
}

fn title_bar() -> impl Scene {
    bsn! {
        SkillsTitlebar
        @window_title_bar("Skills", bsn! {
            Node { align_items: AlignItems::Center, column_gap: px(12) }
            @ignore_picking()
            Children [
                SkillsSubtitle
                @chrome_text(String::new(), 12.0, ro_dark::LABEL)
                --
                @close_button()
                on(close_window)
            ]
        })
        Pickable
        on(drag_window::<SkillsTitlebar, SkillsWindowRoot>)
    }
}

fn details_panel() -> impl Scene {
    bsn! {
        @inset()
        Node {
            width: px(DETAILS_WIDTH),
            height: px(DETAILS_HEIGHT),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
        }
        @ignore_picking()
        Children [
            #details
            SkillsDetails
            ScrollArea
            Pickable
            Node {
                position_type: PositionType::Absolute,
                left: px(0), top: px(0), right: px(0), bottom: px(0),
                flex_direction: FlexDirection::Column,
                padding: px(16),
                overflow: {Overflow::scroll_y()},
            }
            --
            @FeathersScrollbar { @target: #details, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(4),
                top: px(8),
                bottom: px(8),
                width: px(6),
            }
        ]
    }
}

fn close_window(_: On<Activate>, mut roots: Query<&mut Visibility, With<SkillsWindowRoot>>) {
    for mut visibility in &mut roots {
        *visibility = Visibility::Hidden;
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
