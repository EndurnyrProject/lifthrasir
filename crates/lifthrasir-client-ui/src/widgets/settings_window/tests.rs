use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::scene::ScenePlugin;
use lifthrasir_client::domain::settings::resources::Ssao;
use lifthrasir_client::domain::settings::{
    ActionBinds, Anisotropy, AntiAliasing, DisplayMode, FpsCap, UiScaling,
};

use super::view::{ControlView, KeyState, Row};
use super::*;
use crate::screens::shared::keycap_label;

fn settings_ui() -> SettingsUi {
    let mut world = World::new();
    world.insert_resource(GraphicsSettings::default());
    world.insert_resource(AudioConfig::default());
    world.insert_resource(Keybinds::default());
    SettingsUi::from_world(&mut world)
}

fn dirtied_ui() -> SettingsUi {
    let mut ui = settings_ui();
    ui.draft.graphics.fps_cap = FpsCap::F120;
    ui
}

/// The settings named by the rows `ui` shows.
fn listed(ui: &SettingsUi) -> Vec<Setting> {
    view::rows(ui)
        .into_iter()
        .filter_map(|row| match row {
            Row::Setting { row, .. } => Some(row.setting),
            _ => None,
        })
        .collect()
}

fn setting_row(ui: &SettingsUi, setting: Setting) -> view::SettingRow {
    view::rows(ui)
        .into_iter()
        .find_map(|row| match row {
            Row::Setting { row, .. } if row.setting == setting => Some(row),
            _ => None,
        })
        .expect("the setting is listed")
}

fn bind_row(ui: &SettingsUi, action: PlayerAction) -> view::BindRow {
    view::rows(ui)
        .into_iter()
        .find_map(|row| match row {
            Row::Bind { row, .. } if row.action == action => Some(row),
            _ => None,
        })
        .expect("the action is listed")
}

// ── The draft ─────────────────────────────────────────────────────────────

#[test]
fn settings_ui_snapshots_loaded_resources() {
    let graphics = GraphicsSettings {
        fps_cap: FpsCap::F120,
        ..default()
    };
    let audio = AudioConfig {
        bgm_volume: 0.25,
        ..default()
    };
    let keybinds = Keybinds {
        sit: ActionBinds::default(),
        ..default()
    };

    let mut world = World::new();
    world.insert_resource(graphics);
    world.insert_resource(audio);
    world.insert_resource(keybinds.clone());

    let ui = <SettingsUi as FromWorld>::from_world(&mut world);

    assert_eq!(ui.draft.graphics, graphics);
    assert_eq!(ui.draft.audio, audio);
    assert_eq!(ui.draft.keybinds, keybinds);
    assert_eq!(ui.committed, ui.draft);
}

#[test]
fn editing_the_draft_makes_it_dirty() {
    let clean = settings_ui();
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
    let mut ui = settings_ui();
    ui.draft.graphics.antialiasing = AntiAliasing::MsaaX4;
    ui.committed.graphics.antialiasing = AntiAliasing::MsaaX4;
    assert!(!ui.dirty());

    ui.reset();

    assert_eq!(ui.draft, SettingsDraft::default());
    assert!(ui.dirty());
}

// ── Sections and search ───────────────────────────────────────────────────

#[test]
fn each_section_lists_its_own_settings() {
    let mut ui = settings_ui();
    let graphics = listed(&ui);
    assert_eq!(graphics.first(), Some(&Setting::DisplayMode));
    assert!(graphics.contains(&Setting::Shadows));
    assert_eq!(
        graphics.contains(&Setting::Dlss),
        cfg!(feature = "dlss"),
        "DLSS shows only where it is compiled in"
    );

    ui.edit(SettingsAction::Section(SettingsTab::Sound));
    assert_eq!(
        listed(&ui),
        [
            Setting::Volume(AudioChannel::Bgm),
            Setting::Volume(AudioChannel::Sfx),
            Setting::Volume(AudioChannel::Ambient),
        ]
    );

    ui.edit(SettingsAction::Section(SettingsTab::Interface));
    assert_eq!(listed(&ui), [Setting::UiScaling, Setting::ShowNames]);
}

