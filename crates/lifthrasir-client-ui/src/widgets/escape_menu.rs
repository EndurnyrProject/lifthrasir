//! The in-game Escape menu, after the `Escape Menu` artboard.
//!
//! Escape opens it over a dimmed world: who is playing, then Resume, Settings,
//! Character Selection, Log Out and Quit Game. The last two ask first. The arrows move
//! the lit row and Enter activates it; Escape backs out of the prompt, then the menu.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::combat::components::DeadEntity;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::targeting::cancel_targeting;
use lifthrasir_client::domain::input::ui_unfocused;
use lifthrasir_client::domain::world::spawn_context::MapSpawnContext;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, HoverColors, WINDOW_SHADOW, close_button, display_text, window_title,
    window_title_bar, window_titled,
};
use net_contract::commands::RespawnRequested;

use crate::screens::shared::{
    DANGER_TINT, RowTone, menu_row, modal_layer, usable, vignette, wide_button,
};
use crate::widgets::card_compounding::ActiveCardPicker;
use crate::widgets::info_modal::InfoModalRoot;
use crate::widgets::npc_dialog::ActiveNpcDialog;
use crate::widgets::settings_window::{SettingsUi, SettingsWindowRoot};
use crate::widgets::shop_window::ShopSession;
use crate::widgets::skill_menu::ActiveSkillMenu;
use crate::widgets::skill_text_input::ActiveSkillTextInput;
use crate::widgets::status_bar::{Hero, Portrait};
use crate::widgets::system_dialog::SystemDialogRoot;

/// Under the system dialog, with a tier left free for the prompt that asks before
/// leaving.
const DIALOG_Z: i32 = i32::MAX - 4;
const PROMPT_Z: i32 = DIALOG_Z + 1;

const _: () = assert!(
    PROMPT_Z < crate::widgets::system_dialog::DIALOG_Z,
    "the escape menu must stack under the system dialog"
);
const MENU_WIDTH: f32 = 400.0;
const PROMPT_WIDTH: f32 = 380.0;
const ROW_HEIGHT: f32 = 44.0;

/// The rows from top to bottom. The gap falls before `GAP_BEFORE`.
const ROWS: [(EscapeAction, RowTone, &str, &str, Option<&str>); 5] = [
    (
        EscapeAction::Resume,
        RowTone::Primary,
        "play",
        "Resume",
        Some("Esc"),
    ),
    (
        EscapeAction::Settings,
        RowTone::Plain,
        "gear",
        "Settings",
        None,
    ),
    (
        EscapeAction::CharacterSelect,
        RowTone::Plain,
        "members",
        "Character Selection",
        None,
    ),
    (
        EscapeAction::Ask(Leave::LogOut),
        RowTone::Plain,
        "door",
        "Log Out",
        None,
    ),
    (
        EscapeAction::Ask(Leave::Quit),
        RowTone::Danger,
        "power",
        "Quit Game",
        None,
    ),
];
/// The first row of the group that leaves the game.
const GAP_BEFORE: usize = 2;

pub struct EscapeMenuPlugin;

impl Plugin for EscapeMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                toggle_escape_menu
                    .after(cancel_targeting)
                    .after(crate::widgets::info_modal::show_info_modal)
                    .after(crate::widgets::system_dialog::show_system_dialog)
                    .run_if(ui_unfocused),
                // Enter is the menu's while it is open, so the chat does not open on it.
                menu_keys.before(crate::widgets::chat_box::chat_input_control),
                paint_cursor,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// The menu's layer, holding the row the keys act on.
#[derive(Component, Default, Clone)]
pub struct EscapeMenuRoot {
    cursor: usize,
}

/// A row of the menu, by its place in [`ROWS`].
#[derive(Component, Default, Clone, Copy)]
struct MenuRow(usize);

/// The layer of the prompt that asks before leaving.
#[derive(Component, Default, Clone)]
struct LeavePrompt;

/// A way out of the game that asks first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    #[default]
    LogOut,
    Quit,
}

impl Leave {
    /// The prompt's title, its body and its confirming label.
    fn prompt(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::LogOut => (
                "Log out?",
                "You\u{2019}ll return to the sign-in screen.",
                "Log Out",
            ),
            Self::Quit => ("Quit Lifthrasir?", "The game will close.", "Quit Game"),
        }
    }
}

