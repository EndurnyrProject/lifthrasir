//! What the Settings window shows, as plain data: the settings it lists and how each
//! reads and edits the draft, the quality presets, the key binds, and the view of every
//! region the window redraws.

use lifthrasir_assets::settings::Upscaling;
use lifthrasir_client::domain::input::{HOTBAR_ACTIONS, PlayerAction};
use lifthrasir_client::domain::settings::resources::{DlssMode, Ssao};
use lifthrasir_client::domain::settings::{
    ActionBinds, Anisotropy, AntiAliasing, AudioConfig, DisplayMode, FpsCap, GraphicsSettings,
    KeyBind, Keybinds, RESOLUTIONS, UiScaling, resolution_label,
};

use super::{SettingsDraft, SettingsUi};
use crate::screens::shared::keycap_label;

/// The sections of the left rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Sound,
    Interface,
    Controls,
}

impl SettingsTab {
    pub const ALL: [Self; 4] = [Self::Graphics, Self::Sound, Self::Interface, Self::Controls];

    /// The section's name, the line under it and its `ui/icons` glyph.
    pub(super) fn meta(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Graphics => ("Graphics", "How the world looks", "eye"),
            Self::Sound => ("Sound", "Music, effects and the world", "mega"),
            Self::Interface => ("Interface", "Windows and text", "grid"),
            Self::Controls => ("Controls", "Keys for windows and actions", "wrench"),
        }
    }
}

/// The audio channels, each a volume and a mute.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AudioChannel {
    #[default]
    Bgm,
    Sfx,
    Ambient,
}

impl AudioChannel {
    /// The channel's `(volume, muted)`.
    pub(super) fn read(self, audio: &AudioConfig) -> (f32, bool) {
        match self {
            Self::Bgm => (audio.bgm_volume, audio.bgm_muted),
            Self::Sfx => (audio.sfx_volume, audio.sfx_muted),
            Self::Ambient => (audio.ambient_volume, audio.ambient_muted),
        }
    }

    pub(super) fn set_volume(self, audio: &mut AudioConfig, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        match self {
            Self::Bgm => audio.bgm_volume = volume,
            Self::Sfx => audio.sfx_volume = volume,
            Self::Ambient => audio.ambient_volume = volume,
        }
    }

    pub(super) fn set_muted(self, audio: &mut AudioConfig, muted: bool) {
        match self {
            Self::Bgm => audio.bgm_muted = muted,
            Self::Sfx => audio.sfx_muted = muted,
            Self::Ambient => audio.ambient_muted = muted,
        }
    }
}

/// The slider's readout: `"Muted"` when muted, else the volume as a whole percent.
pub(super) fn percent_label(volume: f32, muted: bool) -> String {
    if muted {
        return "Muted".to_string();
    }
    format!("{}%", (volume.clamp(0.0, 1.0) * 100.0).round() as i32)
}

/// How a setting is edited.
#[derive(Clone, PartialEq, Debug)]
pub enum ControlView {
    /// A box showing the current option, stepping to the next on a click.
    Cycle(String),
    Segments {
        options: Vec<&'static str>,
        selected: usize,
    },
    Toggle(bool),
    /// A volume slider. The volume is written in place, so a drag is never redrawn.
    Slider {
        channel: AudioChannel,
        muted: bool,
    },
}

/// A setting the window lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Setting {
    #[default]
    DisplayMode,
    Resolution,
    Vsync,
    FpsCap,
    Antialiasing,
    Anisotropy,
    Upscaling,
    Dlss,
    Ssao,
    Bloom,
    Shadows,
    Volume(AudioChannel),
    UiScaling,
    ShowNames,
}

impl Setting {
    /// Every setting in the order its section lists it.
    const ALL: [Self; 16] = [
        Self::DisplayMode,
        Self::Resolution,
        Self::Vsync,
        Self::FpsCap,
        Self::Antialiasing,
        Self::Anisotropy,
        Self::Upscaling,
        Self::Dlss,
        Self::Ssao,
        Self::Bloom,
        Self::Shadows,
        Self::Volume(AudioChannel::Bgm),
        Self::Volume(AudioChannel::Sfx),
        Self::Volume(AudioChannel::Ambient),
        Self::UiScaling,
        Self::ShowNames,
    ];