#[test]
fn the_presets_lead_the_graphics_section_until_a_search_is_typed() {
    let mut ui = settings_ui();
    assert!(matches!(view::rows(&ui)[0], Row::Presets(_)));

    ui.query = "  SHADOW ".into();
    let rows = view::rows(&ui);
    assert!(!rows.iter().any(|row| matches!(row, Row::Presets(_))));
    assert_eq!(listed(&ui), [Setting::Ssao, Setting::Shadows]);
    assert!(
        rows.iter()
            .all(|row| !matches!(row, Row::Setting { lit: false, .. })),
        "found rows are lit"
    );
}

#[test]
fn the_rail_counts_what_the_search_finds_in_each_section() {
    let mut ui = settings_ui();
    assert!(view::nav(&ui).iter().all(|item| item.hits.is_none()));

    ui.query = "hotbar".into();
    let hits: Vec<_> = view::nav(&ui)
        .into_iter()
        .map(|item| (item.tab, item.hits))
        .collect();
    assert_eq!(
        hits,
        [
            (SettingsTab::Graphics, Some(0)),
            (SettingsTab::Sound, Some(0)),
            (SettingsTab::Interface, Some(0)),
            (SettingsTab::Controls, Some(12)),
        ]
    );
}

#[test]
fn a_search_that_finds_nothing_says_so() {
    let mut ui = settings_ui();
    ui.query = " zzz ".into();
    assert_eq!(view::rows(&ui), [Row::Empty("zzz".into())]);

    ui.edit(SettingsAction::Section(SettingsTab::Controls));
    assert_eq!(view::rows(&ui), [Row::Empty("zzz".into())]);
}

// ── Editing settings ──────────────────────────────────────────────────────

#[test]
fn a_segment_picks_its_option_and_lights_up() {
    let mut ui = settings_ui();
    ui.edit(SettingsAction::Choose(Setting::Antialiasing, 2));

    assert_eq!(ui.draft.graphics.antialiasing, AntiAliasing::ALL[2]);
    let row = setting_row(&ui, Setting::Antialiasing);
    assert!(row.changed);
    assert!(matches!(
        row.control,
        ControlView::Segments { selected: 2, .. }
    ));

    ui.edit(SettingsAction::Choose(Setting::Antialiasing, 99));
    assert_eq!(ui.draft.graphics.antialiasing, AntiAliasing::ALL[2]);
}

#[test]
fn a_cycle_box_steps_through_its_options_and_wraps() {
    let mut ui = settings_ui();
    ui.draft.graphics.display_mode = DisplayMode::ALL[0];
    for expected in [1, 2, 0] {
        ui.edit(SettingsAction::Cycle(Setting::DisplayMode));
        assert_eq!(ui.draft.graphics.display_mode, DisplayMode::ALL[expected]);
    }
    assert_eq!(
        setting_row(&ui, Setting::DisplayMode).control,
        ControlView::Cycle(DisplayMode::ALL[0].label().into())
    );
}

#[test]
fn a_switch_flips_its_setting() {
    let mut ui = settings_ui();
    assert_eq!(
        setting_row(&ui, Setting::Bloom).control,
        ControlView::Toggle(true)
    );

    ui.edit(SettingsAction::Toggle(Setting::Bloom));

    assert!(!ui.draft.graphics.bloom);
    assert_eq!(
        setting_row(&ui, Setting::Bloom).control,
        ControlView::Toggle(false)
    );
}

#[test]
fn pending_upscaling_is_tagged_as_applying_on_map_reload() {
    let mut ui = settings_ui();
    assert_eq!(setting_row(&ui, Setting::Upscaling).tag, None);

    ui.edit(SettingsAction::Choose(Setting::Upscaling, 1));
    assert!(setting_row(&ui, Setting::Upscaling).tag.is_some());
}

#[test]
fn a_preset_sets_the_quality_settings_and_tuning_one_makes_it_custom() {
    let mut ui = settings_ui();
    ui.edit(SettingsAction::Preset(Preset::Ultra));

    let graphics = &ui.draft.graphics;
    assert_eq!(graphics.antialiasing, AntiAliasing::Taa);
    assert_eq!(graphics.anisotropy, Anisotropy::X16);
    assert_eq!(graphics.ssao, Ssao::High);
    assert!(graphics.bloom && graphics.shadows);
    assert_eq!(view::rows(&ui)[0], Row::Presets(Some(Preset::Ultra)));

    ui.edit(SettingsAction::Toggle(Setting::Bloom));
    assert_eq!(view::rows(&ui)[0], Row::Presets(None));
}

