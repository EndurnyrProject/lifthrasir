use bevy::ui_widgets::Activate;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, futures_lite::future::poll_once},
    text::{FontSize, FontSourceTemplate},
};
use bevy_feathers::{
    controls::{
        FeathersMenu, FeathersMenuButton, FeathersMenuDivider, FeathersMenuItem, FeathersMenuPopup,
    },
    dark_theme::create_dark_theme,
    theme::{ThemedText, UiTheme},
    tokens,
};

use crate::application::{EditorDocument, EditorNotice, EditorSystems, control_pressed, save};
use crate::{infrastructure::content::MapSceneRequest, map_asset_path, presentation::MapAssetPath};

const MENU_BAR_HEIGHT: f32 = 28.0;
const FILE_MENU_WIDTH: f32 = 220.0;

/// Keep Feathers' menu controls aligned with the editor's blue/slate overlays.
pub(super) fn editor_theme() -> UiTheme {
    let mut theme = UiTheme(create_dark_theme());
    let color = &mut theme.0.color;
    let background = Color::srgb(0.075, 0.085, 0.12);
    let hover = Color::srgb(0.12, 0.16, 0.25);
    let pressed = Color::srgb(0.16, 0.28, 0.48);
    let border = Color::srgb(0.23, 0.28, 0.4);
    let text = Color::srgb(0.9, 0.93, 1.0);

    color.insert(tokens::BUTTON_BG, background);
    color.insert(tokens::BUTTON_BG_HOVER, hover);
    color.insert(tokens::BUTTON_BG_PRESSED, pressed);
    color.insert(tokens::BUTTON_TEXT, text);
    color.insert(tokens::MENU_BG, background);
    color.insert(tokens::MENU_BORDER, border);
    color.insert(tokens::MENUITEM_BG_HOVER, hover);
    color.insert(tokens::MENUITEM_BG_PRESSED, pressed);
    color.insert(tokens::MENUITEM_BG_FOCUSED, pressed);
    color.insert(tokens::MENUITEM_TEXT, text);
    theme
}

#[derive(Component, Default, Clone)]
struct NoticeLabel;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FileAction {
    #[default]
    Open,
    Save,
    Quit,
}

#[derive(Resource, Default)]
struct MapFilePicker(Option<Task<Option<std::path::PathBuf>>>);

pub(super) struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    open_file_shortcut,
                    poll_map_file_picker,
                    update_notice.in_set(EditorSystems::Present),
                ),
            )
            .init_resource::<MapFilePicker>();
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_scene(menu_bar());
    commands.spawn_scene(command_overlay());
    commands.spawn_scene(notice_overlay());
}

fn menu_bar() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: px(MENU_BAR_HEIGHT),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            padding: {UiRect::horizontal(px(4))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor(Color::srgb(0.075, 0.085, 0.12))
        BorderColor::all(Color::srgb(0.23, 0.28, 0.4))
        GlobalZIndex(100)
        Pickable
        Children [ file_menu() ]
    }
}

fn file_menu() -> impl Scene {
    bsn! {
        @FeathersMenu
        Node { position_type: PositionType::Relative, width: px(58), height: percent(100) }
        Children [
            (
                @FeathersMenuButton { @caption: bsn! { Text("File") ThemedText } }
                Node { width: percent(100), height: percent(100) }
            ),
            file_popup(),
        ]
    }
}

fn file_popup() -> impl Scene {
    bsn! {
        @FeathersMenuPopup
        Node {
            width: px(FILE_MENU_WIDTH),
            min_width: px(FILE_MENU_WIDTH),
        }
        Children [
            file_action("Open File", "Ctrl+O", FileAction::Open),
            file_action("Save File", "Ctrl+S", FileAction::Save),
            @FeathersMenuDivider,
            file_action("Quit", "Alt+F4", FileAction::Quit),
        ]
    }
}

fn file_action(label_text: &'static str, shortcut: &'static str, action: FileAction) -> impl Scene {
    bsn! {
        template_value(action)
        @FeathersMenuItem { @caption: bsn! { menu_item_caption(label_text, shortcut) } }
        on(on_file_action)
    }
}

fn menu_item_caption(label_text: &'static str, shortcut: &'static str) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
        }
        Children [
            (Text(label_text) ThemedText),
            label(shortcut, 12.0, Color::srgb(0.48, 0.52, 0.62)),
        ]
    }
}

fn label(text: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        Pickable::IGNORE
    }
}