    /// The settings this build offers: DLSS only where it is compiled in.
    fn offered() -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(|setting| *setting != Self::Dlss || cfg!(feature = "dlss"))
    }

    /// The section, the group heading in it, the name and what it does.
    fn meta(self) -> (SettingsTab, &'static str, &'static str, &'static str) {
        use SettingsTab::{Graphics, Interface, Sound};
        match self {
            Self::DisplayMode => (
                Graphics,
                "DISPLAY",
                "Display Mode",
                "How the game fills your screen",
            ),
            Self::Resolution => (Graphics, "DISPLAY", "Resolution", "Screen size in pixels"),
            Self::Vsync => (
                Graphics,
                "DISPLAY",
                "VSync",
                "Sync frames to your display to stop tearing",
            ),
            Self::FpsCap => (
                Graphics,
                "DISPLAY",
                "Frame Rate Cap",
                "Maximum frames per second",
            ),
            Self::Antialiasing => (Graphics, "QUALITY", "Antialiasing", "Smooths jagged edges"),
            Self::Anisotropy => (
                Graphics,
                "QUALITY",
                "Anisotropic Filtering",
                "Sharpens ground textures at grazing angles",
            ),
            Self::Upscaling => (
                Graphics,
                "QUALITY",
                "Upscaling",
                "xBRZ smoothing of sprites and textures",
            ),
            Self::Dlss => (
                Graphics,
                "QUALITY",
                "DLSS",
                "NVIDIA render-resolution upscaling (RTX only)",
            ),
            Self::Ssao => (
                Graphics,
                "QUALITY",
                "Ambient Occlusion",
                "Soft shadows where surfaces meet. Turns MSAA off",
            ),
            Self::Bloom => (Graphics, "QUALITY", "Bloom", "Glow around bright lights"),
            Self::Shadows => (Graphics, "QUALITY", "Shadows", "Sun shadow casting"),
            Self::Volume(AudioChannel::Bgm) => (
                Sound,
                "VOLUME",
                "Background Music",
                "Ambient score and town themes",
            ),
            Self::Volume(AudioChannel::Sfx) => {
                (Sound, "VOLUME", "Sound Effects", "Hits, skills and impacts")
            }
            Self::Volume(AudioChannel::Ambient) => {
                (Sound, "VOLUME", "Ambient", "World, weather and footsteps")
            }
            Self::UiScaling => (
                Interface,
                "WINDOWS",
                "UI Scale",
                "Makes every window and font bigger or smaller",
            ),
            Self::ShowNames => (
                Interface,
                "IN THE WORLD",
                "Show All Names",
                "Names every hero and monster without pointing at them",
            ),
        }
    }

    /// The labels of a setting picked from a list. Empty for switches and sliders.
    fn options(self) -> Vec<String> {
        fn labels<T: Copy>(all: &[T], label: fn(T) -> &'static str) -> Vec<String> {
            all.iter().map(|value| label(*value).to_string()).collect()
        }
        match self {
            Self::DisplayMode => labels(&DisplayMode::ALL, DisplayMode::label),
            Self::Resolution => RESOLUTIONS.iter().map(|r| resolution_label(*r)).collect(),
            Self::FpsCap => labels(&FpsCap::ALL, FpsCap::label),
            Self::Antialiasing => labels(&AntiAliasing::ALL, AntiAliasing::label),
            Self::Anisotropy => labels(&Anisotropy::ALL, Anisotropy::label),
            Self::Upscaling => labels(&Upscaling::ALL, Upscaling::label),
            Self::Dlss => labels(&DlssMode::ALL, DlssMode::label),
            Self::Ssao => labels(&Ssao::ALL, Ssao::label),
            Self::UiScaling => labels(&UiScaling::ALL, UiScaling::label),
            Self::Vsync | Self::Bloom | Self::Shadows | Self::ShowNames | Self::Volume(_) => {
                Vec::new()
            }
        }
    }

    /// The place of the current option in [`Self::options`].
    fn selected(self, graphics: &GraphicsSettings) -> usize {
        fn place<T: PartialEq>(all: &[T], value: T) -> usize {
            all.iter().position(|option| *option == value).unwrap_or(0)
        }
        match self {
            Self::DisplayMode => place(&DisplayMode::ALL, graphics.display_mode),
            Self::Resolution => place(&RESOLUTIONS, graphics.resolution),
            Self::FpsCap => place(&FpsCap::ALL, graphics.fps_cap),
            Self::Antialiasing => place(&AntiAliasing::ALL, graphics.antialiasing),
            Self::Anisotropy => place(&Anisotropy::ALL, graphics.anisotropy),
            Self::Upscaling => place(&Upscaling::ALL, graphics.upscaling),
            Self::Dlss => place(&DlssMode::ALL, graphics.dlss),
            Self::Ssao => place(&Ssao::ALL, graphics.ssao),
            Self::UiScaling => place(&UiScaling::ALL, graphics.ui_scaling),
            Self::Vsync | Self::Bloom | Self::Shadows | Self::ShowNames | Self::Volume(_) => 0,
        }
    }

    /// Picks the option at `index` of [`Self::options`]. An index past the list is
    /// ignored.
    pub(super) fn choose(self, graphics: &mut GraphicsSettings, index: usize) {
        fn pick<T: Copy>(all: &[T], index: usize, slot: &mut T) {
            if let Some(value) = all.get(index) {
                *slot = *value;
            }
        }
        match self {
            Self::DisplayMode => pick(&DisplayMode::ALL, index, &mut graphics.display_mode),
            Self::Resolution => pick(&RESOLUTIONS, index, &mut graphics.resolution),
            Self::FpsCap => pick(&FpsCap::ALL, index, &mut graphics.fps_cap),
            Self::Antialiasing => pick(&AntiAliasing::ALL, index, &mut graphics.antialiasing),
            Self::Anisotropy => pick(&Anisotropy::ALL, index, &mut graphics.anisotropy),
            Self::Upscaling => pick(&Upscaling::ALL, index, &mut graphics.upscaling),
            Self::Dlss => pick(&DlssMode::ALL, index, &mut graphics.dlss),
            Self::Ssao => pick(&Ssao::ALL, index, &mut graphics.ssao),
            Self::UiScaling => pick(&UiScaling::ALL, index, &mut graphics.ui_scaling),
            Self::Vsync | Self::Bloom | Self::Shadows | Self::ShowNames | Self::Volume(_) => {}
        }
    }

    /// Steps a listed setting to its next option, wrapping after the last.
    pub(super) fn cycle(self, graphics: &mut GraphicsSettings) {
        let count = self.options().len();
        if count > 0 {
            self.choose(graphics, (self.selected(graphics) + 1) % count);
        }
    }

    fn switch(self, graphics: &GraphicsSettings) -> Option<bool> {
        match self {
            Self::Vsync => Some(graphics.vsync),
            Self::Bloom => Some(graphics.bloom),
            Self::Shadows => Some(graphics.shadows),
            Self::ShowNames => Some(graphics.show_names),
            _ => None,
        }
    }

    /// Flips a switch. Other settings are left alone.
    pub(super) fn toggle(self, graphics: &mut GraphicsSettings) {
        match self {
            Self::Vsync => graphics.vsync = !graphics.vsync,
            Self::Bloom => graphics.bloom = !graphics.bloom,
            Self::Shadows => graphics.shadows = !graphics.shadows,
            Self::ShowNames => graphics.show_names = !graphics.show_names,
            _ => {}
        }
    }

    /// Copies this setting's value from one draft into another.
    fn copy(self, from: &SettingsDraft, to: &mut SettingsDraft) {
        let (source, graphics) = (&from.graphics, &mut to.graphics);
        match self {
            Self::DisplayMode => graphics.display_mode = source.display_mode,
            Self::Resolution => graphics.resolution = source.resolution,
            Self::Vsync => graphics.vsync = source.vsync,
            Self::FpsCap => graphics.fps_cap = source.fps_cap,
            Self::Antialiasing => graphics.antialiasing = source.antialiasing,
            Self::Anisotropy => graphics.anisotropy = source.anisotropy,
            Self::Upscaling => graphics.upscaling = source.upscaling,
            Self::Dlss => graphics.dlss = source.dlss,
            Self::Ssao => graphics.ssao = source.ssao,
            Self::Bloom => graphics.bloom = source.bloom,
            Self::Shadows => graphics.shadows = source.shadows,
            Self::UiScaling => graphics.ui_scaling = source.ui_scaling,
            Self::ShowNames => graphics.show_names = source.show_names,
            Self::Volume(channel) => {
                let (volume, muted) = channel.read(&from.audio);
                channel.set_volume(&mut to.audio, volume);
                channel.set_muted(&mut to.audio, muted);
            }
        }
    }

    /// Whether two drafts agree on this setting.
    fn same(self, a: &SettingsDraft, b: &SettingsDraft) -> bool {
        let mut merged = a.clone();
        self.copy(b, &mut merged);
        merged == *a
    }

    fn control(self, draft: &SettingsDraft) -> ControlView {
        if let Self::Volume(channel) = self {
            let (_, muted) = channel.read(&draft.audio);
            return ControlView::Slider { channel, muted };
        }
        if let Some(on) = self.switch(&draft.graphics) {
            return ControlView::Toggle(on);
        }
        let selected = self.selected(&draft.graphics);
        let options = self.options();
        // Long lists and long names do not fit a row of segments.
        if matches!(self, Self::DisplayMode | Self::Resolution | Self::Dlss) {
            return ControlView::Cycle(options[selected].clone());
        }
        ControlView::Segments {
            options: self.segment_labels(),
            selected,
        }
    }

    /// The labels of a segmented setting, shortened to fit their segments.
    fn segment_labels(self) -> Vec<&'static str> {
        match self {
            Self::FpsCap => FpsCap::ALL
                .iter()
                .map(|cap| match cap {
                    FpsCap::Unlimited => "None",
                    cap => cap.label(),
                })
                .collect(),
            Self::Antialiasing => AntiAliasing::ALL.iter().map(|v| v.label()).collect(),
            Self::Anisotropy => Anisotropy::ALL.iter().map(|v| v.label()).collect(),
            Self::Upscaling => Upscaling::ALL.iter().map(|v| v.label()).collect(),
            Self::Ssao => Ssao::ALL.iter().map(|v| v.label()).collect(),
            Self::UiScaling => UiScaling::ALL.iter().map(|v| v.label()).collect(),
            _ => Vec::new(),
        }
    }
}