#[test]
fn every_preset_is_told_apart() {
    for preset in Preset::ALL {
        let mut graphics = GraphicsSettings::default();
        preset.apply(&mut graphics);
        assert_eq!(Preset::of(&graphics), Some(preset));
    }
}

#[test]
fn the_footer_counts_each_changed_setting_and_action_once() {
    let mut ui = settings_ui();
    assert_eq!(view::foot(&ui).pending_label(), "All changes saved");

    ui.edit(SettingsAction::Toggle(Setting::Vsync));
    assert_eq!(view::foot(&ui).pending_label(), "1 unsaved change");

    // The volume and the mute of one channel are one setting.
    AudioChannel::Sfx.set_volume(&mut ui.draft.audio, 0.1);
    ui.edit(SettingsAction::Mute(AudioChannel::Sfx));
    ui.capture(PlayerAction::Sit, BindSlot::Secondary, KeyBind::new("KeyZ"));
    assert_eq!(view::foot(&ui).pending_label(), "3 unsaved changes");

    let changed: Vec<_> = view::nav(&ui)
        .into_iter()
        .filter(|item| item.changed)
        .map(|item| item.tab)
        .collect();
    assert_eq!(
        changed,
        [
            SettingsTab::Graphics,
            SettingsTab::Sound,
            SettingsTab::Controls
        ]
    );
}

#[test]
fn resetting_a_section_leaves_the_others_alone() {
    let mut ui = settings_ui();
    ui.draft.graphics.fps_cap = FpsCap::F120;
    ui.draft.graphics.ui_scaling = UiScaling::P150;
    ui.draft.audio.bgm_volume = 0.1;
    assert!(view::head(&ui).can_reset);

    ui.edit(SettingsAction::ResetSection);

    assert_eq!(
        ui.draft.graphics.fps_cap,
        GraphicsSettings::default().fps_cap
    );
    assert_eq!(ui.draft.graphics.ui_scaling, UiScaling::P150);
    assert_eq!(ui.draft.audio.bgm_volume, 0.1);
    assert!(!view::head(&ui).can_reset);
    assert!(view::foot(&ui).toast.is_some());
}

#[test]
fn resetting_the_controls_restores_every_default_bind() {
    let mut ui = settings_ui();
    ui.edit(SettingsAction::Section(SettingsTab::Controls));
    ui.capture(PlayerAction::Sit, BindSlot::Primary, KeyBind::new("KeyZ"));
    assert!(view::head(&ui).can_reset);

    ui.edit(SettingsAction::ResetSection);
    assert_eq!(ui.draft.keybinds, Keybinds::default());
}

#[test]
fn percent_label_rounds_and_reports_muted() {
    assert_eq!(view::percent_label(0.55, false), "55%");
    assert_eq!(view::percent_label(0.0, false), "0%");
    assert_eq!(view::percent_label(1.0, false), "100%");
    assert_eq!(view::percent_label(0.854, false), "85%");
    assert_eq!(view::percent_label(0.7, true), "Muted");
}

// ── Key binds ─────────────────────────────────────────────────────────────

fn controls_ui() -> SettingsUi {
    let mut ui = settings_ui();
    ui.edit(SettingsAction::Section(SettingsTab::Controls));
    ui
}

#[test]
fn every_action_is_listed_under_its_group() {
    let ui = controls_ui();
    let rows = view::rows(&ui);
    let groups: Vec<_> = rows
        .iter()
        .filter_map(|row| match row {
            Row::Group(group) => Some(*group),
            _ => None,
        })
        .collect();
    assert_eq!(groups, ["WINDOWS", "ACTIONS", "HOTBAR"]);
    let actions = rows
        .iter()
        .filter(|row| matches!(row, Row::Bind { .. }))
        .count();
    assert_eq!(actions, 23);
    assert_eq!(rows.first(), Some(&Row::BindHeader));
    assert_eq!(rows.last(), Some(&Row::BindHint));
}

