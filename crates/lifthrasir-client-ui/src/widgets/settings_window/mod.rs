//! Settings window, after the `Settings` artboard: the sections on a left rail with a
//! search over them, the open section's rows, and the draft / Apply / Cancel / Reset
//! footer.
//!
//! The window edits a [`SettingsDraft`] held in [`SettingsUi`]; nothing touches the
//! live world until Apply, which commits the draft, queues a Bevy settings save, and
//! emits `ApplySettings`. [`view`] projects the draft into plain views of the window's
//! regions and [`rebuild`] redraws a region when its view changes. Sliders are the
//! exception: their fill, knob and readout are written in place, and the rows are not
//! redrawn while one is held, so a drag is never cut short.
//!
//! Spawned hidden at `Startup` so it survives state changes and is reachable from the
//! title screen and in-game.
//!
//! NOTE: the artboard also lists Social and Accessibility sections, more Interface
//! rows, master and interface volume and two sound behaviours. The client has no such
//! settings, so they are left out until it does.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::settings::SaveSettings;
use bevy::text::EditableText;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Activate;
use lifthrasir_client::domain::input::PlayerAction;
use lifthrasir_client::domain::settings::{
    ApplySettings, AudioConfig, GraphicsSettings, KeyBind, Keybinds, Modifier,
};

use crate::screens::shared::light_search_focus;

pub mod scene;
mod view;

#[cfg(test)]
mod tests;

pub use view::{AudioChannel, BindSlot, Clash, Preset, Setting, SettingsTab};

/// Seconds a footer toast stays up.
const TOAST_SECS: f32 = 2.4;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct SettingsDraft {
    pub graphics: GraphicsSettings,
    pub audio: AudioConfig,
    pub keybinds: Keybinds,
}

/// UI-side draft state for the settings window. The active resources are only
/// mutated when Apply commits `draft`.
#[derive(Resource)]
pub struct SettingsUi {
    pub draft: SettingsDraft,
    pub committed: SettingsDraft,
    pub tab: SettingsTab,
    pub listening: Option<(PlayerAction, BindSlot)>,
    /// The search as typed.
    query: String,
    /// A pressed key that another slot already holds, waiting for the player's call.
    clash: Option<Clash>,
    toast: Option<(String, Timer)>,
    /// Whether a slider is held. The rows are not redrawn until it is let go.
    dragging: bool,
}

impl FromWorld for SettingsUi {
    fn from_world(world: &mut World) -> Self {
        let draft = SettingsDraft {
            graphics: *world.resource::<GraphicsSettings>(),
            audio: *world.resource::<AudioConfig>(),
            keybinds: world.resource::<Keybinds>().clone(),
        };
        Self {
            committed: draft.clone(),
            draft,
            tab: default(),
            listening: None,
            query: String::new(),
            clash: None,
            toast: None,
            dragging: false,
        }
    }
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
        self.clash = None;
    }

    /// Resets the draft to the built-in defaults.
    pub fn reset(&mut self) {
        self.draft = SettingsDraft::default();
        self.listening = None;
        self.clash = None;
    }

    fn flash(&mut self, text: impl Into<String>) {
        self.toast = Some((
            text.into(),
            Timer::from_seconds(TOAST_SECS, TimerMode::Once),
        ));
    }

    /// Binds a pressed key to the slot that was listening, unless another slot holds
    /// it: then the clash is raised for the player to settle.
    fn capture(&mut self, action: PlayerAction, slot: BindSlot, bind: KeyBind) {
        self.listening = None;
        match view::holder(&self.draft.keybinds, &bind, (action, slot)) {
            Some(other) => {
                self.clash = Some(Clash {
                    action,
                    slot,
                    bind,
                    other,
                });
            }
            None => *view::bind_mut(&mut self.draft.keybinds, action, slot) = bind,
        }
    }

    /// Settles a clash by trading the two binds.
    fn swap_binds(&mut self) {
        let Some(clash) = self.clash.take() else {
            return;
        };
        let keybinds = &mut self.draft.keybinds;
        let moved = std::mem::replace(
            view::bind_mut(keybinds, clash.action, clash.slot),
            clash.bind.clone(),
        );
        *view::bind_mut(keybinds, clash.other.0, clash.other.1) = moved.clone();
        self.flash(view::swap_notice(&clash, &moved));
    }

    /// Everything an action does to the draft. Applying and closing reach outside it
    /// and are left to [`SettingsDesk`].
    fn edit(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::Section(tab) => {
                self.tab = tab;
                self.listening = None;
                self.clash = None;
            }
            SettingsAction::Choose(setting, index) => {
                setting.choose(&mut self.draft.graphics, index);
            }
            SettingsAction::Cycle(setting) => setting.cycle(&mut self.draft.graphics),
            SettingsAction::Toggle(setting) => setting.toggle(&mut self.draft.graphics),
            SettingsAction::Mute(channel) => {
                let (_, muted) = channel.read(&self.draft.audio);
                channel.set_muted(&mut self.draft.audio, !muted);
            }
            SettingsAction::Preset(preset) => preset.apply(&mut self.draft.graphics),
            SettingsAction::Listen(action, slot) => {
                self.listening = Some((action, slot));
                self.clash = None;
            }
            SettingsAction::KeepBind => self.clash = None,
            SettingsAction::SwapBinds => self.swap_binds(),
            SettingsAction::ResetSection => {
                view::reset_section(&mut self.draft, self.tab);
                self.listening = None;
                self.clash = None;
                let (title, _, _) = self.tab.meta();
                self.flash(format!("{title} reset to defaults \u{b7} Apply to keep it"));
            }
            SettingsAction::ResetAll => {
                self.reset();
                self.flash("Everything reset to defaults \u{b7} Apply to keep it");
            }
            SettingsAction::Cancel => {
                self.cancel();
                self.flash("Changes discarded");
            }
            SettingsAction::Apply | SettingsAction::Close => {}
        }
    }
}