/// A set of quality settings picked in one click.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Preset {
    Low,
    Medium,
    #[default]
    High,
    Ultra,
}

impl Preset {
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }

    /// Antialiasing, anisotropic filtering, ambient occlusion, bloom and shadows.
    fn values(self) -> (AntiAliasing, Anisotropy, Ssao, bool, bool) {
        match self {
            Self::Low => (AntiAliasing::Fxaa, Anisotropy::Off, Ssao::Off, false, false),
            Self::Medium => (AntiAliasing::Fxaa, Anisotropy::X4, Ssao::Low, true, true),
            Self::High => (AntiAliasing::Taa, Anisotropy::X8, Ssao::Medium, true, true),
            Self::Ultra => (AntiAliasing::Taa, Anisotropy::X16, Ssao::High, true, true),
        }
    }

    fn read(graphics: &GraphicsSettings) -> (AntiAliasing, Anisotropy, Ssao, bool, bool) {
        (
            graphics.antialiasing,
            graphics.anisotropy,
            graphics.ssao,
            graphics.bloom,
            graphics.shadows,
        )
    }

    pub(super) fn apply(self, graphics: &mut GraphicsSettings) {
        (
            graphics.antialiasing,
            graphics.anisotropy,
            graphics.ssao,
            graphics.bloom,
            graphics.shadows,
        ) = self.values();
    }

    /// The preset the quality settings match, if any.
    pub(super) fn of(graphics: &GraphicsSettings) -> Option<Self> {
        let current = Self::read(graphics);
        Self::ALL
            .into_iter()
            .find(|preset| preset.values() == current)
    }
}

