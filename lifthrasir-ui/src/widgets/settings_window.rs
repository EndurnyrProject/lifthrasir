//! Settings window: the draggable shell and the draft/Apply/Cancel/Reset model.
//!
//! The window edits a draft `Settings` clone held in `SettingsUi`; nothing
//! touches the live world until Apply, which persists the draft and emits
//! `ApplySettings`. The three tab bodies are empty placeholders here — the
//! per-tab controls land in later tasks. Spawned hidden at `Startup` so it
//! survives state changes and is reachable from the title screen and in-game.

use bevy::prelude::*;
use bevy_persistent::prelude::Persistent;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::settings::{ApplySettings, Settings};

use crate::theme;
use crate::widgets::draggable::make_draggable;

/// Which slot of an action's bindings a rebind capture targets. Declared here
/// for the `listening` state; the capture flow itself is Task 8.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindSlot {
    Primary,
    Secondary,
}

/// The settings tabs, mirroring the mockup's left rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Sound,
    Input,
}

/// UI-side draft state for the settings window.
///
/// `draft` is the edited copy; `committed` is the last applied value. `Apply`
/// persists `draft` and sets `committed = draft`; the "unsaved changes" dot
/// shows whenever they differ. `listening` is the pending rebind capture target
/// (Task 8 consumes it).
#[derive(Resource, Default)]
pub struct SettingsUi {
    pub draft: Settings,
    pub committed: Settings,
    pub tab: SettingsTab,
    pub listening: Option<(PlayerAction, BindSlot)>,
}

impl SettingsUi {
    /// Whether the draft differs from the last committed value.
    pub fn dirty(&self) -> bool {
        self.draft != self.committed
    }

    /// Discards pending edits and any in-progress rebind capture.
    pub fn cancel(&mut self) {
        self.draft = self.committed.clone();
        self.listening = None;
    }

    /// Resets the draft to the built-in defaults.
    pub fn reset(&mut self) {
        self.draft = Settings::default();
    }
}

/// Apply core: when the draft is dirty, persist it and mark it committed.
/// Returns `true` if it applied (the caller should then emit `ApplySettings`),
/// `false` when clean or when persistence failed.
fn apply_draft(ui: &mut SettingsUi, persistent: &mut Persistent<Settings>) -> bool {
    if !ui.dirty() {
        return false;
    }
    if let Err(error) = persistent.set(ui.draft.clone()) {
        error!("failed to persist settings: {error}");
        return false;
    }
    ui.committed = ui.draft.clone();
    true
}

/// Marks the toggled window root (Escape / close / login gear flip its `Visibility`).
#[derive(Component)]
pub struct SettingsWindowRoot;

/// Marks a tab-rail button so its observer and highlight key off a tab.
#[derive(Component, Clone, Copy)]
struct TabButton(SettingsTab);

/// Marks a tab body so the active-tab system can show/hide it.
#[derive(Component, Clone, Copy)]
struct TabBody(SettingsTab);

/// Marks the unsaved-changes dot so its visibility tracks `dirty()`.
#[derive(Component)]
struct DirtyDot;

/// Marks the Apply button so it dims when there is nothing to apply.
#[derive(Component)]
struct ApplyButton;

pub struct SettingsWindowPlugin;

impl Plugin for SettingsWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsUi>();
        app.add_systems(Startup, spawn_settings_root);
        app.add_systems(
            Update,
            (
                seed_from_persistent.run_if(resource_added::<Persistent<Settings>>),
                toggle_settings.run_if(ui_unfocused),
                refresh_tabs.run_if(resource_changed::<SettingsUi>),
                refresh_footer.run_if(resource_changed::<SettingsUi>),
            ),
        );
    }
}

/// Seeds the draft/committed from the loaded persisted settings, once the
/// engine has inserted `Persistent<Settings>` (its insert runs at `Startup`,
/// so this fires on the first `Update` after the resource appears).
fn seed_from_persistent(persistent: Res<Persistent<Settings>>, mut ui: ResMut<SettingsUi>) {
    ui.draft = (**persistent).clone();
    ui.committed = (**persistent).clone();
}

/// Spawns the (hidden) window root as a top-level UI node so it survives state
/// changes and renders over both the title screen and the in-game HUD.
fn spawn_settings_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            SettingsWindowRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(360.0),
                top: Val::Px(120.0),
                width: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Visibility::Hidden,
            Pickable::default(),
        ))
        .id();

    spawn_settings_window(&mut commands, root, &asset_server);
}

/// Builds the window contents under `root`: titlebar, the tab rail + content
/// pane, and the footer. The titlebar drives dragging; the close `X` hides the
/// root. Tab bodies are empty placeholders.
pub fn spawn_settings_window(commands: &mut Commands, root: Entity, asset_server: &AssetServer) {
    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let titlebar = spawn_titlebar(commands, root, asset_server, &font_title);
    spawn_main(commands, root, &font_body);
    spawn_footer(commands, root, &font_body);

    make_draggable(commands, titlebar, root);
}

