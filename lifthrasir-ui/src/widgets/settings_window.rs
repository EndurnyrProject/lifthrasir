//! Settings window: the draggable shell and the draft/Apply/Cancel/Reset model.
//!
//! The window edits a draft `Settings` clone held in `SettingsUi`; nothing
//! touches the live world until Apply, which persists the draft and emits
//! `ApplySettings`. The three tab bodies are empty placeholders here — the
//! per-tab controls land in later tasks. Spawned hidden at `Startup` so it
//! survives state changes and is reachable from the title screen and in-game.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy_persistent::prelude::Persistent;
use game_engine::domain::input::{ui_unfocused, PlayerAction, HOTBAR_ACTIONS};
use game_engine::domain::settings::{
    resolution_label, resolution_next, resolution_prev, ActionBinds, ApplySettings, DisplayMode,
    GraphicsSettings, KeyBind, Modifier, Settings,
};

use crate::theme;
use crate::widgets::draggable::make_draggable;

/// Which slot of an action's bindings a rebind capture targets.
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
                // Capture runs before the Escape toggle and consumes the press, so
                // Escape-while-listening cancels the capture instead of closing.
                capture_rebind
                    .run_if(listening_active)
                    .before(toggle_settings),
                toggle_settings.run_if(ui_unfocused.and(listening_inactive)),
                refresh_tabs.run_if(resource_changed::<SettingsUi>),
                refresh_footer.run_if(resource_changed::<SettingsUi>),
                refresh_graphics.run_if(resource_changed::<SettingsUi>),
                refresh_sound.run_if(resource_changed::<SettingsUi>),
                refresh_input.run_if(resource_changed::<SettingsUi>),
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
            // Float above every other UI root (login panel, in-game HUD) so the
            // window owns picking — otherwise a full-screen screen root spawned
            // later in the stack swallows its clicks and drags.
            GlobalZIndex(1000),
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
    // Toggle `display`, not `Visibility`: a hidden node still reserves its
    // layout slot, which would stack the tabs at different heights (Sound mid,
    // Input at the bottom). `Display::None` removes the slot so the active tab
    // always sits at the top of the content pane.
    let display = if tab == SettingsTab::default() {
        Display::Flex
    } else {
        Display::None
    };
    let body = commands
        .spawn((
            TabBody(tab),
            Node {
                display,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        theme::label(title, font.clone(), 11.0, theme::GOLD),
        ChildOf(body),
    ));

    if tab == SettingsTab::Graphics {
        spawn_graphics_rows(commands, body, font);
    }
    if tab == SettingsTab::Sound {
        spawn_sound_rows(commands, body, font);
    }
    if tab == SettingsTab::Input {
        spawn_input_rows(commands, body, font);
    }
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
    mut bodies: Query<(&mut Node, &TabBody)>,
    mut buttons: Query<(&mut BackgroundColor, &TabButton)>,
) {
    for (mut node, body) in &mut bodies {
        let display = if body.0 == ui.tab {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
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

// ── Graphics tab ──────────────────────────────────────────────────────────

/// Which `draft.graphics` field a control edits. Drives both interaction
/// (steppers/switch/segmented mutate the matching field) and the displayed
/// value refresh.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum GraphicsField {
    DisplayMode,
    Resolution,
    Antialiasing,
    Anisotropy,
    Upscaling,
    Vsync,
    Bloom,
    Shadows,
    FpsCap,
    UiScaling,
}

/// Direction a stepper arrow moves the value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StepDir {
    Prev,
    Next,
}

/// A segmented-control button: edits `field` to the variant at `index` in
/// `DisplayMode::ALL`.
#[derive(Component, Clone, Copy)]
struct SegButton {
    field: GraphicsField,
    index: usize,
}

/// A stepper arrow: steps `field` one preset in `dir`.
#[derive(Component, Clone, Copy)]
struct StepperArrow {
    field: GraphicsField,
    dir: StepDir,
}

/// The value text inside a stepper; `refresh_graphics` rewrites it.
#[derive(Component, Clone, Copy)]
struct StepperValue(GraphicsField);

/// The clickable switch pill; flips `field`'s bool.
#[derive(Component, Clone, Copy)]
struct SwitchPill(GraphicsField);

/// The sliding knob inside a switch; `refresh_graphics` repositions it.
#[derive(Component, Clone, Copy)]
struct SwitchKnob(GraphicsField);

/// Reads a field's current stepper/switch display value off the draft.
fn field_label(graphics: &GraphicsSettings, field: GraphicsField) -> String {
    match field {
        GraphicsField::Resolution => resolution_label(graphics.resolution),
        GraphicsField::Antialiasing => graphics.antialiasing.label().to_string(),
        GraphicsField::Anisotropy => graphics.anisotropy.label().to_string(),
        GraphicsField::Upscaling => graphics.upscaling.label().to_string(),
        GraphicsField::FpsCap => graphics.fps_cap.label().to_string(),
        GraphicsField::UiScaling => graphics.ui_scaling.label().to_string(),
        GraphicsField::DisplayMode
        | GraphicsField::Vsync
        | GraphicsField::Bloom
        | GraphicsField::Shadows => String::new(),
    }
}

/// Reads a bool (switch) graphics field off the draft, or `None` for non-switch
/// fields. Drives both the switch click toggle and the refresh display.
fn switch_value(graphics: &GraphicsSettings, field: GraphicsField) -> Option<bool> {
    match field {
        GraphicsField::Vsync => Some(graphics.vsync),
        GraphicsField::Bloom => Some(graphics.bloom),
        GraphicsField::Shadows => Some(graphics.shadows),
        _ => None,
    }
}

/// Flips a bool (switch) graphics field on the draft.
fn toggle_switch(graphics: &mut GraphicsSettings, field: GraphicsField) {
    match field {
        GraphicsField::Vsync => graphics.vsync = !graphics.vsync,
        GraphicsField::Bloom => graphics.bloom = !graphics.bloom,
        GraphicsField::Shadows => graphics.shadows = !graphics.shadows,
        _ => {}
    }
}

/// Steps a field's value one preset in `dir` on the draft.
fn step_field(graphics: &mut GraphicsSettings, field: GraphicsField, dir: StepDir) {
    match (field, dir) {
        (GraphicsField::Resolution, StepDir::Next) => {
            graphics.resolution = resolution_next(graphics.resolution)
        }
        (GraphicsField::Resolution, StepDir::Prev) => {
            graphics.resolution = resolution_prev(graphics.resolution)
        }
        (GraphicsField::Antialiasing, StepDir::Next) => {
            graphics.antialiasing = graphics.antialiasing.next()
        }
        (GraphicsField::Antialiasing, StepDir::Prev) => {
            graphics.antialiasing = graphics.antialiasing.prev()
        }
        (GraphicsField::Anisotropy, StepDir::Next) => {
            graphics.anisotropy = graphics.anisotropy.next()
        }
        (GraphicsField::Anisotropy, StepDir::Prev) => {
            graphics.anisotropy = graphics.anisotropy.prev()
        }
        (GraphicsField::Upscaling, StepDir::Next) => graphics.upscaling = graphics.upscaling.next(),
        (GraphicsField::Upscaling, StepDir::Prev) => graphics.upscaling = graphics.upscaling.prev(),
        (GraphicsField::FpsCap, StepDir::Next) => graphics.fps_cap = graphics.fps_cap.next(),
        (GraphicsField::FpsCap, StepDir::Prev) => graphics.fps_cap = graphics.fps_cap.prev(),
        (GraphicsField::UiScaling, StepDir::Next) => {
            graphics.ui_scaling = graphics.ui_scaling.next()
        }
        (GraphicsField::UiScaling, StepDir::Prev) => {
            graphics.ui_scaling = graphics.ui_scaling.prev()
        }
        _ => {}
    }
}

/// Builds the Graphics rows under `body`.
fn spawn_graphics_rows(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    spawn_section(commands, body, "Display", font);

    let ctrl = spawn_row(
        commands,
        body,
        "Display Mode",
        "How the game fills your screen",
        font,
    );
    spawn_segmented(commands, ctrl, GraphicsField::DisplayMode, font);

    let ctrl = spawn_row(commands, body, "Resolution", "Screen size in pixels", font);
    spawn_stepper(commands, ctrl, GraphicsField::Resolution, font);

    spawn_section(commands, body, "Quality", font);

    let ctrl = spawn_row(commands, body, "Antialiasing", "Smooths jagged edges", font);
    spawn_stepper(commands, ctrl, GraphicsField::Antialiasing, font);

    let ctrl = spawn_row(
        commands,
        body,
        "Anisotropic Filtering",
        "Sharpens ground textures at grazing angles",
        font,
    );
    spawn_stepper(commands, ctrl, GraphicsField::Anisotropy, font);

    let ctrl = spawn_row(
        commands,
        body,
        "Upscaling",
        "xBRZ sprite & texture upscaling (applies on map reload)",
        font,
    );
    spawn_stepper(commands, ctrl, GraphicsField::Upscaling, font);

    let ctrl = spawn_row(commands, body, "Bloom", "Glow around bright lights", font);
    spawn_switch(commands, ctrl, GraphicsField::Bloom);

    let ctrl = spawn_row(commands, body, "Shadows", "Sun shadow casting", font);
    spawn_switch(commands, ctrl, GraphicsField::Shadows);

    let ctrl = spawn_row(
        commands,
        body,
        "VSync",
        "Sync frames to display refresh",
        font,
    );
    spawn_switch(commands, ctrl, GraphicsField::Vsync);

    let ctrl = spawn_row(
        commands,
        body,
        "Frame Rate Cap",
        "Maximum frames per second",
        font,
    );
    spawn_stepper(commands, ctrl, GraphicsField::FpsCap, font);

    spawn_section(commands, body, "Interface", font);

    let ctrl = spawn_row(
        commands,
        body,
        "UI Scaling",
        "Scales the interface for high resolutions",
        font,
    );
    spawn_stepper(commands, ctrl, GraphicsField::UiScaling, font);
}

/// A gold uppercase section caption with a trailing hairline.
fn spawn_section(commands: &mut Commands, body: Entity, text: &str, font: &Handle<Font>) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        theme::label(text, font.clone(), 10.0, theme::GOLD),
        ChildOf(row),
    ));
    commands.spawn((
        Node {
            flex_grow: 1.0,
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(theme::GOLD_FAINT),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}

/// A setting row: a label column (title + sublabel) and a right-aligned control
/// column. Returns the control column entity to attach the control to.
fn spawn_row(
    commands: &mut Commands,
    body: Entity,
    label: &str,
    sublabel: &str,
    font: &Handle<Font>,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(18.0),
                min_height: Val::Px(46.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();

    let labels = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        theme::label(label, font.clone(), 13.0, theme::TEXT),
        ChildOf(labels),
    ));
    commands.spawn((
        theme::label(sublabel, font.clone(), 11.0, theme::TEXT_FAINT),
        ChildOf(labels),
    ));

    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id()
}

/// Segmented control for `DisplayMode`: one button per variant, active one
/// highlighted by `refresh_graphics`.
fn spawn_segmented(
    commands: &mut Commands,
    ctrl: Entity,
    field: GraphicsField,
    font: &Handle<Font>,
) {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(ctrl),
        ))
        .id();

    for (index, mode) in DisplayMode::ALL.into_iter().enumerate() {
        let button = commands
            .spawn((
                SegButton { field, index },
                Node {
                    height: Val::Px(30.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(Val::Px(13.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Pickable::default(),
                ChildOf(group),
            ))
            .id();
        commands.spawn((
            theme::label(mode.label(), font.clone(), 12.0, theme::TEXT_DIM),
            ChildOf(button),
        ));
        commands.entity(button).observe(on_segment_click);
    }
}

/// Clicking a segment sets the segmented field to the clicked variant.
fn on_segment_click(
    click: On<Pointer<Click>>,
    buttons: Query<&SegButton>,
    mut ui: ResMut<SettingsUi>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    if button.field == GraphicsField::DisplayMode {
        ui.draft.graphics.display_mode = DisplayMode::ALL[button.index];
    }
}

/// Stepper control: ◀ value ▶ over a field's presets.
fn spawn_stepper(commands: &mut Commands, ctrl: Entity, field: GraphicsField, font: &Handle<Font>) {
    let stepper = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                min_width: Val::Px(188.0),
                height: Val::Px(38.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(ctrl),
        ))
        .id();

    spawn_stepper_arrow(commands, stepper, field, StepDir::Prev, font);

    commands.spawn((
        StepperValue(field),
        theme::label("", font.clone(), 13.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            ..default()
        },
        TextLayout::new_with_justify(Justify::Center),
        ChildOf(stepper),
    ));

    spawn_stepper_arrow(commands, stepper, field, StepDir::Next, font);
}

fn spawn_stepper_arrow(
    commands: &mut Commands,
    stepper: Entity,
    field: GraphicsField,
    dir: StepDir,
    font: &Handle<Font>,
) {
    let arrow = commands
        .spawn((
            StepperArrow { field, dir },
            Node {
                width: Val::Px(38.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::default(),
            ChildOf(stepper),
        ))
        .id();
    commands.spawn((
        Text::new(if dir == StepDir::Prev { "<" } else { ">" }),
        TextFont {
            font: font.clone(),
            font_size: 14.0,
            ..default()
        },
        TextColor(theme::TEXT_DIM),
        Pickable::IGNORE,
        ChildOf(arrow),
    ));
    commands.entity(arrow).observe(on_stepper_click);
}

/// Clicking a stepper arrow steps its field one preset.
fn on_stepper_click(
    click: On<Pointer<Click>>,
    arrows: Query<&StepperArrow>,
    mut ui: ResMut<SettingsUi>,
) {
    let Ok(arrow) = arrows.get(click.entity) else {
        return;
    };
    step_field(&mut ui.draft.graphics, arrow.field, arrow.dir);
}

/// Toggle switch (VSync): a pill with a sliding knob.
fn spawn_switch(commands: &mut Commands, ctrl: Entity, field: GraphicsField) {
    let pill = commands
        .spawn((
            SwitchPill(field),
            Node {
                width: Val::Px(50.0),
                height: Val::Px(28.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::default(),
            ChildOf(ctrl),
        ))
        .id();
    commands.spawn((
        SwitchKnob(field),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(3.0),
            left: Val::Px(3.0),
            width: Val::Px(20.0),
            height: Val::Px(20.0),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(theme::TEXT_DIM),
        Pickable::IGNORE,
        ChildOf(pill),
    ));
    commands.entity(pill).observe(on_switch_click);
}

/// Clicking the switch flips its bool field.
fn on_switch_click(
    click: On<Pointer<Click>>,
    pills: Query<&SwitchPill>,
    mut ui: ResMut<SettingsUi>,
) {
    let Ok(pill) = pills.get(click.entity) else {
        return;
    };
    toggle_switch(&mut ui.draft.graphics, pill.0);
}

/// Reflects the current `draft.graphics` onto every graphics control: segmented
/// highlight, stepper value text, switch colour + knob position. Runs whenever
/// `SettingsUi` changes, so Cancel/Reset (which rewrite the draft) update the UI.
fn refresh_graphics(
    ui: Res<SettingsUi>,
    mut segments: Query<(&SegButton, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut TextColor>,
    mut values: Query<(&StepperValue, &mut Text)>,
    mut switches: Query<(&SwitchPill, &mut BackgroundColor), Without<SegButton>>,
    mut knobs: Query<(&SwitchKnob, &mut Node)>,
) {
    let graphics = &ui.draft.graphics;

    for (button, mut bg, children) in &mut segments {
        let active = button.field == GraphicsField::DisplayMode
            && DisplayMode::ALL[button.index] == graphics.display_mode;
        bg.0 = if active { theme::EMERALD } else { Color::NONE };
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                color.0 = if active {
                    theme::EMERALD_INK
                } else {
                    theme::TEXT_DIM
                };
            }
        }
    }

    for (value, mut text) in &mut values {
        let label = field_label(graphics, value.0);
        if text.0 != label {
            text.0 = label;
        }
    }

    for (pill, mut bg) in &mut switches {
        if let Some(on) = switch_value(graphics, pill.0) {
            bg.0 = if on { theme::EMERALD } else { theme::FIELD };
        }
    }

    for (knob, mut node) in &mut knobs {
        if let Some(on) = switch_value(graphics, knob.0) {
            node.left = if on { Val::Px(27.0) } else { Val::Px(3.0) };
        }
    }
}

// ── Sound tab ─────────────────────────────────────────────────────────────

/// The three audio channels, each a `draft.audio` volume + mute pair.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum AudioChannel {
    Bgm,
    Sfx,
    Ambient,
}

impl AudioChannel {
    /// Reads the channel's `(volume, muted)` off the draft audio config.
    fn read(self, audio: &game_engine::domain::settings::AudioConfig) -> (f32, bool) {
        match self {
            AudioChannel::Bgm => (audio.bgm_volume, audio.bgm_muted),
            AudioChannel::Sfx => (audio.sfx_volume, audio.sfx_muted),
            AudioChannel::Ambient => (audio.ambient_volume, audio.ambient_muted),
        }
    }

    /// Sets the channel's volume on the draft audio config.
    fn set_volume(self, audio: &mut game_engine::domain::settings::AudioConfig, volume: f32) {
        match self {
            AudioChannel::Bgm => audio.bgm_volume = volume,
            AudioChannel::Sfx => audio.sfx_volume = volume,
            AudioChannel::Ambient => audio.ambient_volume = volume,
        }
    }

    /// Flips the channel's mute on the draft audio config.
    fn toggle_muted(self, audio: &mut game_engine::domain::settings::AudioConfig) {
        match self {
            AudioChannel::Bgm => audio.bgm_muted = !audio.bgm_muted,
            AudioChannel::Sfx => audio.sfx_muted = !audio.sfx_muted,
            AudioChannel::Ambient => audio.ambient_muted = !audio.ambient_muted,
        }
    }
}

/// Maps a slider rail cursor fraction (0.0..=1.0, possibly slightly out of range
/// near the edges) to a stored volume, clamped to the valid 0.0..=1.0 range.
fn fraction_to_volume(fraction: f32) -> f32 {
    fraction.clamp(0.0, 1.0)
}

/// The slider's readout: `"Muted"` when muted, else the volume as a whole
/// percent (0.55 → `"55%"`).
fn percent_label(volume: f32, muted: bool) -> String {
    if muted {
        return "Muted".to_string();
    }
    format!("{}%", (volume.clamp(0.0, 1.0) * 100.0).round() as i32)
}

/// Fill/knob position as a percentage; collapses to 0 when muted.
fn slider_percent(volume: f32, muted: bool) -> f32 {
    if muted {
        return 0.0;
    }
    volume.clamp(0.0, 1.0) * 100.0
}

/// The clickable mute button; toggles its channel's `*_muted`.
#[derive(Component, Clone, Copy)]
struct MuteButton(AudioChannel);

/// The draggable slider rail; carries `RelativeCursorPosition` so pointer events
/// map straight to a 0..1 fraction without manual geometry.
#[derive(Component, Clone, Copy)]
struct SliderRail(AudioChannel);

/// The fill bar inside a rail; `refresh_sound` sets its width.
#[derive(Component, Clone, Copy)]
struct SliderFill(AudioChannel);

/// The knob inside a rail; `refresh_sound` sets its left offset.
#[derive(Component, Clone, Copy)]
struct SliderKnob(AudioChannel);

/// The percent (or "Muted") readout; `refresh_sound` rewrites its text.
#[derive(Component, Clone, Copy)]
struct SliderPercent(AudioChannel);

const SOUND_CHANNELS: [(AudioChannel, &str, &str); 3] = [
    (
        AudioChannel::Bgm,
        "Background Music",
        "Ambient score & themes",
    ),
    (AudioChannel::Sfx, "Sound Effects", "Hits, skills & impacts"),
    (
        AudioChannel::Ambient,
        "Ambient",
        "World, weather & footsteps",
    ),
];

/// Builds the three Sound rows under `body`.
fn spawn_sound_rows(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    spawn_section(commands, body, "Volume Mix", font);

    for (channel, label, sublabel) in SOUND_CHANNELS {
        let ctrl = spawn_row(commands, body, label, sublabel, font);
        spawn_mute_button(commands, ctrl, channel, font);
        spawn_slider(commands, ctrl, channel, font);
    }
}

/// A small square button that flips the channel's mute. `refresh_sound` tints it.
fn spawn_mute_button(
    commands: &mut Commands,
    ctrl: Entity,
    channel: AudioChannel,
    font: &Handle<Font>,
) {
    let button = commands
        .spawn((
            MuteButton(channel),
            Node {
                width: Val::Px(30.0),
                height: Val::Px(30.0),
                margin: UiRect::right(Val::Px(12.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::default(),
            ChildOf(ctrl),
        ))
        .id();
    commands.spawn((
        theme::label("M", font.clone(), 11.0, theme::TEXT_FAINT),
        ChildOf(button),
    ));
    commands.entity(button).observe(on_mute_click);
}

/// Clicking a mute button flips its channel's mute on the draft.
fn on_mute_click(
    click: On<Pointer<Click>>,
    buttons: Query<&MuteButton>,
    mut ui: ResMut<SettingsUi>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    button.0.toggle_muted(&mut ui.draft.audio);
}

/// A volume slider: a rail (track + fill + knob) and a percent readout. Click and
/// drag on the rail map the cursor's `RelativeCursorPosition` to the volume.
fn spawn_slider(commands: &mut Commands, ctrl: Entity, channel: AudioChannel, font: &Handle<Font>) {
    let rail = commands
        .spawn((
            SliderRail(channel),
            Node {
                width: Val::Px(200.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                ..default()
            },
            RelativeCursorPosition::default(),
            Pickable::default(),
            ChildOf(ctrl),
        ))
        .id();

    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(rail),
        ))
        .id();

    commands.spawn((
        SliderFill(channel),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Percent(0.0),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::EMERALD),
        Pickable::IGNORE,
        ChildOf(track),
    ));

    commands.spawn((
        SliderKnob(channel),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(3.0),
            left: Val::Percent(0.0),
            margin: UiRect::left(Val::Px(-7.0)),
            width: Val::Px(15.0),
            height: Val::Px(15.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(theme::DISPLAY_GOLD),
        BorderColor::all(theme::EMERALD_DEEP),
        Pickable::IGNORE,
        ChildOf(rail),
    ));

    commands.spawn((
        SliderPercent(channel),
        theme::label("", font.clone(), 13.0, theme::TEXT_DIM),
        Node {
            width: Val::Px(44.0),
            margin: UiRect::left(Val::Px(14.0)),
            ..default()
        },
        TextLayout::new_with_justify(Justify::Right),
        ChildOf(ctrl),
    ));

    commands
        .entity(rail)
        .observe(on_slider_press)
        .observe(on_slider_drag);
}

/// Reads a rail's cursor fraction and writes it as the channel's volume.
/// Pressing a muted channel's rail unmutes it (matching the mockup's set-on-grab).
fn set_slider_from_cursor(
    rail: Entity,
    rails: &Query<(&SliderRail, &RelativeCursorPosition)>,
    ui: &mut SettingsUi,
) {
    let Ok((rail, cursor)) = rails.get(rail) else {
        return;
    };
    let Some(normalized) = cursor.normalized else {
        return;
    };
    rail.0
        .set_volume(&mut ui.draft.audio, fraction_to_volume(normalized.x));
}

/// Pressing the rail jumps the volume to the cursor position (click-to-set).
fn on_slider_press(
    press: On<Pointer<Press>>,
    rails: Query<(&SliderRail, &RelativeCursorPosition)>,
    mut ui: ResMut<SettingsUi>,
) {
    set_slider_from_cursor(press.entity, &rails, &mut ui);
}

/// Dragging on the rail tracks the cursor (drag-to-set).
fn on_slider_drag(
    drag: On<Pointer<Drag>>,
    rails: Query<(&SliderRail, &RelativeCursorPosition)>,
    mut ui: ResMut<SettingsUi>,
) {
    set_slider_from_cursor(drag.entity, &rails, &mut ui);
}

/// Reflects `draft.audio` onto every sound control: mute-button tint, slider
/// fill width, knob position, and percent text. Runs whenever `SettingsUi`
/// changes, so Cancel/Reset re-sync the controls.
///
/// Query disjointness: `mutes` and `fills`/`knobs` mutate different component
/// types or are split by mutually-exclusive markers. `BackgroundColor` is touched
/// by `mutes` (`With<MuteButton>`) and `fills` (`With<SliderFill>`), which can
/// never co-occur on one entity, so the two `&mut BackgroundColor` queries are
/// disjoint. `Node` is mutated only by `fills` (`With<SliderFill>`) and `knobs`
/// (`With<SliderKnob>`), again disjoint markers.
fn refresh_sound(
    ui: Res<SettingsUi>,
    mut mutes: Query<(&MuteButton, &mut BackgroundColor, &mut BorderColor), With<MuteButton>>,
    mut fills: Query<(&SliderFill, &mut Node, &mut BackgroundColor), Without<MuteButton>>,
    mut knobs: Query<(&SliderKnob, &mut Node), Without<SliderFill>>,
    mut percents: Query<(&SliderPercent, &mut Text)>,
) {
    let audio = &ui.draft.audio;

    for (button, mut bg, mut border) in &mut mutes {
        let (_, muted) = button.0.read(audio);
        bg.0 = if muted { theme::BAD } else { theme::FIELD };
        *border = if muted {
            BorderColor::all(theme::BAD)
        } else {
            BorderColor::all(theme::STROKE)
        };
    }

    for (fill, mut node, mut bg) in &mut fills {
        let (volume, muted) = fill.0.read(audio);
        node.width = Val::Percent(slider_percent(volume, muted));
        bg.0 = if muted {
            theme::STROKE_STRONG
        } else {
            theme::EMERALD
        };
    }

    for (knob, mut node) in &mut knobs {
        let (volume, muted) = knob.0.read(audio);
        node.left = Val::Percent(slider_percent(volume, muted));
    }

    for (percent, mut text) in &mut percents {
        let (volume, muted) = percent.0.read(audio);
        let label = percent_label(volume, muted);
        if text.0 != label {
            text.0 = label;
        }
    }
}

// ── Input tab ─────────────────────────────────────────────────────────────

/// The rebindable non-hotbar actions in display order. The twelve hotbar slots
/// follow these rows (see `spawn_input_rows`), labelled `Hotbar F1`..`Hotbar F12`.
const ACTIONS: [(PlayerAction, &str); 4] = [
    (PlayerAction::Sit, "Sit / Stand"),
    (PlayerAction::Status, "Status Window"),
    (PlayerAction::Inventory, "Inventory"),
    (PlayerAction::Skills, "Skills Window"),
];

/// Borrows the stored binds for an action off the draft keybinds.
fn action_binds(
    keybinds: &game_engine::domain::settings::Keybinds,
    action: PlayerAction,
) -> &ActionBinds {
    match action {
        PlayerAction::Sit => &keybinds.sit,
        PlayerAction::Status => &keybinds.status,
        PlayerAction::Inventory => &keybinds.inventory,
        PlayerAction::Skills => &keybinds.skills,
        slot => &keybinds.hotbar[slot.hotbar_index().expect("hotbar action")],
    }
}

/// Mutably borrows the stored binds for an action off the draft keybinds.
fn action_binds_mut(
    keybinds: &mut game_engine::domain::settings::Keybinds,
    action: PlayerAction,
) -> &mut ActionBinds {
    match action {
        PlayerAction::Sit => &mut keybinds.sit,
        PlayerAction::Status => &mut keybinds.status,
        PlayerAction::Inventory => &mut keybinds.inventory,
        PlayerAction::Skills => &mut keybinds.skills,
        slot => &mut keybinds.hotbar[slot.hotbar_index().expect("hotbar action")],
    }
}

/// The slot of an `ActionBinds` a `BindSlot` selects.
fn slot_mut(binds: &mut ActionBinds, slot: BindSlot) -> &mut Option<KeyBind> {
    match slot {
        BindSlot::Primary => &mut binds.primary,
        BindSlot::Secondary => &mut binds.secondary,
    }
}

fn slot_ref(binds: &ActionBinds, slot: BindSlot) -> &Option<KeyBind> {
    match slot {
        BindSlot::Primary => &binds.primary,
        BindSlot::Secondary => &binds.secondary,
    }
}

/// The held modifier (Alt/Ctrl/Shift/Super) to fold into a captured chord, or
/// `None` when no modifier is down. First match wins; chords carry one modifier.
fn held_modifier(keys: &ButtonInput<KeyCode>) -> Option<Modifier> {
    let pressed = |a, b| keys.pressed(a) || keys.pressed(b);
    if pressed(KeyCode::AltLeft, KeyCode::AltRight) {
        Some(Modifier::Alt)
    } else if pressed(KeyCode::ControlLeft, KeyCode::ControlRight) {
        Some(Modifier::Control)
    } else if pressed(KeyCode::ShiftLeft, KeyCode::ShiftRight) {
        Some(Modifier::Shift)
    } else if pressed(KeyCode::SuperLeft, KeyCode::SuperRight) {
        Some(Modifier::Super)
    } else {
        None
    }
}

/// Whether a freshly-pressed key is itself only a modifier (so we wait for the
/// real key rather than binding "Alt" alone).
fn is_modifier_key(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// Builds a `KeyBind` from a captured key + held modifier. The key is stored as
/// its `KeyCode` reflect name (`format!("{:?}", key)`, e.g. "KeyA"/"Insert") —
/// the exact format `key_code_from_name` parses back when the bind is applied.
fn keybind_from_capture(modifier: Option<Modifier>, key: KeyCode) -> KeyBind {
    let name = format!("{key:?}");
    match modifier {
        Some(modifier) => KeyBind::modified(modifier, name),
        None => KeyBind::new(name),
    }
}

/// Human-readable name for a stored key. Strips the `Key`/`Digit` prefixes the
/// `KeyCode` reflect names carry ("KeyA" → "A", "Digit1" → "1"); other names
/// pass through ("Insert", "Space", "F5").
fn key_display(name: &str) -> &str {
    name.strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(name)
}

/// The keycap label for a slot: the bound key (with modifier prefix, e.g.
/// "Alt + A"), or "—" when the slot is empty.
fn keycap_label(bind: &Option<KeyBind>) -> String {
    let Some(bind) = bind else {
        return "—".to_string();
    };
    let key = key_display(&bind.key);
    match bind.modifier {
        Some(modifier) => format!("{} + {key}", modifier_label(modifier)),
        None => key.to_string(),
    }
}

/// Short label for a modifier in a chord display.
fn modifier_label(modifier: Modifier) -> &'static str {
    match modifier {
        Modifier::Alt => "Alt",
        Modifier::Control => "Ctrl",
        Modifier::Shift => "Shift",
        Modifier::Super => "Super",
    }
}

/// Whether a rebind capture is in progress (gates `capture_rebind`).
fn listening_active(ui: Res<SettingsUi>) -> bool {
    ui.listening.is_some()
}

/// Whether no rebind capture is in progress (gates `toggle_settings` so Escape
/// cancels the capture instead of closing the window).
fn listening_inactive(ui: Res<SettingsUi>) -> bool {
    ui.listening.is_none()
}

/// A clickable keycap cell; clicking it starts listening for `(action, slot)`.
#[derive(Component, Clone, Copy)]
struct Keycap {
    action: PlayerAction,
    slot: BindSlot,
}

/// Builds the three Input rows under `body`: a header, then one row per action
/// with a Primary and Secondary keycap.
fn spawn_input_rows(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    spawn_section(commands, body, "Key Bindings", font);
    spawn_bind_header(commands, body, font);

    for (action, label) in ACTIONS {
        spawn_bind_row(commands, body, action, label, font);
    }
    for (i, action) in HOTBAR_ACTIONS.into_iter().enumerate() {
        spawn_bind_row(commands, body, action, &format!("Hotbar F{}", i + 1), font);
    }
}

/// The "Action / Primary / Secondary" column header.
fn spawn_bind_header(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();
    spawn_header_cell(commands, row, "Action", 1.0, font);
    spawn_header_cell(commands, row, "Primary", 0.0, font);
    spawn_header_cell(commands, row, "Secondary", 0.0, font);
}

fn spawn_header_cell(
    commands: &mut Commands,
    row: Entity,
    text: &str,
    grow: f32,
    font: &Handle<Font>,
) {
    let cell = commands
        .spawn((
            Node {
                width: if grow == 0.0 {
                    Val::Px(112.0)
                } else {
                    Val::Auto
                },
                flex_grow: grow,
                margin: UiRect::left(Val::Px(if grow == 0.0 { 8.0 } else { 0.0 })),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        theme::label(text, font.clone(), 10.0, theme::TEXT_FAINT),
        ChildOf(cell),
    ));
}

/// One action row: action name + two keycaps.
fn spawn_bind_row(
    commands: &mut Commands,
    body: Entity,
    action: PlayerAction,
    label: &str,
    font: &Handle<Font>,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                min_height: Val::Px(38.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();

    commands.spawn((
        theme::label(label, font.clone(), 13.0, theme::TEXT),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(row),
    ));

    spawn_keycap(commands, row, action, BindSlot::Primary, font);
    spawn_keycap(commands, row, action, BindSlot::Secondary, font);
}

/// A clickable keycap cell. `refresh_input` rewrites its label; clicking it
/// starts a rebind capture for this `(action, slot)`.
fn spawn_keycap(
    commands: &mut Commands,
    row: Entity,
    action: PlayerAction,
    slot: BindSlot,
    font: &Handle<Font>,
) {
    let cap = commands
        .spawn((
            Keycap { action, slot },
            Node {
                width: Val::Px(104.0),
                height: Val::Px(30.0),
                margin: UiRect::left(Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            Pickable::default(),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        theme::label("", font.clone(), 12.0, theme::TEXT_DIM),
        ChildOf(cap),
    ));
    commands.entity(cap).observe(on_keycap_click);
}

/// Clicking a keycap arms a rebind capture for its slot.
fn on_keycap_click(click: On<Pointer<Click>>, caps: Query<&Keycap>, mut ui: ResMut<SettingsUi>) {
    let Ok(cap) = caps.get(click.entity) else {
        return;
    };
    ui.listening = Some((cap.action, cap.slot));
}

/// While listening, captures the next just-pressed key. `Escape` cancels;
/// modifier-only presses are ignored (wait for the real key); any other key is
/// folded with the held modifier into a `KeyBind` written to the draft slot.
///
/// Consumes the pressed key with `clear_just_pressed` so it neither leaks into
/// gameplay/other UI nor (for `Escape`) reaches `toggle_settings` and closes the
/// window — `listening` acts like a focus state for input.
fn capture_rebind(mut keys: ResMut<ButtonInput<KeyCode>>, mut ui: ResMut<SettingsUi>) {
    let Some((action, slot)) = ui.listening else {
        return;
    };

    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        ui.listening = None;
        return;
    }

    let Some(key) = keys
        .get_just_pressed()
        .copied()
        .find(|&key| !is_modifier_key(key))
    else {
        return;
    };

    let modifier = held_modifier(&keys);
    keys.clear_just_pressed(key);
    *slot_mut(action_binds_mut(&mut ui.draft.keybinds, action), slot) =
        Some(keybind_from_capture(modifier, key));
    ui.listening = None;
}

/// Re-renders each keycap from the draft keybinds, showing "Press a key…" on the
/// cell currently being listened to. Runs whenever `SettingsUi` changes, so a
/// completed capture, Cancel, and Reset all re-sync the cells.
fn refresh_input(
    ui: Res<SettingsUi>,
    caps: Query<(&Keycap, &Children)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (cap, children) in &caps {
        let listening = ui.listening == Some((cap.action, cap.slot));
        let label = if listening {
            "Press a key…".to_string()
        } else {
            keycap_label(slot_ref(
                action_binds(&ui.draft.keybinds, cap.action),
                cap.slot,
            ))
        };
        let color = if listening {
            theme::EMERALD
        } else {
            theme::TEXT_DIM
        };
        for child in children.iter() {
            if let Ok((mut text, mut text_color)) = texts.get_mut(child) {
                if text.0 != label {
                    text.0 = label.clone();
                }
                if text_color.0 != color {
                    text_color.0 = color;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy_persistent::prelude::StorageFormat;
    use game_engine::domain::settings::{AntiAliasing, FpsCap, Keybinds};

    /// Spawns the entire window tree (titlebar + all three tab bodies) so a
    /// malformed bundle — e.g. a duplicate `Pickable` — panics here at
    /// command-buffer apply instead of only at runtime. Guards the class of
    /// bug the draft-logic tests can't see because they never build the UI.
    #[test]
    fn spawn_settings_root_builds_the_full_tree_without_duplicate_components() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_systems(Startup, spawn_settings_root);

        app.update();

        let roots = app
            .world_mut()
            .query_filtered::<(), With<SettingsWindowRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(roots, 1);
    }

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

    #[test]
    fn cursor_fraction_maps_to_clamped_volume() {
        assert_eq!(fraction_to_volume(0.0), 0.0);
        assert_eq!(fraction_to_volume(0.55), 0.55);
        assert_eq!(fraction_to_volume(1.0), 1.0);
        assert_eq!(fraction_to_volume(-0.2), 0.0);
        assert_eq!(fraction_to_volume(1.4), 1.0);
    }

    #[test]
    fn percent_label_rounds_and_reports_muted() {
        assert_eq!(percent_label(0.55, false), "55%");
        assert_eq!(percent_label(0.0, false), "0%");
        assert_eq!(percent_label(1.0, false), "100%");
        assert_eq!(percent_label(0.854, false), "85%");
        assert_eq!(percent_label(0.7, true), "Muted");
    }

    #[test]
    fn slider_percent_collapses_to_zero_when_muted() {
        assert_eq!(slider_percent(0.55, false), 55.0);
        assert_eq!(slider_percent(0.55, true), 0.0);
        assert_eq!(slider_percent(1.0, false), 100.0);
    }

    #[test]
    fn audio_channel_reads_and_edits_the_matching_fields() {
        let mut ui = SettingsUi::default();

        AudioChannel::Sfx.set_volume(&mut ui.draft.audio, 0.25);
        assert_eq!(AudioChannel::Sfx.read(&ui.draft.audio), (0.25, false));

        AudioChannel::Sfx.toggle_muted(&mut ui.draft.audio);
        assert_eq!(AudioChannel::Sfx.read(&ui.draft.audio), (0.25, true));
        assert!(ui.dirty());
    }

    /// A plain captured key, once stored and built into an `InputMap` (which
    /// parses the stored name back via the Task-4 `key_code_from_name`), maps to
    /// the same `KeyCode` a directly-built map produces. Guards the round-trip.
    #[test]
    fn plain_capture_round_trips_through_the_input_map() {
        let binds = Keybinds {
            sit: ActionBinds {
                primary: Some(keybind_from_capture(None, KeyCode::Insert)),
                secondary: None,
            },
            status: ActionBinds::default(),
            inventory: ActionBinds::default(),
            skills: ActionBinds::default(),
            hotbar: Default::default(),
        };
        let expected = {
            let mut map = leafwing_input_manager::prelude::InputMap::default();
            map.insert(PlayerAction::Sit, KeyCode::Insert);
            map
        };
        assert_eq!(binds.to_input_map(), expected);
    }

    /// A modified chord capture round-trips to the same `ButtonlikeChord` a
    /// directly-built map produces.
    #[test]
    fn modified_capture_round_trips_through_the_input_map() {
        let binds = Keybinds {
            sit: ActionBinds::default(),
            status: ActionBinds {
                primary: Some(keybind_from_capture(Some(Modifier::Alt), KeyCode::KeyA)),
                secondary: None,
            },
            inventory: ActionBinds::default(),
            skills: ActionBinds::default(),
            hotbar: Default::default(),
        };
        let expected = {
            use leafwing_input_manager::prelude::*;
            let mut map = InputMap::default();
            map.insert(
                PlayerAction::Status,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyA),
            );
            map
        };
        assert_eq!(binds.to_input_map(), expected);
    }

    #[test]
    fn keybind_from_capture_stores_the_reflect_name() {
        assert_eq!(keybind_from_capture(None, KeyCode::KeyA).key, "KeyA");
        assert_eq!(keybind_from_capture(None, KeyCode::Insert).key, "Insert");
        let chord = keybind_from_capture(Some(Modifier::Alt), KeyCode::KeyE);
        assert_eq!(chord.key, "KeyE");
        assert_eq!(chord.modifier, Some(Modifier::Alt));
    }

    #[test]
    fn keycap_label_renders_keys_chords_and_empty() {
        assert_eq!(keycap_label(&Some(KeyBind::new("Insert"))), "Insert");
        assert_eq!(keycap_label(&Some(KeyBind::new("KeyA"))), "A");
        assert_eq!(keycap_label(&Some(KeyBind::new("Digit1"))), "1");
        assert_eq!(
            keycap_label(&Some(KeyBind::modified(Modifier::Alt, "KeyA"))),
            "Alt + A"
        );
        assert_eq!(
            keycap_label(&Some(KeyBind::modified(Modifier::Control, "KeyE"))),
            "Ctrl + E"
        );
        assert_eq!(keycap_label(&None), "—");
    }

    #[test]
    fn default_keybinds_render_the_expected_labels() {
        let binds = Keybinds::default();
        assert_eq!(keycap_label(&binds.sit.primary), "Insert");
        assert_eq!(keycap_label(&binds.sit.secondary), "Help");
        assert_eq!(keycap_label(&binds.status.primary), "Alt + A");
        assert_eq!(keycap_label(&binds.status.secondary), "—");
        assert_eq!(keycap_label(&binds.inventory.primary), "Alt + E");
    }
}
