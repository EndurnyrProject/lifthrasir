//! In-game Escape menu, styled after the death dialog.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText};
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::combat::components::DeadEntity;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::targeting::cancel_targeting;
use game_engine::domain::input::ui_unfocused;
use net_contract::commands::RespawnRequested;

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER, install_norse_theme};
use crate::widgets::info_modal::InfoModalRoot;
use crate::widgets::npc_dialog::ActiveNpcDialog;
use crate::widgets::settings_window::{SettingsUi, SettingsWindowRoot};
use crate::widgets::shop_window::ShopSession;
use crate::widgets::system_dialog::SystemDialogRoot;

const DIALOG_Z: i32 = i32::MAX - 3;
const DIALOG_WIDTH: f32 = 340.0;

pub struct EscapeMenuPlugin;

impl Plugin for EscapeMenuPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(
            Update,
            toggle_escape_menu
                .after(cancel_targeting)
                .after(crate::widgets::info_modal::show_info_modal)
                .after(crate::widgets::system_dialog::show_system_dialog)
                .run_if(in_state(GameState::InGame))
                .run_if(ui_unfocused),
        );
    }
}

#[derive(Component, Default, Clone)]
pub struct EscapeMenuRoot;

type ModalBlocker = Or<(With<InfoModalRoot>, With<SystemDialogRoot>)>;
type DeadLocalPlayer = (With<LocalPlayer>, With<DeadEntity>);

#[derive(SystemParam)]
struct EscapeBlockers<'w, 's> {
    settings_ui: Res<'w, SettingsUi>,
    npc_dialog: Option<Res<'w, ActiveNpcDialog>>,
    shop: Option<Res<'w, ShopSession>>,
    modal: Query<'w, 's, (), ModalBlocker>,
    dead: Query<'w, 's, (), DeadLocalPlayer>,
}

impl EscapeBlockers<'_, '_> {
    fn active(&self) -> bool {
        self.settings_ui.listening.is_some()
            || self.npc_dialog.is_some()
            || self.shop.is_some()
            || !self.modal.is_empty()
            || !self.dead.is_empty()
    }
}

fn toggle_escape_menu(
    keys: Res<ButtonInput<KeyCode>>,
    blockers: EscapeBlockers,
    roots: Query<Entity, With<EscapeMenuRoot>>,
    mut settings: Query<&mut Visibility, With<SettingsWindowRoot>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) || blockers.active() {
        return;
    }
    if let Ok(mut visibility) = settings.single_mut()
        && *visibility != Visibility::Hidden
    {
        *visibility = Visibility::Hidden;
        return;
    }
    if let Ok(root) = roots.single() {
        commands.entity(root).despawn();
        return;
    }
    commands
        .spawn_scene(escape_menu())
        .insert(DespawnOnExit(GameState::InGame));
}

fn close_menu(commands: &mut Commands, roots: &Query<Entity, With<EscapeMenuRoot>>) {
    for root in roots {
        commands.entity(root).despawn();
    }
}

fn on_character_select(
    _: On<Activate>,
    roots: Query<Entity, With<EscapeMenuRoot>>,
    mut commands: Commands,
    mut respawn: MessageWriter<RespawnRequested>,
) {
    respawn.write(RespawnRequested { type_: 1 });
    close_menu(&mut commands, &roots);
}

fn on_settings(
    _: On<Activate>,
    roots: Query<Entity, With<EscapeMenuRoot>>,
    mut settings: Query<&mut Visibility, With<SettingsWindowRoot>>,
    mut commands: Commands,
) {
    if let Ok(mut visibility) = settings.single_mut() {
        *visibility = Visibility::Visible;
    }
    close_menu(&mut commands, &roots);
}

fn on_close_game(_: On<Activate>, mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}

fn on_exit(_: On<Activate>, roots: Query<Entity, With<EscapeMenuRoot>>, mut commands: Commands) {
    close_menu(&mut commands, &roots);
}

fn escape_menu() -> impl Scene {
    bsn! {
        EscapeMenuRoot
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.012, 0.027, 0.024, 0.55)})
        GlobalZIndex({DIALOG_Z})
        Pickable
        Children [ card() ]
    }
}