/// What a button of the menu does when activated.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum EscapeAction {
    #[default]
    Resume,
    Settings,
    CharacterSelect,
    /// Raises the prompt for a way out.
    Ask(Leave),
    Stay,
    Leave(Leave),
}

type ModalBlocker = Or<(With<InfoModalRoot>, With<SystemDialogRoot>)>;
type DeadLocalPlayer = (With<LocalPlayer>, With<DeadEntity>);

#[derive(SystemParam)]
struct EscapeBlockers<'w, 's> {
    settings_ui: Res<'w, SettingsUi>,
    npc_dialog: Option<Res<'w, ActiveNpcDialog>>,
    shop: Option<Res<'w, ShopSession>>,
    skill_menu: Option<Res<'w, ActiveSkillMenu>>,
    card_picker: Option<Res<'w, ActiveCardPicker>>,
    skill_text_input: Option<Res<'w, ActiveSkillTextInput>>,
    modal: Query<'w, 's, (), ModalBlocker>,
    dead: Query<'w, 's, (), DeadLocalPlayer>,
}

impl EscapeBlockers<'_, '_> {
    fn active(&self) -> bool {
        self.settings_ui.listening.is_some()
            || self.npc_dialog.is_some()
            || self.shop.is_some()
            || self.skill_menu.is_some()
            || self.card_picker.is_some()
            || self.skill_text_input.is_some()
            || !self.modal.is_empty()
            || !self.dead.is_empty()
    }
}

/// Everything a menu action touches.
#[derive(SystemParam)]
struct EscapeActions<'w, 's> {
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, Entity, With<EscapeMenuRoot>>,
    prompts: Query<'w, 's, Entity, With<LeavePrompt>>,
    settings: Query<'w, 's, &'static mut Visibility, With<SettingsWindowRoot>>,
    respawn: MessageWriter<'w, RespawnRequested>,
    exit: MessageWriter<'w, AppExit>,
    next_state: ResMut<'w, NextState<GameState>>,
}

impl EscapeActions<'_, '_> {
    fn run(&mut self, action: EscapeAction) {
        match action {
            EscapeAction::Resume => self.close(),
            EscapeAction::Settings => {
                if let Ok(mut visibility) = self.settings.single_mut() {
                    *visibility = Visibility::Visible;
                }
                self.close();
            }
            EscapeAction::CharacterSelect => {
                self.respawn.write(RespawnRequested { type_: 1 });
                self.close();
            }
            EscapeAction::Ask(leave) => {
                let Ok(root) = self.roots.single() else {
                    return;
                };
                self.close_prompt();
                self.commands
                    .spawn_scene(leave_prompt(leave))
                    .insert(ChildOf(root));
            }
            EscapeAction::Stay => self.close_prompt(),
            EscapeAction::Leave(Leave::LogOut) => {
                // Entering the login screen ends the zone and the account session.
                self.next_state.set(GameState::Login);
                self.close();
            }
            EscapeAction::Leave(Leave::Quit) => {
                self.exit.write(AppExit::Success);
            }
        }
    }

    fn close(&mut self) {
        for root in &self.roots {
            self.commands.entity(root).despawn();
        }
    }

    fn close_prompt(&mut self) {
        for prompt in &self.prompts {
            self.commands.entity(prompt).despawn();
        }
    }
}

fn on_escape_button(
    activate: On<Activate>,
    buttons: Query<&EscapeAction>,
    mut menu: EscapeActions,
) {
    if let Ok(action) = buttons.get(activate.entity) {
        menu.run(*action);
    }
}

/// Escape closes the settings window if it is up, else backs out of the prompt, else
/// opens or closes the menu.
fn toggle_escape_menu(
    keys: Res<ButtonInput<KeyCode>>,
    blockers: EscapeBlockers,
    hero: Hero,
    map: Option<Res<MapSpawnContext>>,
    mut menu: EscapeActions,
) {
    if !keys.just_pressed(KeyCode::Escape) || blockers.active() {
        return;
    }
    if let Ok(mut visibility) = menu.settings.single_mut()
        && *visibility != Visibility::Hidden
    {
        *visibility = Visibility::Hidden;
        return;
    }
    if !menu.prompts.is_empty() {
        menu.close_prompt();
        return;
    }
    if !menu.roots.is_empty() {
        menu.close();
        return;
    }
    let who = hero
        .identity()
        .map(|(name, job, level)| (name, about(&job, level, map.as_deref())));
    menu.commands
        .spawn_scene(escape_menu(who))
        .insert(DespawnOnExit(GameState::InGame));
}