fn spawn_titlebar(
    commands: &mut Commands,
    root: Entity,
    asset_server: &AssetServer,
    font_title: &Handle<Font>,
) -> Entity {
    let titlebar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(11.0)),
                border: UiRect {
                    bottom: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::default(),
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        theme::icon(asset_server, "gear", 16.0, theme::GOLD),
        ChildOf(titlebar),
    ));
    commands.spawn((
        theme::label("System Settings", font_title.clone(), 15.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(titlebar),
    ));

    let close = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Pickable::default(),
            ChildOf(titlebar),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "close", 13.0, theme::TEXT_DIM),
        ChildOf(close),
    ));
    commands.entity(close).observe(
        |_: On<Pointer<Click>>, mut window: Query<&mut Visibility, With<SettingsWindowRoot>>| {
            if let Ok(mut visibility) = window.single_mut() {
                *visibility = Visibility::Hidden;
            }
        },
    );

    titlebar
}

/// The tab rail (left) and the content pane (right) with one empty body per tab.
fn spawn_main(commands: &mut Commands, root: Entity, font: &Handle<Font>) {
    let main = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                min_height: Val::Px(260.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    spawn_rail(commands, main, font);

    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(main),
        ))
        .id();

    for tab in [
        SettingsTab::Graphics,
        SettingsTab::Sound,
        SettingsTab::Input,
    ] {
        spawn_tab_body(commands, content, tab, font);
    }
}

const TABS: [(SettingsTab, &str); 3] = [
    (SettingsTab::Graphics, "Graphics"),
    (SettingsTab::Sound, "Sound"),
    (SettingsTab::Input, "Input"),
];