/// Which of an action's two binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BindSlot {
    #[default]
    Primary,
    Secondary,
}

impl BindSlot {
    const BOTH: [Self; 2] = [Self::Primary, Self::Secondary];
}

/// The actions that are not hotbar slots, under their group headings.
const BIND_GROUPS: [(&str, &[(PlayerAction, &str)]); 2] = [
    (
        "WINDOWS",
        &[
            (PlayerAction::Status, "Status"),
            (PlayerAction::Inventory, "Inventory"),
            (PlayerAction::Equipment, "Equipment"),
            (PlayerAction::Skills, "Skills"),
            (PlayerAction::Party, "Party"),
            (PlayerAction::Guild, "Guild"),
            (PlayerAction::Cart, "Pushcart"),
            (PlayerAction::ChatRoom, "New Chat Room"),
            (PlayerAction::QuestLedger, "Quest Ledger"),
            (PlayerAction::Homunculus, "Homunculus"),
        ],
    ),
    (
        "ACTIONS",
        &[
            (PlayerAction::Sit, "Sit / Stand"),
            (PlayerAction::Emote, "Emotes"),
            (PlayerAction::Names, "Show All Names"),
            (PlayerAction::HomunculusFollow, "Homunculus: Follow"),
            (PlayerAction::HomunculusAttack, "Homunculus: Attack"),
            (PlayerAction::HomunculusStandby, "Homunculus: Stay"),
            (PlayerAction::HomunculusFeed, "Homunculus: Feed"),
        ],
    ),
];

