use bevy::prelude::*;
use bevy::reflect::{DynamicEnum, TypeInfo, Typed};
use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
use bevy_auto_plugin::prelude::auto_register_type;
use bevy_framepace::Limiter;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::domain::input::PlayerAction;

/// Resolution presets offered in the settings UI.
pub const RESOLUTIONS: [(u32, u32); 5] = [
    (1280, 720),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

/// Stepper label for a resolution preset (e.g. `1920 x 1080`).
pub fn resolution_label((w, h): (u32, u32)) -> String {
    format!("{w} x {h}")
}

/// Index of `resolution` within `RESOLUTIONS`; falls back to the default preset
/// (1920x1080) when an off-preset value somehow lands in the draft.
pub fn resolution_index(resolution: (u32, u32)) -> usize {
    RESOLUTIONS
        .iter()
        .position(|&preset| preset == resolution)
        .unwrap_or(2)
}

/// Next preset after `resolution`, clamped at the last.
pub fn resolution_next(resolution: (u32, u32)) -> (u32, u32) {
    let i = resolution_index(resolution);
    RESOLUTIONS[(i + 1).min(RESOLUTIONS.len() - 1)]
}

/// Previous preset before `resolution`, clamped at the first.
pub fn resolution_prev(resolution: (u32, u32)) -> (u32, u32) {
    let i = resolution_index(resolution);
    RESOLUTIONS[i.saturating_sub(1)]
}

/// Next variant after `value` in `all`, clamped at the last element.
fn cycle_next<T: Copy + PartialEq>(all: &[T], value: T) -> T {
    let i = all.iter().position(|&v| v == value).unwrap_or(0);
    all[(i + 1).min(all.len() - 1)]
}

/// Previous variant before `value` in `all`, clamped at the first element.
fn cycle_prev<T: Copy + PartialEq>(all: &[T], value: T) -> T {
    let i = all.iter().position(|&v| v == value).unwrap_or(0);
    all[i.saturating_sub(1)]
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
pub enum DisplayMode {
    Windowed,
    BorderlessFullscreen,
    Fullscreen,
}

impl DisplayMode {
    /// The variants in segmented-control order (Windowed / Borderless / Fullscreen).
    pub const ALL: [DisplayMode; 3] = [
        DisplayMode::Windowed,
        DisplayMode::BorderlessFullscreen,
        DisplayMode::Fullscreen,
    ];

    /// Short button label for the segmented control.
    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Windowed => "Windowed",
            DisplayMode::BorderlessFullscreen => "Borderless",
            DisplayMode::Fullscreen => "Fullscreen",
        }
    }

    /// Pure mapping to the Bevy window mode (Task 2 applies it to the window).
    pub fn to_window_mode(self) -> WindowMode {
        match self {
            DisplayMode::Windowed => WindowMode::Windowed,
            DisplayMode::BorderlessFullscreen => {
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            DisplayMode::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
pub enum AntiAliasing {
    Off,
    Fxaa,
    MsaaX2,
    MsaaX4,
}

impl AntiAliasing {
    /// The variants in stepper order.
    pub const ALL: [AntiAliasing; 4] = [
        AntiAliasing::Off,
        AntiAliasing::Fxaa,
        AntiAliasing::MsaaX2,
        AntiAliasing::MsaaX4,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            AntiAliasing::Off => "Off",
            AntiAliasing::Fxaa => "FXAA",
            AntiAliasing::MsaaX2 => "MSAA x2",
            AntiAliasing::MsaaX4 => "MSAA x4",
        }
    }

    /// Next variant, clamped at the last (matches the stepper's disabled arrow).
    pub fn next(self) -> AntiAliasing {
        cycle_next(&AntiAliasing::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> AntiAliasing {
        cycle_prev(&AntiAliasing::ALL, self)
    }

    /// Maps to `(Msaa, has_fxaa)`; Task 2 inserts the `Msaa` and adds/removes
    /// `Fxaa` on the world camera accordingly.
    pub fn to_msaa_fxaa(self) -> (Msaa, bool) {
        match self {
            AntiAliasing::Off => (Msaa::Off, false),
            AntiAliasing::Fxaa => (Msaa::Off, true),
            AntiAliasing::MsaaX2 => (Msaa::Sample2, false),
            AntiAliasing::MsaaX4 => (Msaa::Sample4, false),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
pub enum Anisotropy {
    Off,
    X2,
    X4,
    X8,
    X16,
}

impl Anisotropy {
    /// The variants in stepper order.
    pub const ALL: [Anisotropy; 5] = [
        Anisotropy::Off,
        Anisotropy::X2,
        Anisotropy::X4,
        Anisotropy::X8,
        Anisotropy::X16,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            Anisotropy::Off => "Off",
            Anisotropy::X2 => "2x",
            Anisotropy::X4 => "4x",
            Anisotropy::X8 => "8x",
            Anisotropy::X16 => "16x",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> Anisotropy {
        cycle_next(&Anisotropy::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> Anisotropy {
        cycle_prev(&Anisotropy::ALL, self)
    }

    /// wgpu `anisotropy_clamp` tap count; `Off` is `1` (plain trilinear).
    pub fn to_clamp(self) -> u16 {
        match self {
            Anisotropy::Off => 1,
            Anisotropy::X2 => 2,
            Anisotropy::X4 => 4,
            Anisotropy::X8 => 8,
            Anisotropy::X16 => 16,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
pub enum FpsCap {
    F30,
    F60,
    F120,
    F144,
    Unlimited,
}

impl FpsCap {
    /// The variants in stepper order.
    pub const ALL: [FpsCap; 5] = [
        FpsCap::F30,
        FpsCap::F60,
        FpsCap::F120,
        FpsCap::F144,
        FpsCap::Unlimited,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            FpsCap::F30 => "30",
            FpsCap::F60 => "60",
            FpsCap::F120 => "120",
            FpsCap::F144 => "144",
            FpsCap::Unlimited => "Unlimited",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> FpsCap {
        cycle_next(&FpsCap::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> FpsCap {
        cycle_prev(&FpsCap::ALL, self)
    }

    /// Maps to the framepace limiter; `Unlimited` disables limiting.
    pub fn to_limiter(self) -> Limiter {
        match self {
            FpsCap::F30 => Limiter::from_framerate(30.0),
            FpsCap::F60 => Limiter::from_framerate(60.0),
            FpsCap::F120 => Limiter::from_framerate(120.0),
            FpsCap::F144 => Limiter::from_framerate(144.0),
            FpsCap::Unlimited => Limiter::Off,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum UiScaling {
    P80,
    #[default]
    P100,
    P125,
    P150,
    P175,
    P200,
}

impl UiScaling {
    /// The variants in stepper order.
    pub const ALL: [UiScaling; 6] = [
        UiScaling::P80,
        UiScaling::P100,
        UiScaling::P125,
        UiScaling::P150,
        UiScaling::P175,
        UiScaling::P200,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            UiScaling::P80 => "80%",
            UiScaling::P100 => "100%",
            UiScaling::P125 => "125%",
            UiScaling::P150 => "150%",
            UiScaling::P175 => "175%",
            UiScaling::P200 => "200%",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> UiScaling {
        cycle_next(&UiScaling::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> UiScaling {
        cycle_prev(&UiScaling::ALL, self)
    }

    /// Maps to the `bevy::ui::UiScale` factor Task 2 inserts as a resource.
    pub fn to_scale_factor(self) -> f32 {
        match self {
            UiScaling::P80 => 0.8,
            UiScaling::P100 => 1.0,
            UiScaling::P125 => 1.25,
            UiScaling::P150 => 1.5,
            UiScaling::P175 => 1.75,
            UiScaling::P200 => 2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
#[serde(default)]
pub struct GraphicsSettings {
    pub display_mode: DisplayMode,
    pub resolution: (u32, u32),
    pub antialiasing: AntiAliasing,
    /// Anisotropic texture filtering for ground terrain viewed at grazing angles.
    pub anisotropy: Anisotropy,
    pub vsync: bool,
    pub fps_cap: FpsCap,
    pub ui_scaling: UiScaling,
    /// HDR bloom on the world camera. Off drops the HDR pipeline entirely.
    pub bloom: bool,
    /// Directional-light (sun) shadow casting.
    pub shadows: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::BorderlessFullscreen,
            resolution: (1920, 1080),
            antialiasing: AntiAliasing::Fxaa,
            anisotropy: Anisotropy::X8,
            vsync: true,
            fps_cap: FpsCap::F60,
            ui_scaling: UiScaling::P100,
            bloom: true,
            shadows: true,
        }
    }
}

/// Persisted mirror of the runtime `AudioSettings` fields.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Reflect, Debug)]
#[serde(default)]
pub struct AudioConfig {
    pub bgm_volume: f32,
    pub bgm_muted: bool,
    pub sfx_volume: f32,
    pub sfx_muted: bool,
    pub ambient_volume: f32,
    pub ambient_muted: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            bgm_volume: 0.70,
            bgm_muted: false,
            sfx_volume: 0.85,
            sfx_muted: false,
            ambient_volume: 0.55,
            ambient_muted: false,
        }
    }
}

/// A held modifier in a key chord. Serde-only mirror of leafwing's `ModifierKey`;
/// Task 4 owns the conversion.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
pub enum Modifier {
    Alt,
    Control,
    Shift,
    Super,
}

impl Modifier {
    fn to_modifier_key(self) -> ModifierKey {
        match self {
            Modifier::Alt => ModifierKey::Alt,
            Modifier::Control => ModifierKey::Control,
            Modifier::Shift => ModifierKey::Shift,
            Modifier::Super => ModifierKey::Super,
        }
    }
}

/// Resolves a `KeyCode` variant name (e.g. "Insert", "KeyA") into the value via
/// reflection. Returns `None` for unknown names (every meaningful `KeyCode` is a
/// unit variant; only `Unidentified` is not, and it is never a real keybind).
fn key_code_from_name(name: &str) -> Option<KeyCode> {
    let TypeInfo::Enum(info) = KeyCode::type_info() else {
        return None;
    };
    if !info.contains_variant(name) {
        return None;
    }
    KeyCode::from_reflect(&DynamicEnum::new(name, ()))
}

/// A single bound key, optionally modified. `key` is a `KeyCode` name
/// (e.g. "Insert", "KeyA"); Task 4 parses it back into a `KeyCode`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Reflect, Debug)]
pub struct KeyBind {
    pub key: String,
    pub modifier: Option<Modifier>,
}

impl KeyBind {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            modifier: None,
        }
    }

    pub fn modified(modifier: Modifier, key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            modifier: Some(modifier),
        }
    }

    /// Inserts this binding into `map` for `action`. A bare `KeyCode` when
    /// unmodified, a `ButtonlikeChord::modified` when a modifier is set.
    /// Skips with a `warn!` if the key name is unparseable.
    fn insert_into(&self, map: &mut InputMap<PlayerAction>, action: PlayerAction) {
        let Some(key) = key_code_from_name(&self.key) else {
            warn!("unknown key code '{}' in keybinds, skipping", self.key);
            return;
        };
        match self.modifier {
            Some(modifier) => {
                map.insert(
                    action,
                    ButtonlikeChord::modified(modifier.to_modifier_key(), key),
                );
            }
            None => {
                map.insert(action, key);
            }
        }
    }
}

/// Primary + secondary slot for one action.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Reflect, Debug, Default)]
pub struct ActionBinds {
    pub primary: Option<KeyBind>,
    pub secondary: Option<KeyBind>,
}

impl ActionBinds {
    fn insert_into(&self, map: &mut InputMap<PlayerAction>, action: PlayerAction) {
        if let Some(bind) = &self.primary {
            bind.insert_into(map, action);
        }
        if let Some(bind) = &self.secondary {
            bind.insert_into(map, action);
        }
    }
}

/// Serde-only keybinds for the existing `PlayerAction`s. No leafwing coupling
/// here; Task 4 adds `to_input_map` / `from_input_map`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Reflect, Debug)]
#[serde(default)]
pub struct Keybinds {
    pub sit: ActionBinds,
    pub status: ActionBinds,
    pub inventory: ActionBinds,
    pub skills: ActionBinds,
}

impl Default for Keybinds {
    /// Mirrors `PlayerAction::default_input_map()`:
    /// Sit = Insert / Help, Status = Alt+A, Inventory = Alt+E, Skills = Alt+S.
    fn default() -> Self {
        Self {
            sit: ActionBinds {
                primary: Some(KeyBind::new("Insert")),
                secondary: Some(KeyBind::new("Help")),
            },
            status: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyA")),
                secondary: None,
            },
            inventory: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyE")),
                secondary: None,
            },
            skills: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyS")),
                secondary: None,
            },
        }
    }
}

impl Keybinds {
    /// Builds a leafwing `InputMap` from the stored bindings. Unparseable key
    /// names are skipped (with a `warn!`) rather than panicking.
    pub fn to_input_map(&self) -> InputMap<PlayerAction> {
        let mut map = InputMap::default();
        self.sit.insert_into(&mut map, PlayerAction::Sit);
        self.status.insert_into(&mut map, PlayerAction::Status);
        self.inventory
            .insert_into(&mut map, PlayerAction::Inventory);
        self.skills.insert_into(&mut map, PlayerAction::Skills);
        map
    }
}

#[derive(Resource, Serialize, Deserialize, Clone, PartialEq, Reflect, Debug, Default)]
#[serde(default)]
#[reflect(Resource)]
#[auto_register_type(plugin = super::SettingsPlugin)]
pub struct Settings {
    pub graphics: GraphicsSettings,
    pub audio: AudioConfig,
    pub keybinds: Keybinds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_round_trips_through_ron() {
        let settings = Settings::default();
        let encoded = ron::to_string(&settings).expect("serialize");
        let decoded: Settings = ron::from_str(&encoded).expect("deserialize");
        assert_eq!(settings, decoded);
    }

    #[test]
    fn settings_load_with_missing_fields_filled_from_defaults() {
        // A settings.ron written before `keybinds.skills` (and before the
        // graphics/audio sections) existed. `#[serde(default)]` must fill every
        // gap from Default rather than failing to deserialize.
        let partial = r#"(
            keybinds: (
                sit: (primary: Some((key: "Insert", modifier: None)), secondary: Some((key: "Help", modifier: None))),
                status: (primary: Some((key: "KeyA", modifier: Some(Alt))), secondary: None),
                inventory: (primary: Some((key: "KeyE", modifier: Some(Alt))), secondary: None),
            ),
        )"#;

        let decoded: Settings = ron::from_str(partial).expect("partial settings should load");
        let defaults = Settings::default();

        assert_eq!(decoded.keybinds.skills, defaults.keybinds.skills);
        assert_eq!(decoded.graphics, defaults.graphics);
        assert_eq!(decoded.audio, defaults.audio);
        assert_eq!(decoded.keybinds.sit.primary, Some(KeyBind::new("Insert")));
    }

    #[test]
    fn default_settings_match_the_mockup() {
        let s = Settings::default();
        assert_eq!(s.graphics.display_mode, DisplayMode::BorderlessFullscreen);
        assert_eq!(s.graphics.resolution, (1920, 1080));
        assert_eq!(s.graphics.antialiasing, AntiAliasing::Fxaa);
        assert!(s.graphics.vsync);
        assert_eq!(s.graphics.fps_cap, FpsCap::F60);
        assert_eq!(s.graphics.ui_scaling, UiScaling::P100);
        assert!(s.graphics.bloom);
        assert!(s.graphics.shadows);
        assert_eq!(s.audio.bgm_volume, 0.70);
        assert_eq!(s.audio.sfx_volume, 0.85);
        assert_eq!(s.audio.ambient_volume, 0.55);
        assert!(!s.audio.bgm_muted);
        assert!(!s.audio.sfx_muted);
        assert!(!s.audio.ambient_muted);
    }

    #[test]
    fn display_mode_maps_to_window_mode() {
        assert!(matches!(
            DisplayMode::Windowed.to_window_mode(),
            WindowMode::Windowed
        ));
        assert!(matches!(
            DisplayMode::BorderlessFullscreen.to_window_mode(),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        ));
        assert!(matches!(
            DisplayMode::Fullscreen.to_window_mode(),
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        ));
    }

    #[test]
    fn antialiasing_maps_to_msaa_and_fxaa() {
        assert_eq!(AntiAliasing::Off.to_msaa_fxaa(), (Msaa::Off, false));
        assert_eq!(AntiAliasing::Fxaa.to_msaa_fxaa(), (Msaa::Off, true));
        assert_eq!(AntiAliasing::MsaaX2.to_msaa_fxaa(), (Msaa::Sample2, false));
        assert_eq!(AntiAliasing::MsaaX4.to_msaa_fxaa(), (Msaa::Sample4, false));
    }

    #[test]
    fn fps_cap_maps_to_limiter() {
        assert!(matches!(FpsCap::Unlimited.to_limiter(), Limiter::Off));
        assert!(matches!(FpsCap::F60.to_limiter(), Limiter::Manual(_)));
        let Limiter::Manual(d) = FpsCap::F30.to_limiter() else {
            panic!("expected manual limiter");
        };
        assert!((d.as_secs_f64() - 1.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn default_keybinds_match_the_default_input_map() {
        assert_eq!(
            Keybinds::default().to_input_map(),
            PlayerAction::default_input_map()
        );
    }

    #[test]
    fn antialiasing_cycles_and_clamps() {
        assert_eq!(AntiAliasing::Off.next(), AntiAliasing::Fxaa);
        assert_eq!(AntiAliasing::MsaaX4.next(), AntiAliasing::MsaaX4);
        assert_eq!(AntiAliasing::Fxaa.prev(), AntiAliasing::Off);
        assert_eq!(AntiAliasing::Off.prev(), AntiAliasing::Off);
        assert_eq!(AntiAliasing::Off.label(), "Off");
        assert_eq!(AntiAliasing::MsaaX2.label(), "MSAA x2");
    }

    #[test]
    fn fps_cap_cycles_and_clamps() {
        assert_eq!(FpsCap::F30.next(), FpsCap::F60);
        assert_eq!(FpsCap::Unlimited.next(), FpsCap::Unlimited);
        assert_eq!(FpsCap::F60.prev(), FpsCap::F30);
        assert_eq!(FpsCap::F30.prev(), FpsCap::F30);
        assert_eq!(FpsCap::F144.label(), "144");
        assert_eq!(FpsCap::Unlimited.label(), "Unlimited");
    }

    #[test]
    fn graphics_without_ui_scaling_defaults_to_100() {
        let legacy = "(display_mode:Fullscreen,resolution:(1280,720),antialiasing:Off,vsync:false,fps_cap:F120)";
        let decoded: GraphicsSettings = ron::from_str(legacy).expect("deserialize legacy graphics");
        assert_eq!(decoded.ui_scaling, UiScaling::P100);
    }

    #[test]
    fn ui_scaling_cycles_and_clamps() {
        assert_eq!(UiScaling::P80.next(), UiScaling::P100);
        assert_eq!(UiScaling::P200.next(), UiScaling::P200);
        assert_eq!(UiScaling::P100.prev(), UiScaling::P80);
        assert_eq!(UiScaling::P80.prev(), UiScaling::P80);
        assert_eq!(UiScaling::P125.label(), "125%");
        assert_eq!(UiScaling::P200.label(), "200%");
    }

    #[test]
    fn ui_scaling_maps_to_scale_factor() {
        assert_eq!(UiScaling::P80.to_scale_factor(), 0.8);
        assert_eq!(UiScaling::P100.to_scale_factor(), 1.0);
        assert_eq!(UiScaling::P200.to_scale_factor(), 2.0);
    }

    #[test]
    fn display_mode_labels_in_order() {
        assert_eq!(DisplayMode::ALL[0], DisplayMode::Windowed);
        assert_eq!(DisplayMode::Windowed.label(), "Windowed");
        assert_eq!(DisplayMode::BorderlessFullscreen.label(), "Borderless");
        assert_eq!(DisplayMode::Fullscreen.label(), "Fullscreen");
    }

    #[test]
    fn resolution_steps_and_clamps_over_presets() {
        assert_eq!(resolution_index((1920, 1080)), 2);
        assert_eq!(resolution_index((1, 1)), 2);
        assert_eq!(resolution_next((1280, 720)), (1600, 900));
        assert_eq!(resolution_next((3840, 2160)), (3840, 2160));
        assert_eq!(resolution_prev((1600, 900)), (1280, 720));
        assert_eq!(resolution_prev((1280, 720)), (1280, 720));
        assert_eq!(resolution_label((1920, 1080)), "1920 x 1080");
    }

    #[test]
    fn unknown_key_name_is_skipped() {
        assert!(key_code_from_name("NotAKey").is_none());

        let binds = Keybinds {
            sit: ActionBinds {
                primary: Some(KeyBind::new("NotAKey")),
                secondary: Some(KeyBind::new("Insert")),
            },
            status: ActionBinds::default(),
            inventory: ActionBinds::default(),
            skills: ActionBinds::default(),
        };
        let map = binds.to_input_map();
        let sit = map.get(&PlayerAction::Sit).expect("sit binding");
        assert_eq!(sit.len(), 1);
    }
}