#[test]
fn a_key_cap_shows_its_bind_or_that_it_has_none() {
    let ui = controls_ui();
    let sit = bind_row(&ui, PlayerAction::Sit);
    assert_eq!(sit.keys[0], ("Insert".into(), KeyState::Bound));
    let status = bind_row(&ui, PlayerAction::Status);
    assert_eq!(status.keys[0], ("Alt + A".into(), KeyState::Bound));
    assert_eq!(status.keys[1], ("Not set".into(), KeyState::Empty));
}

#[test]
fn listening_shows_on_the_clicked_key_cap() {
    let mut ui = controls_ui();
    ui.edit(SettingsAction::Listen(
        PlayerAction::Guild,
        BindSlot::Secondary,
    ));

    let guild = bind_row(&ui, PlayerAction::Guild);
    assert_eq!(guild.keys[1].1, KeyState::Listening);
    assert_eq!(guild.keys[0].1, KeyState::Bound);
}

#[test]
fn a_free_key_is_bound_straight_away() {
    let mut ui = controls_ui();
    ui.listening = Some((PlayerAction::Guild, BindSlot::Secondary));
    ui.capture(
        PlayerAction::Guild,
        BindSlot::Secondary,
        KeyBind::new("KeyZ"),
    );

    assert!(ui.listening.is_none());
    assert!(ui.clash.is_none());
    let guild = bind_row(&ui, PlayerAction::Guild);
    assert_eq!(guild.keys[1], ("Z".into(), KeyState::Bound));
    assert!(guild.changed);
}

/// Presses Sit's default key for Guild's primary slot.
fn clashing_ui() -> SettingsUi {
    let mut ui = controls_ui();
    let taken = ui.draft.keybinds.sit.primary.clone();
    ui.capture(PlayerAction::Guild, BindSlot::Primary, taken);
    ui
}

#[test]
fn a_taken_key_raises_the_clash_instead_of_binding() {
    let ui = clashing_ui();

    assert_eq!(
        ui.draft.keybinds,
        Keybinds::default(),
        "nothing is bound yet"
    );
    let rows = view::rows(&ui);
    assert_eq!(
        rows[1],
        Row::Clash {
            key: "Insert".into(),
            other: "Sit / Stand".into()
        }
    );
    let guild = bind_row(&ui, PlayerAction::Guild);
    assert_eq!(guild.keys[0], ("Insert".into(), KeyState::Clash));
}

#[test]
fn rebinding_a_slot_to_its_own_key_is_no_clash() {
    let mut ui = controls_ui();
    let own = ui.draft.keybinds.sit.primary.clone();
    ui.capture(PlayerAction::Sit, BindSlot::Primary, own);
    assert!(ui.clash.is_none());
}

#[test]
fn keeping_the_old_bind_drops_the_clash() {
    let mut ui = clashing_ui();
    ui.edit(SettingsAction::KeepBind);

    assert!(ui.clash.is_none());
    assert_eq!(ui.draft.keybinds, Keybinds::default());
}

#[test]
fn swapping_trades_the_two_binds_and_says_so() {
    let mut ui = clashing_ui();
    let defaults = Keybinds::default();
    ui.edit(SettingsAction::SwapBinds);

    assert!(ui.clash.is_none());
    assert_eq!(ui.draft.keybinds.guild.primary, defaults.sit.primary);
    assert_eq!(ui.draft.keybinds.sit.primary, defaults.guild.primary);
    assert_eq!(
        view::foot(&ui).toast.as_deref(),
        Some("Guild is now Insert \u{b7} Sit / Stand moved to Alt + G")
    );
}

fn capture_app(ui: SettingsUi) -> App {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(ui);
    app.add_systems(Update, capture_rebind.run_if(listening_active));
    app
}

fn press(app: &mut App, keys: &[KeyCode]) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.reset_all();
    for key in keys {
        input.press(*key);
    }
    app.update();
}

fn listening_app() -> App {
    let mut ui = controls_ui();
    ui.edit(SettingsAction::Listen(PlayerAction::Sit, BindSlot::Primary));
    capture_app(ui)
}

#[test]
fn a_pressed_chord_is_captured_and_consumed() {
    let mut app = listening_app();
    press(&mut app, &[KeyCode::AltLeft]);
    assert!(
        app.world().resource::<SettingsUi>().listening.is_some(),
        "a modifier alone is not a bind"
    );

    press(&mut app, &[KeyCode::AltLeft, KeyCode::KeyZ]);
    let ui = app.world().resource::<SettingsUi>();
    assert!(ui.listening.is_none());
    assert_eq!(
        ui.draft.keybinds.sit.primary,
        KeyBind::modified(Modifier::Alt, "KeyZ")
    );
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    assert!(!keys.just_pressed(KeyCode::KeyZ), "the key does not leak");
}