/// Every action that can be bound: its group heading, the action and its name.
fn bindable() -> Vec<(&'static str, PlayerAction, String)> {
    let named = BIND_GROUPS.into_iter().flat_map(|(group, actions)| {
        actions
            .iter()
            .map(move |(action, label)| (group, *action, label.to_string()))
    });
    let hotbar = HOTBAR_ACTIONS
        .into_iter()
        .enumerate()
        .map(|(slot, action)| ("HOTBAR", action, format!("Hotbar {}", slot + 1)));
    named.chain(hotbar).collect()
}

fn action_label(action: PlayerAction) -> String {
    bindable()
        .into_iter()
        .find(|(_, bound, _)| *bound == action)
        .map_or_else(String::new, |(_, _, label)| label)
}

fn binds(keybinds: &Keybinds, action: PlayerAction) -> &ActionBinds {
    match action {
        PlayerAction::Sit => &keybinds.sit,
        PlayerAction::Status => &keybinds.status,
        PlayerAction::Inventory => &keybinds.inventory,
        PlayerAction::Skills => &keybinds.skills,
        PlayerAction::Equipment => &keybinds.equipment,
        PlayerAction::Cart => &keybinds.cart,
        PlayerAction::Party => &keybinds.party,
        PlayerAction::Guild => &keybinds.guild,
        PlayerAction::Emote => &keybinds.emote,
        PlayerAction::Names => &keybinds.names,
        PlayerAction::ChatRoom => &keybinds.chat_room,
        PlayerAction::QuestLedger => &keybinds.quest_ledger,
        PlayerAction::Homunculus => &keybinds.homunculus,
        PlayerAction::HomunculusFollow => &keybinds.homunculus_follow,
        PlayerAction::HomunculusAttack => &keybinds.homunculus_attack,
        PlayerAction::HomunculusStandby => &keybinds.homunculus_standby,
        PlayerAction::HomunculusFeed => &keybinds.homunculus_feed,
        slot => &keybinds.hotbar[slot.hotbar_index().expect("hotbar action")],
    }
}

