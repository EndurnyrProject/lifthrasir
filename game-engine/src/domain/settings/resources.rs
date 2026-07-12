use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
use bevy::prelude::*;
use bevy::reflect::enums::DynamicEnum;
use bevy::reflect::{TypeInfo, Typed};
use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
use bevy_auto_plugin::prelude::auto_register_type;
use bevy_framepace::Limiter;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::domain::input::{PlayerAction, HOTBAR_ACTIONS};

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
    Taa,
    MsaaX2,
    MsaaX4,
}

impl AntiAliasing {
    /// The variants in stepper order.
    pub const ALL: [AntiAliasing; 5] = [
        AntiAliasing::Off,
        AntiAliasing::Fxaa,
        AntiAliasing::Taa,
        AntiAliasing::MsaaX2,
        AntiAliasing::MsaaX4,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            AntiAliasing::Off => "Off",
            AntiAliasing::Fxaa => "FXAA",
            AntiAliasing::Taa => "TAA",
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
            AntiAliasing::Taa => (Msaa::Off, false),
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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum Upscaling {
    #[default]
    Off,
    X2,
    X3,
    X4,
}

impl Upscaling {
    /// The variants in stepper order.
    pub const ALL: [Upscaling; 4] = [Upscaling::Off, Upscaling::X2, Upscaling::X3, Upscaling::X4];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            Upscaling::Off => "Off",
            Upscaling::X2 => "2x",
            Upscaling::X3 => "3x",
            Upscaling::X4 => "4x",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> Upscaling {
        cycle_next(&Upscaling::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> Upscaling {
        cycle_prev(&Upscaling::ALL, self)
    }

    /// xBRZ scale factor; `Off` performs no upscaling.
    pub fn factor(self) -> Option<u8> {
        match self {
            Upscaling::Off => None,
            Upscaling::X2 => Some(2),
            Upscaling::X3 => Some(3),
            Upscaling::X4 => Some(4),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum DlssMode {
    #[default]
    Off,
    Dlaa,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

impl DlssMode {
    /// The variants in stepper order (quality-descending).
    pub const ALL: [DlssMode; 6] = [
        DlssMode::Off,
        DlssMode::Dlaa,
        DlssMode::Quality,
        DlssMode::Balanced,
        DlssMode::Performance,
        DlssMode::UltraPerformance,
    ];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            DlssMode::Off => "Off",
            DlssMode::Dlaa => "DLAA",
            DlssMode::Quality => "Quality",
            DlssMode::Balanced => "Balanced",
            DlssMode::Performance => "Performance",
            DlssMode::UltraPerformance => "Ultra Performance",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> DlssMode {
        cycle_next(&DlssMode::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> DlssMode {
        cycle_prev(&DlssMode::ALL, self)
    }

    #[cfg(feature = "dlss")]
    pub fn to_perf_quality_mode(self) -> Option<bevy::anti_alias::dlss::DlssPerfQualityMode> {
        use bevy::anti_alias::dlss::DlssPerfQualityMode;
        match self {
            DlssMode::Off => None,
            DlssMode::Dlaa => Some(DlssPerfQualityMode::Dlaa),
            DlssMode::Quality => Some(DlssPerfQualityMode::Quality),
            DlssMode::Balanced => Some(DlssPerfQualityMode::Balanced),
            DlssMode::Performance => Some(DlssPerfQualityMode::Performance),
            DlssMode::UltraPerformance => Some(DlssPerfQualityMode::UltraPerformance),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum Ssao {
    #[default]
    Off,
    Low,
    Medium,
    High,
    Ultra,
}

impl Ssao {
    /// The variants in stepper order (quality-ascending).
    pub const ALL: [Ssao; 5] = [Ssao::Off, Ssao::Low, Ssao::Medium, Ssao::High, Ssao::Ultra];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            Ssao::Off => "Off",
            Ssao::Low => "Low",
            Ssao::Medium => "Medium",
            Ssao::High => "High",
            Ssao::Ultra => "Ultra",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> Ssao {
        cycle_next(&Ssao::ALL, self)
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> Ssao {
        cycle_prev(&Ssao::ALL, self)
    }

    /// Maps to the GTAO quality level; `Off` means no `ScreenSpaceAmbientOcclusion`
    /// component on the camera.
    pub fn to_quality_level(self) -> Option<ScreenSpaceAmbientOcclusionQualityLevel> {
        use ScreenSpaceAmbientOcclusionQualityLevel as Q;
        match self {
            Ssao::Off => None,
            Ssao::Low => Some(Q::Low),
            Ssao::Medium => Some(Q::Medium),
            Ssao::High => Some(Q::High),
            Ssao::Ultra => Some(Q::Ultra),
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
    /// xBRZ pixel-art upscaling baked into sprite/terrain/model textures at load.
    pub upscaling: Upscaling,
    pub vsync: bool,
    pub fps_cap: FpsCap,
    pub ui_scaling: UiScaling,
    /// HDR bloom on the world camera. Off drops the HDR pipeline entirely.
    pub bloom: bool,
    /// Directional-light (sun) shadow casting.
    pub shadows: bool,
    /// DLSS Super Resolution render-scaling mode (NVIDIA RTX only). Orthogonal to
    /// `upscaling` (xBRZ texture baking): DLSS scales render resolution, xBRZ bakes textures.
    pub dlss: DlssMode,
    /// Screen-space ambient occlusion (GTAO) quality. Adds contact darkening in
    /// terrain/model crevices; forces MSAA off (needs the depth/normal prepass).
    /// Runs on all native backends including macOS Metal.
    pub ssao: Ssao,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::BorderlessFullscreen,
            resolution: (1920, 1080),
            antialiasing: AntiAliasing::Fxaa,
            anisotropy: Anisotropy::X8,
            upscaling: Upscaling::Off,
            vsync: true,
            fps_cap: FpsCap::F60,
            ui_scaling: UiScaling::P100,
            bloom: true,
            shadows: true,
            dlss: DlssMode::Off,
            ssao: Ssao::Off,
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

/// Default binds for the twelve hotbar slots: F1..F12, unmodified. Used both as
/// `Keybinds::default().hotbar` and as the `serde(default)` for the field, so an
/// old `settings.ron` lacking `hotbar` loads the working F-keys, not empty binds.
fn default_hotbar_binds() -> [ActionBinds; 12] {
    std::array::from_fn(|i| ActionBinds {
        primary: Some(KeyBind::new(format!("F{}", i + 1))),
        secondary: None,
    })
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
    pub equipment: ActionBinds,
    pub cart: ActionBinds,
    pub party: ActionBinds,
    pub emote: ActionBinds,
    #[serde(default = "default_hotbar_binds")]
    pub hotbar: [ActionBinds; 12],
}

impl Default for Keybinds {
    /// Mirrors `PlayerAction::default_input_map()`:
    /// Sit = Insert / Help, Status = Alt+A, Inventory = Alt+E, Skills = Alt+S, Equipment = Alt+Q,
    /// Cart = Alt+W, Party = P, Emote = Alt+M.
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
            equipment: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyQ")),
                secondary: None,
            },
            cart: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyW")),
                secondary: None,
            },
            party: ActionBinds {
                primary: Some(KeyBind::new("KeyP")),
                secondary: None,
            },
            emote: ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyM")),
                secondary: None,
            },
            hotbar: default_hotbar_binds(),
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
        self.equipment
            .insert_into(&mut map, PlayerAction::Equipment);
        self.cart.insert_into(&mut map, PlayerAction::Cart);
        self.party.insert_into(&mut map, PlayerAction::Party);
        self.emote.insert_into(&mut map, PlayerAction::Emote);
        for (binds, action) in self.hotbar.iter().zip(HOTBAR_ACTIONS) {
            binds.insert_into(&mut map, action);
        }
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
    fn default_equipment_bind_is_alt_q() {
        assert_eq!(
            Keybinds::default().equipment,
            ActionBinds {
                primary: Some(KeyBind::modified(Modifier::Alt, "KeyQ")),
                secondary: None,
            }
        );
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
    fn default_party_bind_is_unmodified_p_and_unique() {
        let keybinds = Keybinds::default();
        assert_eq!(
            keybinds.party,
            ActionBinds {
                primary: Some(KeyBind::new("KeyP")),
                secondary: None,
            }
        );

        let non_hotbar = [
            &keybinds.sit,
            &keybinds.status,
            &keybinds.inventory,
            &keybinds.skills,
            &keybinds.equipment,
            &keybinds.cart,
            &keybinds.party,
        ];
        let mut seen: Vec<&KeyBind> = Vec::new();
        for action_binds in non_hotbar {
            for bind in [
                action_binds.primary.as_ref(),
                action_binds.secondary.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                assert!(!seen.contains(&bind), "duplicate default keybind {bind:?}");
                seen.push(bind);
            }
        }
    }

    #[test]
    fn default_keybinds_match_the_default_input_map() {
        assert_eq!(
            Keybinds::default().to_input_map(),
            PlayerAction::default_input_map()
        );
    }

    #[test]
    fn default_to_input_map_carries_every_hotbar_slot() {
        let from_keybinds = Keybinds::default().to_input_map();
        let from_actions = PlayerAction::default_input_map();
        for action in HOTBAR_ACTIONS {
            assert!(
                from_keybinds.get(&action).is_some(),
                "missing hotbar binding for {action:?}"
            );
            assert_eq!(from_keybinds.get(&action), from_actions.get(&action));
        }
    }

    #[test]
    fn keybinds_without_hotbar_field_fill_f_key_defaults() {
        let legacy = r#"(
            sit: (primary: Some((key: "Insert", modifier: None)), secondary: Some((key: "Help", modifier: None))),
            status: (primary: Some((key: "KeyA", modifier: Some(Alt))), secondary: None),
            inventory: (primary: Some((key: "KeyE", modifier: Some(Alt))), secondary: None),
            skills: (primary: Some((key: "KeyS", modifier: Some(Alt))), secondary: None),
        )"#;

        let decoded: Keybinds = ron::from_str(legacy).expect("legacy keybinds should load");
        assert_eq!(decoded.hotbar, Keybinds::default().hotbar);
        assert_eq!(
            decoded.hotbar[0].primary,
            Some(KeyBind::new("F1")),
            "hotbar must default to F-keys, not empty binds"
        );
        assert_eq!(decoded.hotbar[11].primary, Some(KeyBind::new("F12")));
        assert!(decoded.hotbar.iter().all(|b| b.primary.is_some()));
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
    fn upscaling_default_is_off() {
        assert_eq!(GraphicsSettings::default().upscaling, Upscaling::Off);
    }

    #[test]
    fn upscaling_serde_round_trips_every_variant() {
        for variant in Upscaling::ALL {
            let encoded = ron::to_string(&variant).expect("serialize");
            let decoded: Upscaling = ron::from_str(&encoded).expect("deserialize");
            assert_eq!(variant, decoded);
        }
    }

    #[test]
    fn upscaling_factor_mapping() {
        assert_eq!(Upscaling::Off.factor(), None);
        assert_eq!(Upscaling::X2.factor(), Some(2));
        assert_eq!(Upscaling::X3.factor(), Some(3));
        assert_eq!(Upscaling::X4.factor(), Some(4));
    }

    #[test]
    fn upscaling_cycles_and_clamps() {
        assert_eq!(Upscaling::Off.next(), Upscaling::X2);
        assert_eq!(Upscaling::X4.next(), Upscaling::X4);
        assert_eq!(Upscaling::X2.prev(), Upscaling::Off);
        assert_eq!(Upscaling::Off.prev(), Upscaling::Off);
        assert_eq!(Upscaling::Off.label(), "Off");
        assert_eq!(Upscaling::X2.label(), "2x");
        assert_eq!(Upscaling::X3.label(), "3x");
        assert_eq!(Upscaling::X4.label(), "4x");
    }

    #[test]
    fn dlss_mode_default_is_off() {
        assert_eq!(DlssMode::default(), DlssMode::Off);
        assert_eq!(GraphicsSettings::default().dlss, DlssMode::Off);
    }

    #[test]
    fn dlss_mode_serde_round_trips_every_variant() {
        for variant in DlssMode::ALL {
            let encoded = ron::to_string(&variant).expect("serialize");
            let decoded: DlssMode = ron::from_str(&encoded).expect("deserialize");
            assert_eq!(variant, decoded);
        }
    }

    #[test]
    fn dlss_mode_cycles_and_clamps() {
        assert_eq!(DlssMode::Off.next(), DlssMode::Dlaa);
        assert_eq!(
            DlssMode::UltraPerformance.next(),
            DlssMode::UltraPerformance
        );
        assert_eq!(DlssMode::Dlaa.prev(), DlssMode::Off);
        assert_eq!(DlssMode::Off.prev(), DlssMode::Off);
        assert_eq!(DlssMode::Off.label(), "Off");
        assert_eq!(DlssMode::Dlaa.label(), "DLAA");
        assert_eq!(DlssMode::UltraPerformance.label(), "Ultra Performance");
    }

    #[test]
    fn ssao_default_is_off() {
        assert_eq!(Ssao::default(), Ssao::Off);
        assert_eq!(GraphicsSettings::default().ssao, Ssao::Off);
    }

    #[test]
    fn ssao_cycles_and_clamps() {
        assert_eq!(Ssao::Off.next(), Ssao::Low);
        assert_eq!(Ssao::Ultra.next(), Ssao::Ultra);
        assert_eq!(Ssao::Low.prev(), Ssao::Off);
        assert_eq!(Ssao::Off.prev(), Ssao::Off);
        assert_eq!(Ssao::Off.label(), "Off");
        assert_eq!(Ssao::Medium.label(), "Medium");
    }

    #[test]
    fn ssao_maps_to_quality_level() {
        use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel as Q;
        assert!(Ssao::Off.to_quality_level().is_none());
        assert!(matches!(Ssao::Low.to_quality_level(), Some(Q::Low)));
        assert!(matches!(Ssao::Medium.to_quality_level(), Some(Q::Medium)));
        assert!(matches!(Ssao::High.to_quality_level(), Some(Q::High)));
        assert!(matches!(Ssao::Ultra.to_quality_level(), Some(Q::Ultra)));
    }

    #[test]
    fn ssao_serde_round_trips_every_variant() {
        for variant in Ssao::ALL {
            let encoded = ron::to_string(&variant).expect("serialize");
            let decoded: Ssao = ron::from_str(&encoded).expect("deserialize");
            assert_eq!(variant, decoded);
        }
    }

    #[test]
    fn graphics_without_ssao_field_defaults_to_off() {
        let legacy = "(display_mode:Fullscreen,resolution:(1280,720),antialiasing:Off,vsync:false,fps_cap:F120)";
        let decoded: GraphicsSettings = ron::from_str(legacy).expect("deserialize legacy graphics");
        assert_eq!(decoded.ssao, Ssao::Off);
    }

    #[test]
    fn antialiasing_taa_maps_to_no_msaa_no_fxaa() {
        assert_eq!(AntiAliasing::Taa.to_msaa_fxaa(), (Msaa::Off, false));
        assert_eq!(AntiAliasing::Taa.label(), "TAA");
        assert_eq!(AntiAliasing::Fxaa.next(), AntiAliasing::Taa);
        assert_eq!(AntiAliasing::Taa.next(), AntiAliasing::MsaaX2);
        assert_eq!(AntiAliasing::Taa.prev(), AntiAliasing::Fxaa);
    }

    #[test]
    fn graphics_without_dlss_field_defaults_to_off() {
        let legacy = "(display_mode:Fullscreen,resolution:(1280,720),antialiasing:Off,vsync:false,fps_cap:F120)";
        let decoded: GraphicsSettings = ron::from_str(legacy).expect("deserialize legacy graphics");
        assert_eq!(decoded.dlss, DlssMode::Off);
    }

    #[cfg(feature = "dlss")]
    #[test]
    fn dlss_mode_maps_to_perf_quality_mode() {
        use bevy::anti_alias::dlss::DlssPerfQualityMode;
        assert_eq!(DlssMode::Off.to_perf_quality_mode(), None);
        assert_eq!(
            DlssMode::Dlaa.to_perf_quality_mode(),
            Some(DlssPerfQualityMode::Dlaa)
        );
        assert_eq!(
            DlssMode::Quality.to_perf_quality_mode(),
            Some(DlssPerfQualityMode::Quality)
        );
        assert_eq!(
            DlssMode::Balanced.to_perf_quality_mode(),
            Some(DlssPerfQualityMode::Balanced)
        );
        assert_eq!(
            DlssMode::Performance.to_perf_quality_mode(),
            Some(DlssPerfQualityMode::Performance)
        );
        assert_eq!(
            DlssMode::UltraPerformance.to_perf_quality_mode(),
            Some(DlssPerfQualityMode::UltraPerformance)
        );
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
            ..Default::default()
        };
        let map = binds.to_input_map();
        let sit = map.get(&PlayerAction::Sit).expect("sit binding");
        assert_eq!(sit.len(), 1);
    }
}