#[test]
fn escape_cancels_the_capture_and_backspace_clears_the_slot() {
    let mut app = listening_app();
    press(&mut app, &[KeyCode::Escape]);
    let ui = app.world().resource::<SettingsUi>();
    assert!(ui.listening.is_none());
    assert_eq!(ui.draft.keybinds, Keybinds::default());
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    assert!(
        !keys.just_pressed(KeyCode::Escape),
        "Escape does not reach the escape menu"
    );

    let mut app = listening_app();
    press(&mut app, &[KeyCode::Backspace]);
    let ui = app.world().resource::<SettingsUi>();
    assert!(ui.listening.is_none());
    assert_eq!(ui.draft.keybinds.sit.primary, KeyBind::UNBOUND);
}

/// A plain captured key, once stored and built into an `InputMap` (which parses the
/// stored name back via `key_code_from_name`), maps to the same `KeyCode` a
/// directly-built map produces. Guards the round-trip.
#[test]
fn plain_capture_round_trips_through_the_input_map() {
    let binds = Keybinds {
        sit: ActionBinds {
            primary: keybind_from_capture(Modifier::None, KeyCode::Insert),
            secondary: KeyBind::UNBOUND,
        },
        status: ActionBinds::default(),
        inventory: ActionBinds::default(),
        skills: ActionBinds::default(),
        equipment: ActionBinds::default(),
        cart: ActionBinds::default(),
        party: ActionBinds::default(),
        guild: ActionBinds::default(),
        emote: ActionBinds::default(),
        names: ActionBinds::default(),
        chat_room: ActionBinds::default(),
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
            primary: keybind_from_capture(Modifier::Alt, KeyCode::KeyA),
            secondary: KeyBind::UNBOUND,
        },
        inventory: ActionBinds::default(),
        skills: ActionBinds::default(),
        equipment: ActionBinds::default(),
        cart: ActionBinds::default(),
        party: ActionBinds::default(),
        guild: ActionBinds::default(),
        emote: ActionBinds::default(),
        names: ActionBinds::default(),
        chat_room: ActionBinds::default(),
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
    assert_eq!(
        keybind_from_capture(Modifier::None, KeyCode::KeyA).key,
        "KeyA"
    );
    assert_eq!(
        keybind_from_capture(Modifier::None, KeyCode::Insert).key,
        "Insert"
    );
    let chord = keybind_from_capture(Modifier::Alt, KeyCode::KeyE);
    assert_eq!(chord.key, "KeyE");
    assert_eq!(chord.modifier, Modifier::Alt);
}

#[test]
fn keycap_label_renders_keys_chords_and_empty() {
    assert_eq!(keycap_label(&KeyBind::new("Insert")), "Insert");
    assert_eq!(keycap_label(&KeyBind::new("KeyA")), "A");
    assert_eq!(keycap_label(&KeyBind::new("Digit1")), "1");
    assert_eq!(
        keycap_label(&KeyBind::modified(Modifier::Alt, "KeyA")),
        "Alt + A"
    );
    assert_eq!(
        keycap_label(&KeyBind::modified(Modifier::Control, "KeyE")),
        "Ctrl + E"
    );
    assert_eq!(keycap_label(&KeyBind::UNBOUND), "—");
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

// ── Applying ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
struct ChangedGroups {
    graphics: bool,
    audio: bool,
    keybinds: bool,
}

fn record_changed_groups(
    graphics: Res<GraphicsSettings>,
    audio: Res<AudioConfig>,
    keybinds: Res<Keybinds>,
    mut changed: ResMut<ChangedGroups>,
) {
    changed.graphics = graphics.is_changed();
    changed.audio = audio.is_changed();
    changed.keybinds = keybinds.is_changed();
}

/// An app holding the settings resources, with a button for `action`.
fn desk_app(ui: SettingsUi, action: SettingsAction) -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(GraphicsSettings::default());
    app.insert_resource(AudioConfig::default());
    app.insert_resource(Keybinds::default());
    app.insert_resource(ui);
    app.init_resource::<ChangedGroups>();
    app.add_message::<ApplySettings>();
    app.add_systems(Update, record_changed_groups);
    let button = app
        .world_mut()
        .spawn(action)
        .observe(on_settings_button)
        .id();
    app.update();
    app.world_mut().clear_trackers();
    (app, button)
}