fn binds_mut(keybinds: &mut Keybinds, action: PlayerAction) -> &mut ActionBinds {
    match action {
        PlayerAction::Sit => &mut keybinds.sit,
        PlayerAction::Status => &mut keybinds.status,
        PlayerAction::Inventory => &mut keybinds.inventory,
        PlayerAction::Skills => &mut keybinds.skills,
        PlayerAction::Equipment => &mut keybinds.equipment,
        PlayerAction::Cart => &mut keybinds.cart,
        PlayerAction::Party => &mut keybinds.party,
        PlayerAction::Guild => &mut keybinds.guild,
        PlayerAction::Emote => &mut keybinds.emote,
        PlayerAction::Names => &mut keybinds.names,
        PlayerAction::ChatRoom => &mut keybinds.chat_room,
        PlayerAction::QuestLedger => &mut keybinds.quest_ledger,
        PlayerAction::Homunculus => &mut keybinds.homunculus,
        PlayerAction::HomunculusFollow => &mut keybinds.homunculus_follow,
        PlayerAction::HomunculusAttack => &mut keybinds.homunculus_attack,
        PlayerAction::HomunculusStandby => &mut keybinds.homunculus_standby,
        PlayerAction::HomunculusFeed => &mut keybinds.homunculus_feed,
        slot => &mut keybinds.hotbar[slot.hotbar_index().expect("hotbar action")],
    }
}

pub(super) fn bind(keybinds: &Keybinds, action: PlayerAction, slot: BindSlot) -> &KeyBind {
    let binds = binds(keybinds, action);
    match slot {
        BindSlot::Primary => &binds.primary,
        BindSlot::Secondary => &binds.secondary,
    }
}

pub(super) fn bind_mut(
    keybinds: &mut Keybinds,
    action: PlayerAction,
    slot: BindSlot,
) -> &mut KeyBind {
    let binds = binds_mut(keybinds, action);
    match slot {
        BindSlot::Primary => &mut binds.primary,
        BindSlot::Secondary => &mut binds.secondary,
    }
}

/// The slot other than `except` that already holds `wanted`.
pub(super) fn holder(
    keybinds: &Keybinds,
    wanted: &KeyBind,
    except: (PlayerAction, BindSlot),
) -> Option<(PlayerAction, BindSlot)> {
    bindable()
        .into_iter()
        .flat_map(|(_, action, _)| BindSlot::BOTH.map(|slot| (action, slot)))
        .find(|&(action, slot)| (action, slot) != except && bind(keybinds, action, slot) == wanted)
}

/// A key the player pressed for one slot while another slot already holds it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Clash {
    pub action: PlayerAction,
    pub slot: BindSlot,
    pub bind: KeyBind,
    pub other: (PlayerAction, BindSlot),
}

/// Resets every setting of `tab` to its default, or every key bind for Controls.
pub(super) fn reset_section(draft: &mut SettingsDraft, tab: SettingsTab) {
    let defaults = SettingsDraft::default();
    if tab == SettingsTab::Controls {
        draft.keybinds = defaults.keybinds;
        return;
    }
    for setting in Setting::ALL.into_iter().filter(|s| s.meta().0 == tab) {
        setting.copy(&defaults, draft);
    }
}

/// Whether `tab` holds anything that is not at its default.
fn off_default(draft: &SettingsDraft, tab: SettingsTab) -> bool {
    let defaults = SettingsDraft::default();
    if tab == SettingsTab::Controls {
        return draft.keybinds != defaults.keybinds;
    }
    Setting::ALL
        .into_iter()
        .any(|setting| setting.meta().0 == tab && !setting.same(draft, &defaults))
}