/// Commits a dirty draft into the native resources without marking unchanged
/// groups changed.
fn commit_draft(
    ui: &mut SettingsUi,
    graphics: &mut ResMut<GraphicsSettings>,
    audio: &mut ResMut<AudioConfig>,
    keybinds: &mut ResMut<Keybinds>,
) -> bool {
    if !ui.dirty() {
        return false;
    }

    if **graphics != ui.draft.graphics {
        **graphics = ui.draft.graphics;
    }
    if **audio != ui.draft.audio {
        **audio = ui.draft.audio;
    }
    if **keybinds != ui.draft.keybinds {
        **keybinds = ui.draft.keybinds.clone();
    }

    ui.committed = ui.draft.clone();
    true
}

/// Marks the toggled window root (menu / close / login gear change its `Visibility`).
#[derive(Component, Default, Clone)]
pub struct SettingsWindowRoot;

/// The draggable title bar.
#[derive(Component, Default, Clone)]
struct SettingsTitlebar;

/// The search text over the sections.
#[derive(Component, Default, Clone)]
struct SettingsSearchField;

/// A region of the window that [`rebuild`] redraws when its view changes.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum SettingsHost {
    #[default]
    Nav,
    Head,
    Rows,
    Foot,
}

/// What a control of the window does.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsAction {
    #[default]
    Close,
    Section(SettingsTab),
    /// Picks the option at this place in the setting's list.
    Choose(Setting, usize),
    /// Steps the setting to its next option.
    Cycle(Setting),
    Toggle(Setting),
    Mute(AudioChannel),
    Preset(Preset),
    /// Waits for the key to bind to this slot.
    Listen(PlayerAction, BindSlot),
    /// Settles a clash by leaving both binds as they were.
    KeepBind,
    /// Settles a clash by trading the two binds.
    SwapBinds,
    ResetSection,
    ResetAll,
    Cancel,
    Apply,
}

/// The slider's rail: a press or a drag on it sets the channel's volume.
#[derive(Component, Clone, Copy, Default)]
struct SliderRail(AudioChannel);

/// A part of a slider that [`sync_sliders`] writes in place.
#[derive(Component, Clone, Copy, Default)]
enum SliderPart {
    #[default]
    Fill,
    Knob,
    Readout,
}

/// Which channel a [`SliderPart`] shows.
#[derive(Component, Clone, Copy, Default)]
struct SliderOf(AudioChannel);

/// Everything a settings action touches.
#[derive(SystemParam)]
struct SettingsDesk<'w, 's> {
    commands: Commands<'w, 's>,
    ui: ResMut<'w, SettingsUi>,
    graphics: ResMut<'w, GraphicsSettings>,
    audio: ResMut<'w, AudioConfig>,
    keybinds: ResMut<'w, Keybinds>,
    apply: MessageWriter<'w, ApplySettings>,
    roots: Query<'w, 's, &'static mut Visibility, With<SettingsWindowRoot>>,
}