fn card() -> impl Scene {
    bsn! {
        Node {
            width: px(DIALOG_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: px(14),
            padding: {UiRect::axes(px(28), px(26))},
            border: px(1),
            border_radius: BorderRadius::all(px(14)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Children [
            title("Menu"),
            (
                @FeathersButton { @caption: bsn! { button_label("Return to Character Selection") } }
                Node { height: px(40) }
                on(on_character_select)
            ),
            (
                @FeathersButton { @caption: bsn! { button_label("Settings") } }
                Node { height: px(40) }
                on(on_settings)
            ),
            (
                @FeathersButton { @caption: bsn! { button_label("Close Game") } }
                Node { height: px(40) }
                on(on_close_game)
            ),
            (
                @FeathersButton { @caption: bsn! { button_label("Exit") } }
                Node { height: px(40) }
                on(on_exit)
            ),
        ]
    }
}

fn title(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
            font_size: {FontSize::Px(19.0)},
        }
        TextColor({theme::DISPLAY_GOLD})
        Node { align_self: {AlignSelf::Center}, margin: {UiRect::bottom(px(4))} }
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

fn button_label(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        ThemedText
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::input::targeting::TargetingMode;
    use game_engine::presentation::ui::events::{
        DialogSeverity, ShowSystemDialog, SystemDialogKind,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<SettingsUi>();
        app.init_resource::<TargetingMode>();
        app.add_message::<RespawnRequested>();
        app.add_message::<AppExit>();
        app.add_message::<ShowSystemDialog>();
        app.add_systems(
            Update,
            (
                cancel_targeting,
                crate::widgets::system_dialog::show_system_dialog,
                toggle_escape_menu
                    .after(cancel_targeting)
                    .after(crate::widgets::system_dialog::show_system_dialog),
            ),
        );
        app
    }

    fn menu_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<EscapeMenuRoot>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn escape_toggles_the_menu() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(menu_count(&mut app), 1);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(menu_count(&mut app), 0);
    }

    #[test]
    fn escape_closes_visible_settings_instead_of_opening_the_menu() {
        let mut app = test_app();
        let settings = app
            .world_mut()
            .spawn((SettingsWindowRoot, Visibility::Visible))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert_eq!(
            app.world().entity(settings).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(menu_count(&mut app), 0);
    }

    #[test]
    fn first_escape_cancels_targeting_without_opening_the_menu() {
        let mut app = test_app();
        *app.world_mut().resource_mut::<TargetingMode>() = TargetingMode::AwaitingGround {
            skill_id: 18,
            level: 1,
        };
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert_eq!(
            *app.world().resource::<TargetingMode>(),
            TargetingMode::Idle
        );
        assert_eq!(menu_count(&mut app), 0);
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
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let dialogs = app
            .world_mut()
            .query_filtered::<(), With<SystemDialogRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(dialogs, 1);
        assert_eq!(menu_count(&mut app), 0);
    }

    #[test]
    fn character_select_requests_type_1_and_closes() {
        let mut app = App::new();
        app.add_message::<RespawnRequested>();
        app.world_mut().spawn(EscapeMenuRoot);
        let button = app
            .world_mut()
            .spawn_empty()
            .observe(on_character_select)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<RespawnRequested>>();
        let sent: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].type_, 1);
        assert_eq!(menu_count(&mut app), 0);
    }

    #[test]
    fn settings_button_opens_settings_and_closes() {
        let mut app = App::new();
        app.world_mut().spawn(EscapeMenuRoot);
        let settings = app
            .world_mut()
            .spawn((SettingsWindowRoot, Visibility::Hidden))
            .id();
        let button = app.world_mut().spawn_empty().observe(on_settings).id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        assert_eq!(
            app.world().entity(settings).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(menu_count(&mut app), 0);
    }

    #[test]
    fn close_game_requests_successful_app_exit() {
        let mut app = App::new();
        app.add_message::<AppExit>();
        let button = app.world_mut().spawn_empty().observe(on_close_game).id();
        app.world_mut().trigger(Activate { entity: button });

        let messages = app.world().resource::<Messages<AppExit>>();
        assert_eq!(
            messages.iter_current_update_messages().next(),
            Some(&AppExit::Success)
        );
    }

    #[test]
    fn exit_closes_the_menu() {
        let mut app = App::new();
        app.world_mut().spawn(EscapeMenuRoot);
        let button = app.world_mut().spawn_empty().observe(on_exit).id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        assert_eq!(menu_count(&mut app), 0);
    }
}