fn bind_changed(ui: &SettingsUi, action: PlayerAction) -> bool {
    binds(&ui.draft.keybinds, action) != binds(&ui.committed.keybinds, action)
}

/// How many settings and actions differ from what was last applied.
pub(super) fn pending(ui: &SettingsUi) -> usize {
    let settings = Setting::ALL
        .into_iter()
        .filter(|setting| !setting.same(&ui.draft, &ui.committed))
        .count();
    let actions = bindable()
        .into_iter()
        .filter(|(_, action, _)| bind_changed(ui, *action))
        .count();
    settings + actions
}

fn changed_in(ui: &SettingsUi, tab: SettingsTab) -> bool {
    if tab == SettingsTab::Controls {
        return ui.draft.keybinds != ui.committed.keybinds;
    }
    Setting::ALL
        .into_iter()
        .any(|setting| setting.meta().0 == tab && !setting.same(&ui.draft, &ui.committed))
}

/// The typed search, trimmed and lower-cased.
fn needle(query: &str) -> String {
    query.trim().to_lowercase()
}

fn matches(needle: &str, text: &str) -> bool {
    needle.is_empty() || text.to_lowercase().contains(needle)
}

fn setting_matches(needle: &str, setting: Setting) -> bool {
    let (_, _, label, desc) = setting.meta();
    matches(needle, &format!("{label} {desc}"))
}

/// A section of the left rail.
#[derive(Clone, PartialEq, Debug)]
pub struct NavItem {
    pub tab: SettingsTab,
    pub active: bool,
    /// How many of its rows match the search, while there is one.
    pub hits: Option<usize>,
    pub changed: bool,
}

pub(super) fn nav(ui: &SettingsUi) -> Vec<NavItem> {
    let needle = needle(&ui.query);
    SettingsTab::ALL
        .into_iter()
        .map(|tab| {
            let hits = if tab == SettingsTab::Controls {
                bindable()
                    .iter()
                    .filter(|(_, _, label)| matches(&needle, label))
                    .count()
            } else {
                Setting::offered()
                    .filter(|s| s.meta().0 == tab && setting_matches(&needle, *s))
                    .count()
            };
            NavItem {
                tab,
                active: ui.tab == tab,
                hits: (!needle.is_empty()).then_some(hits),
                changed: changed_in(ui, tab),
            }
        })
        .collect()
}

/// The heading of the open section.
#[derive(Clone, PartialEq, Debug)]
pub struct HeadView {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub can_reset: bool,
}

pub(super) fn head(ui: &SettingsUi) -> HeadView {
    let (title, subtitle, _) = ui.tab.meta();
    HeadView {
        title,
        subtitle,
        can_reset: off_default(&ui.draft, ui.tab),
    }
}

/// A setting's row.
#[derive(Clone, PartialEq, Debug)]
pub struct SettingRow {
    pub setting: Setting,
    pub label: &'static str,
    pub desc: &'static str,
    pub changed: bool,
    /// A note beside the name, shown while the change is pending.
    pub tag: Option<&'static str>,
    pub control: ControlView,
}

/// How a key cap is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyState {
    Bound,
    Empty,
    Listening,
    Clash,
}

/// An action's row: its name and its two key caps.
#[derive(Clone, PartialEq, Debug)]
pub struct BindRow {
    pub action: PlayerAction,
    pub label: String,
    pub changed: bool,
    pub keys: [(String, KeyState); 2],
}