fn activate(app: &mut App, button: Entity) {
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
    app.update();
}

#[test]
fn applying_a_dirty_draft_commits_it_and_asks_for_one_live_apply() {
    let (mut app, button) = desk_app(dirtied_ui(), SettingsAction::Apply);
    activate(&mut app, button);

    assert_eq!(
        app.world().resource::<GraphicsSettings>().fps_cap,
        FpsCap::F120
    );
    let ui = app.world().resource::<SettingsUi>();
    assert!(!ui.dirty());
    assert_eq!(view::foot(ui).toast.as_deref(), Some("Settings applied"));
    assert_eq!(app.world().resource::<Messages<ApplySettings>>().len(), 1);
    let changed = app.world().resource::<ChangedGroups>();
    assert!(changed.graphics);
    assert!(!changed.audio, "an unchanged group is not marked changed");
    assert!(!changed.keybinds);
}

#[test]
fn applying_a_clean_draft_does_nothing() {
    let (mut app, button) = desk_app(settings_ui(), SettingsAction::Apply);
    activate(&mut app, button);

    assert_eq!(
        app.world().resource::<GraphicsSettings>().fps_cap,
        FpsCap::F60
    );
    assert!(app.world().resource::<Messages<ApplySettings>>().is_empty());
    assert!(
        view::foot(app.world().resource::<SettingsUi>())
            .toast
            .is_none()
    );
    let changed = app.world().resource::<ChangedGroups>();
    assert!(!changed.graphics && !changed.audio && !changed.keybinds);
}

#[test]
fn cancel_discards_the_draft_without_touching_the_live_settings() {
    let (mut app, button) = desk_app(dirtied_ui(), SettingsAction::Cancel);
    activate(&mut app, button);

    let ui = app.world().resource::<SettingsUi>();
    assert!(!ui.dirty());
    assert_eq!(view::foot(ui).toast.as_deref(), Some("Changes discarded"));
    assert_eq!(
        app.world().resource::<GraphicsSettings>().fps_cap,
        FpsCap::F60
    );
    assert!(app.world().resource::<Messages<ApplySettings>>().is_empty());
}

#[test]
fn the_close_button_hides_the_window() {
    let (mut app, button) = desk_app(settings_ui(), SettingsAction::Close);
    let window = app
        .world_mut()
        .spawn((SettingsWindowRoot, Visibility::Visible))
        .id();
    activate(&mut app, button);

    assert_eq!(
        app.world().entity(window).get::<Visibility>(),
        Some(&Visibility::Hidden)
    );
}

// ── The window ────────────────────────────────────────────────────────────

fn window_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<GraphicsSettings>();
    app.init_resource::<AudioConfig>();
    app.init_resource::<Keybinds>();
    app.init_resource::<SettingsUi>();
    app.add_systems(Startup, spawn_settings_root);
    app.add_systems(Update, (end_drag, rebuild, sync_sliders).chain());
    app.update();
    app.update();
    app
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

fn shows(app: &mut App, text: &str) -> bool {
    texts(app).iter().any(|shown| shown == text)
}

/// Spawns the entire window tree so a malformed scene panics here instead of only
/// at runtime, and checks every region was drawn.
#[test]
fn the_window_is_spawned_once_with_every_region_drawn() {
    let mut app = window_app();

    let world = app.world_mut();
    let roots = world
        .query_filtered::<(), With<SettingsWindowRoot>>()
        .iter(world)
        .count();
    assert_eq!(roots, 1);
    let drawn = world
        .query::<(&SettingsHost, &Children)>()
        .iter(world)
        .count();
    assert_eq!(
        drawn, 4,
        "the rail, heading, rows and footer all have content"
    );
    for text in [
        "Graphics",
        "How the world looks",
        "QUALITY PRESET",
        "Display Mode",
        "Reset All to Defaults",
        "All changes saved",
    ] {
        assert!(shows(&mut app, text), "{text}");
    }
}