impl SettingsDesk<'_, '_> {
    fn run(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::Close => {
                if let Ok(mut visibility) = self.roots.single_mut() {
                    *visibility = Visibility::Hidden;
                }
            }
            SettingsAction::Apply => self.apply(),
            edit => self.ui.edit(edit),
        }
    }

    /// Commits the draft, queues persistence and requests a live re-apply. Does
    /// nothing when the draft is clean.
    fn apply(&mut self) {
        let committed = commit_draft(
            &mut self.ui,
            &mut self.graphics,
            &mut self.audio,
            &mut self.keybinds,
        );
        if !committed {
            return;
        }
        self.commands.queue(SaveSettings::IfChanged);
        self.apply.write(ApplySettings);
        self.ui.flash("Settings applied");
    }
}

fn on_settings_button(
    activate: On<Activate>,
    actions: Query<&SettingsAction>,
    mut desk: SettingsDesk,
) {
    if let Ok(action) = actions.get(activate.entity) {
        desk.run(*action);
    }
}

fn on_settings_click(
    click: On<PointerClick>,
    actions: Query<&SettingsAction>,
    mut desk: SettingsDesk,
) {
    if let Ok(action) = actions.get(click.entity) {
        desk.run(*action);
    }
}

/// Sets the channel's volume from where the pointer is along the rail.
fn set_volume_from_cursor(
    rail: Entity,
    rails: &Query<(&SliderRail, &RelativeCursorPosition)>,
    ui: &mut SettingsUi,
) {
    let Ok((rail, cursor)) = rails.get(rail) else {
        return;
    };
    // `normalized` is centred on the node: -0.5 at its left edge, 0.5 at its right.
    let Some(at) = cursor.normalized else {
        return;
    };
    rail.0.set_volume(&mut ui.draft.audio, at.x + 0.5);
    ui.dragging = true;
}

fn on_slider_press(
    press: On<PointerPress>,
    rails: Query<(&SliderRail, &RelativeCursorPosition)>,
    mut ui: ResMut<SettingsUi>,
) {
    set_volume_from_cursor(press.entity, &rails, &mut ui);
}

fn on_slider_drag(
    drag: On<PointerDrag>,
    rails: Query<(&SliderRail, &RelativeCursorPosition)>,
    mut ui: ResMut<SettingsUi>,
) {
    set_volume_from_cursor(drag.entity, &rails, &mut ui);
}

/// Lets the rows redraw again once the slider is let go.
fn end_drag(mouse: Option<Res<ButtonInput<MouseButton>>>, mut ui: ResMut<SettingsUi>) {
    let held = mouse.is_some_and(|mouse| mouse.pressed(MouseButton::Left));
    if ui.dragging && !held {
        ui.dragging = false;
    }
}

/// Writes each slider's fill, knob and readout from the draft.
fn sync_sliders(
    ui: Res<SettingsUi>,
    mut parts: Query<(&SliderPart, &SliderOf, &mut Node, Option<&mut Text>)>,
) {
    if !ui.is_changed() {
        return;
    }
    for (part, of, mut node, text) in &mut parts {
        let (volume, muted) = of.0.read(&ui.draft.audio);
        let at = Val::Percent(if muted { 0.0 } else { volume * 100.0 });
        match part {
            SliderPart::Fill if node.width != at => node.width = at,
            SliderPart::Knob if node.left != at => node.left = at,
            SliderPart::Readout => {
                let label = view::percent_label(volume, muted);
                if let Some(mut text) = text
                    && text.0 != label
                {
                    text.0 = label;
                }
            }
            _ => {}
        }
    }
}

fn sync_search(
    fields: Query<&EditableText, With<SettingsSearchField>>,
    mut ui: ResMut<SettingsUi>,
) {
    let Ok(field) = fields.single() else {
        return;
    };
    let typed = field.value().to_string();
    if typed != ui.query {
        ui.query = typed;
    }
}

fn tick_toast(time: Res<Time>, mut ui: ResMut<SettingsUi>) {
    let Some((_, timer)) = ui.bypass_change_detection().toast.as_mut() else {
        return;
    };
    if timer.tick(time.delta()).is_finished() {
        ui.toast = None;
    }
}

