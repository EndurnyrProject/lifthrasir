//! Settings window: the draggable shell and the draft/Apply/Cancel/Reset model.
//!
//! The chrome (titlebar, tab rail, fixed-height scrollable content pane, and
//! footer) is authored declaratively with `bsn!` in [`scene`]; Feathers supplies
//! the buttons and the scrollbar. The window edits a draft `Settings` clone held
//! in `SettingsUi`; nothing touches the live world until Apply, which persists
//! the draft and emits `ApplySettings`. The tree is static — the `refresh_*`
//! systems project the draft onto the controls via their marker components.
//! Spawned hidden at `Startup` so it survives state changes and is reachable
//! from the title screen and in-game.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use bevy_persistent::prelude::Persistent;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::settings::{
    resolution_label, resolution_next, resolution_prev, ActionBinds, ApplySettings, DisplayMode,
    GraphicsSettings, KeyBind, Modifier, Settings,
};

use crate::theme;
use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

/// Which slot of an action's bindings a rebind capture targets.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BindSlot {
    #[default]
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
#[derive(Component, Default, Clone)]
pub struct SettingsWindowRoot;

/// The draggable titlebar; the drag observer only moves the window when the
/// drag's target is the titlebar itself, so dragging from the close button is
/// inert.
#[derive(Component, Default, Clone)]
struct SettingsTitlebar;

/// Marks a tab-rail button so its observer and highlight key off a tab.
#[derive(Component, Clone, Copy, Default)]
struct TabButton(SettingsTab);

/// Marks a tab body so the active-tab system can show/hide it.
#[derive(Component, Clone, Copy, Default)]
struct TabBody(SettingsTab);

/// Marks the unsaved-changes dot so its visibility tracks `dirty()`.
#[derive(Component, Default, Clone)]
struct DirtyDot;

/// Marks the Apply button so it dims when there is nothing to apply.
#[derive(Component, Default, Clone)]
struct ApplyButton;

pub struct SettingsWindowPlugin;

impl Plugin for SettingsWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
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
                toggle_settings.run_if(
                    ui_unfocused
                        .and_then(listening_inactive)
                        .and_then(not(resource_exists::<
                            crate::widgets::npc_dialog::ActiveNpcDialog,
                        >))
                        .and_then(not(resource_exists::<
                            crate::widgets::shop_window::ShopSession,
                        >)),
                ),
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

/// Spawns the (hidden) window as a top-level BSN scene so it survives state
/// changes and renders over both the title screen and the in-game HUD.
fn spawn_settings_root(mut commands: Commands) {
    scene::build(&mut commands);
}

/// Tab click: set the active tab in the draft state.
fn on_tab_click(click: On<Pointer<Click>>, tabs: Query<&TabButton>, mut ui: ResMut<SettingsUi>) {
    let Ok(tab) = tabs.get(click.entity) else {
        return;
    };
    ui.tab = tab.0;
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
fn on_cancel(_: On<Activate>, mut ui: ResMut<SettingsUi>) {
    ui.cancel();
}

/// Reset to Defaults: load the built-in defaults into the draft.
fn on_reset(_: On<Activate>, mut ui: ResMut<SettingsUi>) {
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
/// gated by `ui_unfocused` so a focused text field swallows the key, and skipped
/// entirely while an NPC dialogue or shop is open so Escape only closes/cancels
/// that instead (see `npc_dialog::cancel_on_escape` / `shop_window::close_shop`).
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
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
enum GraphicsField {
    #[default]
    DisplayMode,
    Resolution,
    Antialiasing,
    Anisotropy,
    Upscaling,
    Dlss,
    Ssao,
    Vsync,
    Bloom,
    Shadows,
    FpsCap,
    UiScaling,
}

/// Direction a stepper arrow moves the value.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum StepDir {
    #[default]
    Prev,
    Next,
}

/// A segmented-control button: edits `field` to the variant at `index` in
/// `DisplayMode::ALL`.
#[derive(Component, Clone, Copy, Default)]
struct SegButton {
    field: GraphicsField,
    index: usize,
}

/// A stepper arrow: steps `field` one preset in `dir`.
#[derive(Component, Clone, Copy, Default)]
struct StepperArrow {
    field: GraphicsField,
    dir: StepDir,
}

/// The value text inside a stepper; `refresh_graphics` rewrites it.
#[derive(Component, Clone, Copy, Default)]
struct StepperValue(GraphicsField);

/// The clickable switch pill; flips `field`'s bool.
#[derive(Component, Clone, Copy, Default)]
struct SwitchPill(GraphicsField);

/// The sliding knob inside a switch; `refresh_graphics` repositions it.
#[derive(Component, Clone, Copy, Default)]
struct SwitchKnob(GraphicsField);

/// Reads a field's current stepper/switch display value off the draft.
fn field_label(graphics: &GraphicsSettings, field: GraphicsField) -> String {
    match field {
        GraphicsField::Resolution => resolution_label(graphics.resolution),
        GraphicsField::Antialiasing => graphics.antialiasing.label().to_string(),
        GraphicsField::Anisotropy => graphics.anisotropy.label().to_string(),
        GraphicsField::Upscaling => graphics.upscaling.label().to_string(),
        GraphicsField::Dlss => graphics.dlss.label().to_string(),
        GraphicsField::Ssao => graphics.ssao.label().to_string(),
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
        (GraphicsField::Dlss, StepDir::Next) => graphics.dlss = graphics.dlss.next(),
        (GraphicsField::Dlss, StepDir::Prev) => graphics.dlss = graphics.dlss.prev(),
        (GraphicsField::Ssao, StepDir::Next) => graphics.ssao = graphics.ssao.next(),
        (GraphicsField::Ssao, StepDir::Prev) => graphics.ssao = graphics.ssao.prev(),
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
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
enum AudioChannel {
    #[default]
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
#[derive(Component, Clone, Copy, Default)]
struct MuteButton(AudioChannel);

/// The draggable slider rail; carries `RelativeCursorPosition` so pointer events
/// map straight to a 0..1 fraction without manual geometry.
#[derive(Component, Clone, Copy, Default)]
struct SliderRail(AudioChannel);

/// The fill bar inside a rail; `refresh_sound` sets its width.
#[derive(Component, Clone, Copy, Default)]
struct SliderFill(AudioChannel);

/// The knob inside a rail; `refresh_sound` sets its left offset.
#[derive(Component, Clone, Copy, Default)]
struct SliderKnob(AudioChannel);

/// The percent (or "Muted") readout; `refresh_sound` rewrites its text.
#[derive(Component, Clone, Copy, Default)]
struct SliderPercent(AudioChannel);

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
        PlayerAction::Equipment => &keybinds.equipment,
        PlayerAction::Party => &keybinds.party,
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
        PlayerAction::Equipment => &mut keybinds.equipment,
        PlayerAction::Party => &mut keybinds.party,
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

impl Default for Keycap {
    fn default() -> Self {
        Self {
            action: PlayerAction::Sit,
            slot: BindSlot::Primary,
        }
    }
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
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
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
            equipment: ActionBinds::default(),
            cart: ActionBinds::default(),
            party: ActionBinds::default(),
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
            equipment: ActionBinds::default(),
            cart: ActionBinds::default(),
            party: ActionBinds::default(),
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
        assert_eq!(keycap_label(&binds.equipment.primary), "Alt + Q");
    }
}