#[test]
fn picking_a_section_redraws_the_heading_and_the_rows() {
    let mut app = window_app();
    app.world_mut()
        .resource_mut::<SettingsUi>()
        .edit(SettingsAction::Section(SettingsTab::Controls));
    app.update();
    app.update();

    assert!(shows(&mut app, "Keys for windows and actions"));
    assert!(shows(&mut app, "Sit / Stand"));
    assert!(!shows(&mut app, "Display Mode"));
}

fn sound_app() -> App {
    let mut app = window_app();
    app.world_mut()
        .resource_mut::<SettingsUi>()
        .edit(SettingsAction::Section(SettingsTab::Sound));
    app.update();
    app.update();
    app
}

fn rail(app: &mut App, channel: AudioChannel) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &SliderRail)>()
        .iter(world)
        .find(|(_, rail)| rail.0 == channel)
        .map(|(entity, _)| entity)
        .expect("the channel has a slider")
}

/// Points at `at` along the rail, from 0 at its left edge to 1 at its right.
fn point_at(app: &mut App, rail: Entity, at: f32) {
    // `normalized` is centred on the node.
    app.world_mut()
        .entity_mut(rail)
        .insert(RelativeCursorPosition {
            normalized: Some(Vec2::new(at - 0.5, 0.0)),
            ..default()
        });
    app.world_mut()
        .run_system_once(
            move |rails: Query<(&SliderRail, &RelativeCursorPosition)>,
                  mut ui: ResMut<SettingsUi>| {
                set_volume_from_cursor(rail, &rails, &mut ui);
            },
        )
        .unwrap();
}

#[test]
fn the_slider_sets_the_volume_from_where_the_pointer_is() {
    let mut app = sound_app();
    let rail = rail(&mut app, AudioChannel::Sfx);

    point_at(&mut app, rail, 0.25);
    assert_eq!(
        app.world().resource::<SettingsUi>().draft.audio.sfx_volume,
        0.25
    );

    point_at(&mut app, rail, 1.3);
    assert_eq!(
        app.world().resource::<SettingsUi>().draft.audio.sfx_volume,
        1.0
    );
}

#[test]
fn a_held_slider_is_written_in_place_and_redrawn_only_once_let_go() {
    let mut app = sound_app();
    let held = rail(&mut app, AudioChannel::Bgm);
    let mut mouse = ButtonInput::<MouseButton>::default();
    mouse.press(MouseButton::Left);
    app.insert_resource(mouse);

    point_at(&mut app, held, 0.1);
    app.update();
    assert_eq!(
        rail(&mut app, AudioChannel::Bgm),
        held,
        "the rail survives while it is held"
    );
    assert!(shows(&mut app, "10%"), "the readout follows the drag");

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    app.update();
    assert_ne!(rail(&mut app, AudioChannel::Bgm), held);
    assert!(shows(&mut app, "1 unsaved change"));
}

#[test]
fn muting_a_channel_empties_its_slider_and_says_so() {
    let mut app = sound_app();
    app.world_mut()
        .resource_mut::<SettingsUi>()
        .edit(SettingsAction::Mute(AudioChannel::Ambient));
    app.update();
    app.update();

    assert!(shows(&mut app, "Muted"));
    let world = app.world_mut();
    let fill = world
        .query::<(&SliderPart, &SliderOf, &Node)>()
        .iter(world)
        .find(|(part, of, _)| matches!(part, SliderPart::Fill) && of.0 == AudioChannel::Ambient)
        .map(|(_, _, node)| node.width)
        .unwrap();
    assert_eq!(fill, Val::Percent(0.0));
}

#[test]
fn the_names_key_flips_the_live_setting_and_keeps_the_draft_clean() {
    let (mut app, _) = desk_app(settings_ui(), SettingsAction::Close);
    app.add_systems(Update, toggle_names_key);
    let player = app
        .world_mut()
        .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
        .id();
    app.world_mut()
        .entity_mut(player)
        .get_mut::<ActionState<PlayerAction>>()
        .unwrap()
        .press(&PlayerAction::Names);
    app.update();

    assert!(app.world().resource::<GraphicsSettings>().show_names);
    let ui = app.world().resource::<SettingsUi>();
    assert!(ui.draft.graphics.show_names);
    assert!(!ui.dirty(), "the next Apply must not undo the key");
}
