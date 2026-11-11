//! The Character window frame, authored as one `bsn!` tree: the title bar, the header
//! mount, and the equipment and stats columns. Live content is projected into the
//! mounts and marked texts by the systems of the sibling modules.

use bevy::prelude::*;
use lifthrasir_ui::widgets::chrome::{drag_window, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    WINDOW_SHADOW, close_button, divider, divider_diamond, window_title_bar, window_titled,
};

use crate::widgets::character_window::{
    CharacterIdentityMount, CharacterTitlebar, CharacterWindowRoot, attributes, equip,
    on_close_click,
};

const WINDOW_LEFT: f32 = 170.0;
const WINDOW_TOP: f32 = 30.0;
const WINDOW_WIDTH: f32 = 940.0;
const EQUIP_COLUMN: f32 = 612.0;
const STATS_COLUMN: f32 = 260.0;

/// Spawn the window, hidden, under `parent`.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        CharacterWindowRoot
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
            CharacterIdentityMount
            Node { flex_direction: FlexDirection::Column, margin: {UiRect::top(px(10))} }
            @ignore_picking()
            --
            @divider(divider_diamond())
            Node { margin: {UiRect::vertical(px(10))} }
            --
            Node {
                column_gap: px(20),
                align_items: AlignItems::FlexStart,
                margin: {UiRect::bottom(px(8))},
            }
            @ignore_picking()
            Children [
                @equip::equipment_column()
                Node { width: px(EQUIP_COLUMN), flex_shrink: 0.0 }
                --
                @attributes::stats_column()
                Node { width: px(STATS_COLUMN), flex_shrink: 0.0 }
            ]
        ]
    }
}

fn title_bar() -> impl Scene {
    bsn! {
        CharacterTitlebar
        @window_title_bar("Character", bsn! {
            @close_button()
            on(on_close_click)
        })
        Pickable
        on(drag_window::<CharacterTitlebar, CharacterWindowRoot>)
    }
}