/// A row of the open section.
#[derive(Clone, PartialEq, Debug)]
pub enum Row {
    /// The quality presets, lit on the one the settings match.
    Presets(Option<Preset>),
    Group(&'static str),
    /// `lit` marks a row found by the search.
    Setting {
        row: SettingRow,
        lit: bool,
    },
    BindHeader,
    /// The pressed key, and the action that already holds it.
    Clash {
        key: String,
        other: String,
    },
    Bind {
        row: BindRow,
        lit: bool,
    },
    /// How rebinding works.
    BindHint,
    /// Nothing matches this search.
    Empty(String),
}

fn setting_rows(ui: &SettingsUi, needle: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    if ui.tab == SettingsTab::Graphics && needle.is_empty() {
        rows.push(Row::Presets(Preset::of(&ui.draft.graphics)));
    }
    let mut last_group = "";
    for setting in Setting::offered() {
        let (tab, group, label, desc) = setting.meta();
        if tab != ui.tab || !setting_matches(needle, setting) {
            continue;
        }
        if group != last_group {
            rows.push(Row::Group(group));
            last_group = group;
        }
        let changed = !setting.same(&ui.draft, &ui.committed);
        let tag = (changed && setting == Setting::Upscaling).then_some("Applies on map reload");
        rows.push(Row::Setting {
            row: SettingRow {
                setting,
                label,
                desc,
                changed,
                tag,
                control: setting.control(&ui.draft),
            },
            lit: !needle.is_empty(),
        });
    }
    rows
}

fn key_view(ui: &SettingsUi, action: PlayerAction, slot: BindSlot) -> (String, KeyState) {
    if ui.listening == Some((action, slot)) {
        return ("Press a key\u{2026}".to_string(), KeyState::Listening);
    }
    if let Some(clash) = ui
        .clash
        .as_ref()
        .filter(|c| (c.action, c.slot) == (action, slot))
    {
        return (keycap_label(&clash.bind), KeyState::Clash);
    }
    let bind = bind(&ui.draft.keybinds, action, slot);
    if bind.is_bound() {
        (keycap_label(bind), KeyState::Bound)
    } else {
        ("Not set".to_string(), KeyState::Empty)
    }
}

fn bind_rows(ui: &SettingsUi, needle: &str) -> Vec<Row> {
    let found: Vec<_> = bindable()
        .into_iter()
        .filter(|(_, _, label)| matches(needle, label))
        .collect();
    if found.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![Row::BindHeader];
    if let Some(clash) = &ui.clash {
        rows.push(Row::Clash {
            key: keycap_label(&clash.bind),
            other: action_label(clash.other.0),
        });
    }
    let mut last_group = "";
    for (group, action, label) in found {
        if group != last_group {
            rows.push(Row::Group(group));
            last_group = group;
        }
        rows.push(Row::Bind {
            row: BindRow {
                action,
                label,
                changed: bind_changed(ui, action),
                keys: BindSlot::BOTH.map(|slot| key_view(ui, action, slot)),
            },
            lit: !needle.is_empty(),
        });
    }
    rows.push(Row::BindHint);
    rows
}

pub(super) fn rows(ui: &SettingsUi) -> Vec<Row> {
    let needle = needle(&ui.query);
    let rows = if ui.tab == SettingsTab::Controls {
        bind_rows(ui, &needle)
    } else {
        setting_rows(ui, &needle)
    };
    if rows.is_empty() {
        return vec![Row::Empty(ui.query.trim().to_string())];
    }
    rows
}

/// The footer: how much is unsaved, and the toast over it.
#[derive(Clone, PartialEq, Debug)]
pub struct FootView {
    pub pending: usize,
    pub toast: Option<String>,
}

impl FootView {
    pub(super) fn pending_label(&self) -> String {
        match self.pending {
            0 => "All changes saved".to_string(),
            1 => "1 unsaved change".to_string(),
            n => format!("{n} unsaved changes"),
        }
    }
}

pub(super) fn foot(ui: &SettingsUi) -> FootView {
    FootView {
        pending: pending(ui),
        toast: ui.toast.as_ref().map(|(text, _)| text.clone()),
    }
}

/// The toast after two binds trade places.
pub(super) fn swap_notice(clash: &Clash, moved: &KeyBind) -> String {
    let moved = if moved.is_bound() {
        keycap_label(moved)
    } else {
        "nothing".to_string()
    };
    format!(
        "{} is now {} \u{b7} {} moved to {moved}",
        action_label(clash.action),
        keycap_label(&clash.bind),
        action_label(clash.other.0),
    )
}