fn spawn_rail(commands: &mut Commands, main: Entity, font: &Handle<Font>) {
    let rail = commands
        .spawn((
            Node {
                width: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect {
                    right: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(main),
        ))
        .id();

    for (tab, label) in TABS {
        spawn_tab_button(commands, rail, tab, label, font);
    }
}

fn spawn_tab_button(
    commands: &mut Commands,
    rail: Entity,
    tab: SettingsTab,
    label: &str,
    font: &Handle<Font>,
) {
    let button = commands
        .spawn((
            TabButton(tab),
            Node {
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            Pickable::default(),
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        theme::label(label, font.clone(), 13.0, theme::TEXT_DIM),
        ChildOf(button),
    ));
    commands.entity(button).observe(on_tab_click);
}

/// Tab click: set the active tab in the draft state.
fn on_tab_click(click: On<Pointer<Click>>, tabs: Query<&TabButton>, mut ui: ResMut<SettingsUi>) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = tab.0;
}

/// An (empty) per-tab body. Only the active tab's body is visible; later tasks
/// fill these with the per-tab controls.
fn spawn_tab_body(commands: &mut Commands, content: Entity, tab: SettingsTab, font: &Handle<Font>) {
    let title = match tab {
        SettingsTab::Graphics => "GRAPHICS",
        SettingsTab::Sound => "SOUND",
        SettingsTab::Input => "INPUT",
    };
    let visibility = if tab == SettingsTab::default() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let body = commands
        .spawn((
            TabBody(tab),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            visibility,
            Pickable::IGNORE,
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        theme::label(title, font.clone(), 11.0, theme::GOLD),
        ChildOf(body),
    ));
}

/// Footer: Reset to Defaults · unsaved-changes dot · Cancel · Apply.
fn spawn_footer(commands: &mut Commands, root: Entity, font: &Handle<Font>) {
    let footer = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect {
                    top: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();

    spawn_footer_button(
        commands,
        footer,
        "Reset to Defaults",
        theme::FIELD,
        font,
        on_reset,
    );

    // Pushes the dirty dot + Cancel/Apply to the right edge.
    commands.spawn((
        Node {
            flex_grow: 1.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(footer),
    ));

    commands.spawn((
        DirtyDot,
        Node {
            width: Val::Px(8.0),
            height: Val::Px(8.0),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::WARN),
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(footer),
    ));

    spawn_footer_button(commands, footer, "Cancel", theme::FIELD, font, on_cancel);
    let apply = spawn_footer_button(commands, footer, "Apply", theme::EMERALD, font, on_apply);
    commands.entity(apply).insert(ApplyButton);
}

fn spawn_footer_button<M>(
    commands: &mut Commands,
    footer: Entity,
    text: &str,
    bg: Color,
    font: &Handle<Font>,
    observer: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) -> Entity {
    let fg = if bg == theme::EMERALD {
        theme::EMERALD_INK
    } else {
        theme::TEXT_DIM
    };
    let button = commands
        .spawn((
            Node {
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(bg),
            Pickable::default(),
            ChildOf(footer),
        ))
        .id();
    commands.spawn((theme::label(text, font.clone(), 13.0, fg), ChildOf(button)));
    commands.entity(button).observe(observer);
    button
}

/// Apply: persist the draft, mark it committed, and request a live re-apply.
/// No-op when the draft is clean.
fn on_apply(
    _: On<Pointer<Click>>,
    mut ui: ResMut<SettingsUi>,
    mut persistent: ResMut<Persistent<Settings>>,
    mut writer: MessageWriter<ApplySettings>,
) {
    if apply_draft(&mut ui, &mut persistent) {
        writer.write(ApplySettings);
    }
}

/// Cancel: discard pending edits and any in-progress rebind capture.
fn on_cancel(_: On<Pointer<Click>>, mut ui: ResMut<SettingsUi>) {
    ui.cancel();
}

/// Reset to Defaults: load the built-in defaults into the draft.
fn on_reset(_: On<Pointer<Click>>, mut ui: ResMut<SettingsUi>) {
    ui.reset();
}

/// Shows the active tab's body and highlights its rail button.
fn refresh_tabs(
    ui: Res<SettingsUi>,
    mut bodies: Query<(&mut Visibility, &TabBody)>,
    mut buttons: Query<(&mut BackgroundColor, &TabButton)>,
) {
    for (mut visibility, body) in &mut bodies {
        *visibility = if body.0 == ui.tab {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (mut bg, button) in &mut buttons {
        let color = if button.0 == ui.tab {
            theme::EMERALD
        } else {
            theme::FIELD
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Shows the unsaved-changes dot and dims Apply when the draft is clean.
fn refresh_footer(
    ui: Res<SettingsUi>,
    mut dot: Query<&mut Visibility, With<DirtyDot>>,
    mut apply: Query<&mut BackgroundColor, With<ApplyButton>>,
) {
    let dirty = ui.dirty();
    if let Ok(mut visibility) = dot.single_mut() {
        *visibility = if dirty {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut bg) = apply.single_mut() {
        let alpha = if dirty { 1.0 } else { 0.4 };
        if bg.0.alpha() != alpha {
            bg.0.set_alpha(alpha);
        }
    }
}

/// Escape toggles the settings window in any state (title screen and in-game),
/// gated only by `ui_unfocused` so a focused text field swallows the key.
fn toggle_settings(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Query<&mut Visibility, With<SettingsWindowRoot>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_persistent::prelude::StorageFormat;
    use game_engine::domain::settings::{AntiAliasing, FpsCap};

    fn persistent_settings(slug: &str, settings: Settings) -> Persistent<Settings> {
        let path = std::env::temp_dir().join(format!(
            "lifthrasir-settings-ui-{}-{slug}.ron",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Persistent::<Settings>::builder()
            .name("settings")
            .format(StorageFormat::Ron)
            .path(path)
            .default(settings)
            .build()
            .expect("build persistent settings")
    }

    fn dirtied_ui() -> SettingsUi {
        let mut ui = SettingsUi::default();
        ui.draft.graphics.fps_cap = FpsCap::F120;
        ui
    }

    #[test]
    fn editing_the_draft_makes_it_dirty() {
        let clean = SettingsUi::default();
        assert!(!clean.dirty());

        let dirty = dirtied_ui();
        assert!(dirty.dirty());
    }

    #[test]
    fn cancel_reverts_the_draft_and_clears_listening() {
        let mut ui = dirtied_ui();
        ui.listening = Some((PlayerAction::Sit, BindSlot::Primary));

        ui.cancel();

        assert_eq!(ui.draft, ui.committed);
        assert!(!ui.dirty());
        assert!(ui.listening.is_none());
    }

    #[test]
    fn reset_sets_the_draft_to_defaults() {
        let mut ui = SettingsUi::default();
        ui.draft.graphics.antialiasing = AntiAliasing::MsaaX4;
        ui.committed.graphics.antialiasing = AntiAliasing::MsaaX4;
        assert!(!ui.dirty());

        ui.reset();

        assert_eq!(ui.draft, Settings::default());
        assert!(ui.dirty());
    }

    /// Apply persists the draft into `Persistent<Settings>`, marks it committed
    /// (dirty clears), and reports that the caller should emit `ApplySettings`.
    #[test]
    fn apply_persists_commits_and_signals_emit() {
        let mut ui = dirtied_ui();
        let mut persistent = persistent_settings("apply", Settings::default());

        let emitted = apply_draft(&mut ui, &mut persistent);

        assert!(emitted);
        assert!(!ui.dirty());
        assert_eq!(ui.committed.graphics.fps_cap, FpsCap::F120);
        assert_eq!(persistent.graphics.fps_cap, FpsCap::F120);
    }

    #[test]
    fn apply_is_a_noop_when_clean() {
        let mut ui = SettingsUi::default();
        let mut persistent = persistent_settings("clean", Settings::default());

        assert!(!apply_draft(&mut ui, &mut persistent));
        assert_eq!(persistent.graphics.fps_cap, FpsCap::F60);
    }
}