fn on_file_action(
    activate: On<Activate>,
    actions: Query<&FileAction>,
    document: Option<ResMut<EditorDocument>>,
    mut notice: ResMut<EditorNotice>,
    mut picker: ResMut<MapFilePicker>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        FileAction::Open => start_map_file_picker(&mut picker),
        FileAction::Save => {
            if let Some(mut document) = document {
                save(&mut document, &mut notice);
            } else {
                notice.show("Open a map before saving");
            }
        }
        FileAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

fn start_map_file_picker(picker: &mut MapFilePicker) {
    if picker.0.is_some() {
        return;
    }
    picker.0 = Some(IoTaskPool::get().spawn(async {
        rfd::AsyncFileDialog::new()
            .set_directory("assets/data/maps")
            .add_filter("Lifthrasir maps", &["glb"])
            .pick_file()
            .await
            .map(|file| file.path().to_path_buf())
    }));
}

fn open_file_shortcut(keys: Res<ButtonInput<KeyCode>>, mut picker: ResMut<MapFilePicker>) {
    if control_pressed(&keys) && keys.just_pressed(KeyCode::KeyO) {
        start_map_file_picker(&mut picker);
    }
}

fn poll_map_file_picker(
    mut commands: Commands,
    mut picker: ResMut<MapFilePicker>,
    document: Option<ResMut<EditorDocument>>,
    mut map_path: ResMut<MapAssetPath>,
    mut requests: Query<(Entity, &mut MapSceneRequest)>,
    mut notice: ResMut<EditorNotice>,
) {
    let Some(task) = picker.0.as_mut() else {
        return;
    };
    let Some(path) = block_on(poll_once(task)) else {
        return;
    };
    picker.0 = None;
    let Some(path) = path else {
        return;
    };
    let asset_path = match map_asset_path(&path) {
        Ok(path) => path,
        Err(error) => {
            notice.show(format!("Open failed: {error}"));
            return;
        }
    };
    let replacement = match lifthrasir_map_editor::MapDocument::open(&path) {
        Ok(document) => document,
        Err(error) => {
            notice.show(format!("Open failed: {error}"));
            return;
        }
    };
    if let Some(mut document) = document {
        *document = EditorDocument(replacement);
    } else {
        commands.insert_resource(EditorDocument(replacement));
    }
    map_path.0 = Some(asset_path.clone());
    if let Ok((root, mut request)) = requests.single_mut() {
        request.path = format!("ro://{asset_path}");
        commands
            .entity(root)
            .remove::<lifthrasir_runtime::world::MapSceneHandle>();
    } else {
        commands.spawn(MapSceneRequest {
            path: format!("ro://{asset_path}"),
        });
    }
    notice.show(format!("Opened {}", path.display()));
}

fn command_overlay() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            right: px(16),
            width: px(310),
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(Color::srgba(0.02, 0.025, 0.04, 0.88))
        BorderColor::all(Color::srgb(0.35, 0.45, 0.7))
        Pickable::IGNORE
        Children [
            (
                Text("Camera\nWASD - move · Space/Left Alt - up/down\nRMB - look · Wheel - speed\nP - toggle grid\n\nCollision\nLMB / arrows - select cell\nF - toggle walkability\nCtrl+Z / Ctrl+Y - undo / redo\nCtrl+S - save\n\nProps\nClick a prop, drag gizmo axis\nI/K J/L U/O - move\nQ/E - rotate · G/R - gizmo mode\nCtrl+D duplicate · Delete remove")
                TextFont {
                    font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
                    font_size: FontSize::Px(14.0),
                }
                TextColor(Color::srgb(0.9, 0.93, 1.0))
                Pickable::IGNORE
            )
        ]
    }
}

fn notice_overlay() -> impl Scene {
    bsn! {
        NoticeLabel
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: px(18),
            padding: UiRect::axes(px(12), px(8)),
        }
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0))
        Text("")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
            font_size: FontSize::Px(16.0),
        }
        TextColor(Color::srgb(0.9, 0.93, 1.0))
        Pickable::IGNORE
    }
}

fn update_notice(
    time: Res<Time>,
    mut notice: ResMut<EditorNotice>,
    mut labels: Query<(&mut Text, &mut BackgroundColor), With<NoticeLabel>>,
) {
    let Some(timer) = notice.expires.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if timer.is_finished() {
        notice.message.clear();
        notice.expires = None;
    }
    let visible = !notice.message.is_empty();
    for (mut label, mut background) in &mut labels {
        label.0 = notice.message.clone();
        background.0 = if visible {
            Color::srgba(0.02, 0.025, 0.04, 0.88)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
    }
}