/// The line under the hero's name: job, base level and the map they stand on.
fn about(job: &str, level: u32, map: Option<&MapSpawnContext>) -> String {
    let map = map.map_or(String::new(), |map| format!(" \u{b7} {}", map.map_name));
    format!("{job} \u{b7} Lv {level}{map}")
}

/// The row `step` away from `cursor`, wrapping at both ends.
fn stepped(cursor: usize, step: isize) -> usize {
    (cursor as isize + step).rem_euclid(ROWS.len() as isize) as usize
}

/// The arrows move the lit row and Enter activates it. The prompt takes no keys but
/// Escape.
fn menu_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut roots: Query<&mut EscapeMenuRoot>,
    mut menu: EscapeActions,
) {
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    let enter =
        keys.clear_just_pressed(KeyCode::Enter) | keys.clear_just_pressed(KeyCode::NumpadEnter);
    if !menu.prompts.is_empty() {
        return;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        root.cursor = stepped(root.cursor, 1);
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        root.cursor = stepped(root.cursor, -1);
    }
    if enter {
        menu.run(ROWS[root.cursor].0);
    }
}

/// The pointer lights the row it enters.
fn on_row_over(
    over: On<PointerOver>,
    rows: Query<&MenuRow>,
    mut roots: Query<&mut EscapeMenuRoot>,
) {
    let (Ok(row), Ok(mut root)) = (rows.get(over.entity), roots.single_mut()) else {
        return;
    };
    if root.cursor != row.0 {
        root.cursor = row.0;
    }
}

/// Paints the lit row in its hover colours and the others at rest.
fn paint_cursor(
    roots: Query<&EscapeMenuRoot, Changed<EscapeMenuRoot>>,
    mut rows: Query<(
        &MenuRow,
        &HoverColors,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    for (row, colors, mut fill, mut stroke) in &mut rows {
        let (background, border) = if row.0 == root.cursor {
            colors.hover
        } else {
            colors.rest
        };
        fill.0 = background;
        *stroke = BorderColor::all(border);
    }
}

/// The whole menu: a shade over the world centring the window. `who` is the hero's
/// name and the line about them.
fn escape_menu(who: Option<(String, String)>) -> impl Scene {
    let shade = vignette(
        ro_dark::WINDOW_OUTLINE.with_alpha(0.45),
        ro_dark::WINDOW_OUTLINE.with_alpha(0.82),
        0.0,
    );
    let who = who.map(|(name, about)| EntityScene(hero_header(name, about)));
    let rows: Vec<_> = ROWS
        .into_iter()
        .enumerate()
        .map(|(index, row)| row_scene(index, row))
        .collect();
    let close = bsn! {
        @close_button()
        EscapeAction::Resume
        on(on_escape_button)
    };
    bsn! {
        @modal_layer(DIALOG_Z)
        EscapeMenuRoot
        shade
        Children [
            @window_titled()
            Node {
                width: px(MENU_WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Children [
                @window_title_bar("Menu", close)
                --
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    margin: {UiRect::top(px(8))},
                }
                @ignore_picking()
                Children [
                    {who}
                    --
                    {rows}
                    --
                    Node { justify_content: JustifyContent::FlexEnd, margin: {UiRect::top(px(8))} }
                    @ignore_picking()
                    Children [
                        @chrome_text(
                            concat!("Client ", env!("CARGO_PKG_VERSION")).into(),
                            11.0,
                            ro_dark::FAINT,
                        )
                    ]
                ]
            ]
        ]
    }
}

fn row_scene(
    index: usize,
    (action, tone, icon, label, key): (
        EscapeAction,
        RowTone,
        &'static str,
        &'static str,
        Option<&'static str>,
    ),
) -> impl Scene {
    let gap = if index == GAP_BEFORE { 10.0 } else { 0.0 };
    bsn! {
        @menu_row(tone, ROW_HEIGHT, icon, label, key)
        MenuRow(index)
        action
        Node { margin: {UiRect::top(px(gap))} }
        on(on_escape_button)
        on(on_row_over)
    }
}

/// Who is playing: the portrait, the name and the line about them.
fn hero_header(name: String, about: String) -> impl Scene {
    let backdrop = vignette(Color::srgb_u8(0x34, 0x40, 0x4a), ro_dark::INSET, 0.0);
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::bottom(px(14))},
            margin: {UiRect::bottom(px(8))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node {
                width: px(40),
                height: px(40),
                flex_shrink: 0.0,
                border: px(2),
                border_radius: BorderRadius::MAX,
            }
            backdrop
            BorderColor::all(ro_dark::PRIMARY_STROKE)
            @ignore_picking()
            Children [
                Portrait
                Node { width: percent(100), height: percent(100), border_radius: BorderRadius::MAX }
                @ignore_picking()
            ]
            --
            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: px(0) }
            @ignore_picking()
            Children [
                @display_text(name, 15.0, ro_dark::HERO)
                --
                @chrome_text(about, 11.0, ro_dark::MUTED)
            ]
        ]
    }
}

