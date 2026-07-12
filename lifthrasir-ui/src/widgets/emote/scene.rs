//! Idiomatic BSN chrome for the emote picker window (mirrors the party window).
//!
//! [`window`] builds the whole panel as one `bsn!` tree: root, titlebar with a close
//! button, and a flex-wrap grid of one `@FeathersButton` cell per emote id
//! (`0..MAX_EMOTE_ID`). Each cell carries an [`EmoteButton`] marker over an
//! `ImageNode` caption; clicking a cell writes [`EmoteRequested`] with that id.
//!
//! The thumbnails are populated by a system, not here: `EmoteAssets` is not ready
//! when the panel first spawns (it loads a few frames into gameplay), so cells spawn
//! with an empty `ImageNode` that [`populate_emote_thumbnails`](super) fills once the
//! resource exists. The grid is spawned eagerly and stays hidden until toggled.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::emote::table::MAX_EMOTE_ID;
use game_engine::domain::emote::EmoteRequested;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};

use super::{EmoteButton, EmotePickerRoot};

const WINDOW_LEFT: f32 = 360.0;
const WINDOW_TOP: f32 = 120.0;
const GRID_COLUMNS: f32 = 8.0;
const CELL_SIZE: f32 = 30.0;
const CELL_GAP: f32 = 4.0;
const GRID_PADDING: f32 = 12.0;
const WINDOW_WIDTH: f32 =
    GRID_COLUMNS * CELL_SIZE + (GRID_COLUMNS - 1.0) * CELL_GAP + GRID_PADDING * 2.0;

/// Spawn the whole picker as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    let cells: Vec<_> = (0..MAX_EMOTE_ID).map(emote_cell).collect();
    bsn! {
        EmotePickerRoot
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
        Children [ titlebar(), grid(cells) ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            (
                Text("Emotes")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(15.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_DIM) } }
                Node { width: px(22), height: px(22) }
                on(on_close)
            ),
        ]
    }
}

fn grid<C>(cells: Vec<C>) -> impl Scene
where
    C: Scene + Send + Sync + 'static,
{
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(CELL_GAP),
            row_gap: px(CELL_GAP),
            padding: {UiRect::all(px(GRID_PADDING))},
        }
        ignore_picking()
        Children [ {cells} ]
    }
}

/// One grid cell: a small square `@FeathersButton` tagged with its emote id over an
/// empty `ImageNode` caption. The thumbnail is filled later by
/// [`populate_emote_thumbnails`](super) once `EmoteAssets` exists.
fn emote_cell(id: u32) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { thumbnail() } }
        template_value(EmoteButton(id))
        Node {
            width: px(CELL_SIZE),
            height: px(CELL_SIZE),
            padding: {UiRect::all(px(2))},
        }
        on(on_emote_click)
    }
}

fn thumbnail() -> impl Scene {
    bsn! {
        ImageNode {}
        Node { width: px(CELL_SIZE - 6.0), height: px(CELL_SIZE - 6.0) }
    }
}

/// A square white SVG glyph tinted with `color`; used for the close button.
fn glyph_icon(name: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(size), height: px(size) }
        ignore_picking()
    }
}

/// `Pickable::IGNORE` as a scene, so non-interactive nodes don't swallow clicks.
fn ignore_picking() -> impl Scene {
    bsn! {
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<EmotePickerRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Resolve the activated cell's emote id from its [`EmoteButton`] marker and write the
/// [`EmoteRequested`] intent. The cooldown gate downstream drops it if not ready, so
/// this observer never checks the timer itself.
fn on_emote_click(
    activate: On<Activate>,
    buttons: Query<&EmoteButton>,
    mut writer: MessageWriter<EmoteRequested>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    writer.write(EmoteRequested {
        emote_type: button.0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    #[test]
    fn window_scene_spawns_root_and_one_cell_per_emote() {
        let mut app = scene_app();
        app.world_mut().spawn_scene(window()).unwrap();

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<(), With<EmotePickerRoot>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world.query::<&EmoteButton>().iter(world).count(),
            MAX_EMOTE_ID as usize
        );
    }
}