/// The held modifier (Alt/Ctrl/Shift/Super) to fold into a captured chord, or
/// `Modifier::None` when no modifier is down. First match wins; chords carry one modifier.
fn held_modifier(keys: &ButtonInput<KeyCode>) -> Modifier {
    let pressed = |a, b| keys.pressed(a) || keys.pressed(b);
    if pressed(KeyCode::AltLeft, KeyCode::AltRight) {
        Modifier::Alt
    } else if pressed(KeyCode::ControlLeft, KeyCode::ControlRight) {
        Modifier::Control
    } else if pressed(KeyCode::ShiftLeft, KeyCode::ShiftRight) {
        Modifier::Shift
    } else if pressed(KeyCode::SuperLeft, KeyCode::SuperRight) {
        Modifier::Super
    } else {
        Modifier::None
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
/// its `KeyCode` reflect name (`format!("{:?}", key)`, e.g. "KeyA"/"Insert"),
/// the exact format `key_code_from_name` parses back when the bind is applied.
fn keybind_from_capture(modifier: Modifier, key: KeyCode) -> KeyBind {
    KeyBind::modified(modifier, format!("{key:?}"))
}

/// Whether a rebind capture is in progress (gates `capture_rebind`).
fn listening_active(ui: Res<SettingsUi>) -> bool {
    ui.listening.is_some()
}

/// While listening, captures the next just-pressed key. `Escape` cancels, Backspace
/// and Delete clear the slot, modifier-only presses are ignored (wait for the real
/// key), and any other key is folded with the held modifier into a `KeyBind`.
///
/// Consumes the pressed key with `clear_just_pressed` so it neither leaks into
/// gameplay/other UI nor (for `Escape`) reaches the escape menu and closes the
/// window: `listening` acts like a focus state for input.
fn capture_rebind(mut keys: ResMut<ButtonInput<KeyCode>>, mut ui: ResMut<SettingsUi>) {
    let Some((action, slot)) = ui.listening else {
        return;
    };

    if keys.clear_just_pressed(KeyCode::Escape) {
        ui.listening = None;
        return;
    }
    if keys.clear_just_pressed(KeyCode::Backspace) | keys.clear_just_pressed(KeyCode::Delete) {
        *view::bind_mut(&mut ui.draft.keybinds, action, slot) = KeyBind::UNBOUND;
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
    ui.capture(action, slot, keybind_from_capture(modifier, key));
}

/// The view a region was last drawn from.
#[derive(Clone, PartialEq)]
enum Drawn {
    Nav(Vec<view::NavItem>),
    Head(view::HeadView),
    Rows(Vec<view::Row>),
    Foot(view::FootView),
}

impl Drawn {
    fn of(host: SettingsHost, ui: &SettingsUi) -> Self {
        match host {
            SettingsHost::Nav => Self::Nav(view::nav(ui)),
            SettingsHost::Head => Self::Head(view::head(ui)),
            SettingsHost::Rows => Self::Rows(view::rows(ui)),
            SettingsHost::Foot => Self::Foot(view::foot(ui)),
        }
    }
}

/// Redraws each region whose view changed. The rows wait while a slider is held.
fn rebuild(
    mut commands: Commands,
    ui: Res<SettingsUi>,
    hosts: Query<(Entity, &SettingsHost, Option<&Children>)>,
    mounted: Query<(), Added<SettingsHost>>,
    mut drawn: Local<[Option<Drawn>; 4]>,
) {
    if !ui.is_changed() && mounted.is_empty() {
        return;
    }
    for (entity, host, children) in &hosts {
        if *host == SettingsHost::Rows && ui.dragging {
            continue;
        }
        let view = Drawn::of(*host, &ui);
        let slot = &mut drawn[*host as usize];
        if slot.as_ref() == Some(&view) && children.is_some() {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        match view.clone() {
            Drawn::Nav(items) => scene::spawn_nav(&mut commands, entity, items),
            Drawn::Head(head) => scene::spawn_head(&mut commands, entity, head),
            Drawn::Rows(rows) => scene::spawn_rows(&mut commands, entity, rows, &ui.draft.audio),
            Drawn::Foot(foot) => scene::spawn_foot(&mut commands, entity, foot),
        }
        *slot = Some(view);
    }
}

pub struct SettingsWindowPlugin;

impl Plugin for SettingsWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsUi>();
        app.add_systems(Startup, spawn_settings_root);
        app.add_systems(
            Update,
            (
                capture_rebind.run_if(listening_active),
                sync_search,
                end_drag,
                tick_toast,
                rebuild,
                sync_sliders,
                light_search_focus::<SettingsSearchField>,
            )
                .chain(),
        );
    }
}

/// Spawns the (hidden) window as a top-level BSN scene so it survives state
/// changes and renders over both the title screen and the in-game HUD.
fn spawn_settings_root(mut commands: Commands) {
    scene::build(&mut commands);
}