/// The prompt that asks before leaving, over a second shade.
fn leave_prompt(leave: Leave) -> impl Scene {
    let (title, body, confirm) = leave.prompt();
    bsn! {
        @modal_layer(PROMPT_Z)
        LeavePrompt
        BackgroundColor({ro_dark::WINDOW_OUTLINE.with_alpha(0.55)})
        Children [
            @window_titled()
            Node {
                width: px(PROMPT_WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Children [
                @window_title(title)
                --
                Node { margin: {UiRect::top(px(10))} }
                @ignore_picking()
                Children [ @chrome_text(body.into(), 13.0, ro_dark::BODY) ]
                --
                Node {
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(8),
                    margin: {UiRect::top(px(18))},
                }
                @ignore_picking()
                Children [
                    @wide_button(ButtonArt::NORMAL, "Stay", 72.0)
                    EscapeAction::Stay
                    on(on_escape_button)
                    --
                    @wide_button(ButtonArt::NORMAL, confirm, 104.0)
                    @usable(true, DANGER_TINT)
                    EscapeAction::Leave(leave)
                    on(on_escape_button)
                ]
            ]
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::input::targeting::TargetingMode;
    use lifthrasir_client::domain::settings::{AudioConfig, GraphicsSettings, Keybinds};
    use lifthrasir_client::presentation::ui::events::{
        DialogSeverity, ShowSystemDialog, SystemDialogKind,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<GraphicsSettings>();
        app.init_resource::<AudioConfig>();
        app.init_resource::<Keybinds>();
        app.init_resource::<SettingsUi>();
        app.init_resource::<TargetingMode>();
        app.init_resource::<NextState<GameState>>();
        app.add_message::<RespawnRequested>();
        app.add_message::<AppExit>();
        app.add_message::<ShowSystemDialog>();
        app.add_systems(
            Update,
            (
                cancel_targeting,
                crate::widgets::system_dialog::show_system_dialog,
                toggle_escape_menu,
                menu_keys,
                paint_cursor,
            )
                .chain(),
        );
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(key);
        app.update();
    }

    fn count<C: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&C>().iter(world).count()
    }

    fn cursor(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query::<&EscapeMenuRoot>()
            .single(world)
            .unwrap()
            .cursor
    }

    /// Spawns a button carrying `action`, activates it and returns the app.
    fn activated(mut app: App, action: EscapeAction) -> App {
        let button = app.world_mut().spawn(action).observe(on_escape_button).id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();
        app
    }

    fn open_menu() -> App {
        let mut app = test_app();
        press(&mut app, KeyCode::Escape);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 1);
        app
    }

    #[test]
    fn escape_toggles_the_menu() {
        let mut app = open_menu();
        assert_eq!(count::<MenuRow>(&mut app), ROWS.len());

        press(&mut app, KeyCode::Escape);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn escape_closes_visible_settings_instead_of_opening_the_menu() {
        let mut app = test_app();
        let settings = app
            .world_mut()
            .spawn((SettingsWindowRoot, Visibility::Visible))
            .id();
        press(&mut app, KeyCode::Escape);

        assert_eq!(
            app.world().entity(settings).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn first_escape_cancels_targeting_without_opening_the_menu() {
        let mut app = test_app();
        *app.world_mut().resource_mut::<TargetingMode>() = TargetingMode::AwaitingGround {
            skill_id: 18,
            level: 1,
        };
        press(&mut app, KeyCode::Escape);

        assert_eq!(
            *app.world().resource::<TargetingMode>(),
            TargetingMode::Idle
        );
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn same_frame_system_dialog_blocks_the_escape_menu() {
        let mut app = test_app();
        app.world_mut().write_message(ShowSystemDialog {
            severity: DialogSeverity::Info,
            kind: SystemDialogKind::Generic,
            kicker: "System".into(),
            title: "Notice".into(),
            message: "Message".into(),
            code: String::new(),
            button_label: "OK".into(),
            secondary_label: String::new(),
            confirm_state: None,
            correlation: None,
        });
        press(&mut app, KeyCode::Escape);

        assert_eq!(count::<SystemDialogRoot>(&mut app), 1);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn the_arrows_wrap_around_the_rows_and_enter_activates_the_lit_one() {
        let mut app = open_menu();
        assert_eq!(cursor(&mut app), 0);

        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(cursor(&mut app), ROWS.len() - 1);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(cursor(&mut app), 0);

        press(&mut app, KeyCode::Enter);
        assert_eq!(
            count::<EscapeMenuRoot>(&mut app),
            0,
            "Resume closes the menu"
        );
    }

    #[test]
    fn the_lit_row_shows_its_hover_colours() {
        let mut app = open_menu();
        press(&mut app, KeyCode::ArrowDown);

        let world = app.world_mut();
        let fills: Vec<_> = world
            .query::<(&MenuRow, &BackgroundColor)>()
            .iter(world)
            .map(|(row, fill)| (row.0, fill.0))
            .collect();
        for (index, fill) in fills {
            let colors = ROWS[index].1.colors();
            let expected = if index == 1 {
                colors.hover.0
            } else {
                colors.rest.0
            };
            assert_eq!(fill, expected, "row {index}");
        }
    }

    #[test]
    fn leaving_asks_first_and_escape_backs_out_of_the_prompt() {
        let mut app = open_menu();
        for _ in 0..3 {
            press(&mut app, KeyCode::ArrowDown);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(count::<LeavePrompt>(&mut app), 1);
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Unchanged
        ));

        press(&mut app, KeyCode::Enter);
        assert_eq!(
            count::<LeavePrompt>(&mut app),
            1,
            "the prompt takes no Enter"
        );

        press(&mut app, KeyCode::Escape);
        assert_eq!(count::<LeavePrompt>(&mut app), 0);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 1, "the menu stays open");
    }

    #[test]
    fn stay_closes_only_the_prompt() {
        let mut app = open_menu();
        app.world_mut().spawn(LeavePrompt);
        let mut app = activated(app, EscapeAction::Stay);

        assert_eq!(count::<LeavePrompt>(&mut app), 0);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 1);
    }

    #[test]
    fn character_select_requests_type_1_and_closes() {
        let mut app = activated(open_menu(), EscapeAction::CharacterSelect);

        let messages = app.world().resource::<Messages<RespawnRequested>>();
        let sent: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].type_, 1);
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn settings_button_opens_settings_and_closes() {
        let mut app = open_menu();
        let settings = app
            .world_mut()
            .spawn((SettingsWindowRoot, Visibility::Hidden))
            .id();
        let mut app = activated(app, EscapeAction::Settings);

        assert_eq!(
            app.world().entity(settings).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn confirming_log_out_returns_to_the_login_screen() {
        let mut app = activated(open_menu(), EscapeAction::Leave(Leave::LogOut));

        match app.world().resource::<NextState<GameState>>() {
            NextState::Pending(state) => assert_eq!(*state, GameState::Login),
            _ => panic!("logging out should queue the login screen"),
        }
        assert_eq!(count::<EscapeMenuRoot>(&mut app), 0);
    }

    #[test]
    fn confirming_quit_requests_successful_app_exit() {
        let app = activated(open_menu(), EscapeAction::Leave(Leave::Quit));

        let messages = app.world().resource::<Messages<AppExit>>();
        assert_eq!(
            messages.iter_current_update_messages().next(),
            Some(&AppExit::Success)
        );
    }

    #[test]
    fn the_line_about_the_hero_names_the_map_when_it_is_known() {
        assert_eq!(about("Knight", 99, None), "Knight \u{b7} Lv 99");
        let map = MapSpawnContext::new("prontera".into(), 150, 150, 1);
        assert_eq!(
            about("Knight", 99, Some(&map)),
            "Knight \u{b7} Lv 99 \u{b7} prontera"
        );
    }
}
