use super::view::{TipView, percent};
use super::*;
use lifthrasir_client::domain::entities::character::components::core::CharacterStats;

fn hero(job_id: u16, status: CharacterStatus) -> impl Bundle {
    (
        status,
        CharacterData {
            name: "Hero".to_string(),
            job_id,
            level: 1,
            experience: 0,
            stats: CharacterStats::default(),
            slot: 0,
        },
        LocalPlayer,
    )
}

fn refreshed(job_id: u16, status: CharacterStatus) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_systems(Update, refresh);
    app.world_mut().spawn(hero(job_id, status));
    app
}

fn field(app: &mut App, field: Field) -> Entity {
    app.world_mut()
        .spawn((field, Text::new(""), TextColor::default()))
        .id()
}

fn fill(app: &mut App, gauge: Gauge) -> Entity {
    app.world_mut()
        .spawn((Fill(gauge), Node::default(), BackgroundColor::default()))
        .id()
}

fn text(app: &App, entity: Entity) -> &str {
    &app.world().get::<Text>(entity).unwrap().0
}

#[test]
fn percent_handles_empty_and_overfull_limits() {
    assert_eq!(percent(50, 100), 50.0);
    assert_eq!(percent(0, 0), 0.0);
    assert_eq!(percent(150, 100), 100.0);
}

#[test]
fn hud_settings_use_the_persisted_hud_group() {
    assert_eq!(HudSettings::settings_group_name(), "hud");
    assert_eq!(HudSettings::settings_source(), None);
    assert!(!HudSettings::default().character_info_minimal);
}

#[test]
fn bars_and_their_numbers_follow_the_status() {
    let mut app = refreshed(
        0,
        CharacterStatus {
            hp: 24_105,
            max_hp: 48_210,
            sp: 365,
            max_sp: 1_460,
            ..default()
        },
    );
    let hp_fill = fill(&mut app, Gauge::Hp);
    let hp = field(&mut app, Field::Value(Gauge::Hp));
    let hp_max = field(&mut app, Field::Max(Gauge::Hp));
    let sp_short = field(&mut app, Field::Short(Gauge::Sp));

    app.update();

    assert_eq!(
        app.world().get::<Node>(hp_fill).unwrap().width,
        Val::Percent(50.0)
    );
    assert_eq!(text(&app, hp), "24,105");
    assert_eq!(text(&app, hp_max), "/ 48,210");
    assert_eq!(text(&app, sp_short), "25%");
}

#[test]
fn ap_appears_only_for_a_fourth_job_with_an_ap_pool() {
    let mut app = refreshed(
        4001,
        CharacterStatus {
            ap: 50,
            max_ap: 100,
            ..default()
        },
    );
    let ap_fill = fill(&mut app, Gauge::Ap);
    let short = field(&mut app, Field::Short(Gauge::Ap));
    let hidden = Node {
        display: Display::None,
        ..default()
    };
    let row = app.world_mut().spawn((hidden, ApRow)).id();

    app.update();
    assert_eq!(app.world().get::<Node>(row).unwrap().display, Display::None);

    let mut players = app
        .world_mut()
        .query_filtered::<&mut CharacterData, With<LocalPlayer>>();
    players.single_mut(app.world_mut()).unwrap().job_id = 4252;
    app.update();

    assert_eq!(app.world().get::<Node>(row).unwrap().display, Display::Flex);
    assert_eq!(
        app.world().get::<Node>(ap_fill).unwrap().width,
        Val::Percent(50.0)
    );
    assert_eq!(text(&app, short), "50");
}

#[test]
fn zeny_is_shortened_and_weight_is_toned_by_its_tier() {
    let mut app = refreshed(
        0,
        CharacterStatus {
            zeny: 12_480_300,
            weight: 1_600,
            max_weight: 3_100,
            ..default()
        },
    );
    let zeny = field(&mut app, Field::Zeny);
    let weight = field(&mut app, Field::Value(Gauge::Weight));
    let share = field(&mut app, Field::Short(Gauge::Weight));
    let weight_fill = fill(&mut app, Gauge::Weight);

    app.update();

    assert_eq!(text(&app, zeny), "12.5m z");
    assert_eq!(text(&app, weight), "1,600 / 3,100");
    assert_eq!(text(&app, share), "51%");
    let amber = ro_dark::STATUS_BUSY;
    assert_eq!(app.world().get::<TextColor>(share).unwrap().0, amber);
    assert_eq!(
        app.world().get::<BackgroundColor>(weight_fill).unwrap().0,
        amber
    );
}

#[test]
fn zeny_keeps_every_digit_until_it_runs_out_of_room() {
    let zeny = |zeny| StatusView { zeny, ..default() }.text(Field::Zeny);
    assert_eq!(zeny(3_420), "3,420 z");
    assert_eq!(zeny(48_210), "48k z");
    assert_eq!(zeny(1_250_000), "1.25m z");
    assert_eq!(zeny(10_000_000), "10m z");
}

#[test]
fn tooltips_spell_out_what_the_plate_abbreviates() {
    let view = StatusView {
        job: "Dragon Knight".into(),
        base_level: 215,
        hp: (300, 1_000),
        base_exp: (624, 1_000),
        weight: (1_600, 3_100),
        zeny: 12_480_300,
        ..default()
    };

    assert_eq!(view.tip(Tip::Zeny).title, "12,480,300 z");
    assert_eq!(
        view.tip(Tip::Gauge(Gauge::Hp)),
        TipView {
            title: "HP 300 / 1,000".into(),
            tone: Gauge::Hp.ink(),
            lines: vec!["30% \u{b7} Regenerates while sitting (stopped: overweight)".into()],
        }
    );
    assert_eq!(
        view.tip(Tip::Gauge(Gauge::BaseExp)).lines,
        ["Level 215 \u{2192} 216", "37.6% to go"]
    );
    assert_eq!(
        view.tip(Tip::Gauge(Gauge::Weight)).lines,
        [
            "Over 50%: HP and SP stop regenerating",
            "1,190 until the next limit"
        ]
    );
}

#[test]
fn only_a_real_climb_is_a_level_up() {
    assert_eq!(
        LevelUp::between((98, 49), (99, 49)),
        Some(LevelUp::Base(99))
    );
    assert_eq!(LevelUp::between((99, 49), (99, 50)), Some(LevelUp::Job(50)));
    assert_eq!(LevelUp::between((99, 50), (99, 50)), None);
    // A job change resets the job level, and a status not yet filled in reads zero.
    assert_eq!(LevelUp::between((99, 50), (99, 1)), None);
    assert_eq!(LevelUp::between((0, 0), (99, 50)), None);
}

#[test]
fn a_spent_bar_leaves_a_ghost_that_holds_then_drains() {
    let full = Trail {
        gauge: Gauge::Hp,
        shown: 80.0,
        hold: TRAIL_HOLD,
    };
    // The drop is held first, then drained, and never past the bar itself.
    assert_eq!(step_trail(&full, 40.0, 0.1), (80.0, TRAIL_HOLD - 0.1));
    let draining = Trail { hold: 0.0, ..full };
    assert_eq!(step_trail(&draining, 40.0, 0.5), (50.0, 0.0));
    assert_eq!(step_trail(&draining, 40.0, 5.0), (40.0, 0.0));
    // Healing takes the ghost along at once.
    assert_eq!(step_trail(&draining, 90.0, 0.1), (90.0, TRAIL_HOLD));
}

#[test]
fn the_banner_fades_in_and_out() {
    assert_eq!(banner_alpha(0.0), 0.0);
    assert_eq!(banner_alpha(BANNER_SECS / 2.0), 1.0);
    assert_eq!(banner_alpha(BANNER_SECS), 0.0);
}

#[test]
fn toggling_the_size_swaps_the_plate_and_lays_the_corner_out() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Font>()
    .init_resource::<HudSettings>()
    .add_systems(Update, rebuild_plate.run_if(size_changed));
    let hud = app.world_mut().spawn(Node::default()).id();
    spawn_status_bar(&mut app.world_mut().commands(), hud);
    // One update mounts the corner, the next one its plate.
    app.update();
    app.update();

    let count = |app: &mut App, field: Field| {
        let mut fields = app.world_mut().query::<&Field>();
        fields
            .iter(app.world())
            .filter(|shown| **shown == field)
            .count()
    };
    let mut roots = app
        .world_mut()
        .query_filtered::<&Node, With<StatusBarRoot>>();
    let mut plates = app.world_mut().query_filtered::<(), With<Plate>>();
    let mut tips = app.world_mut().query_filtered::<(), With<StatusTip>>();

    assert_eq!(
        roots.single(app.world()).unwrap().flex_direction,
        FlexDirection::Column
    );
    assert_eq!(count(&mut app, Field::Zeny), 1);
    assert_eq!(count(&mut app, Field::Short(Gauge::Hp)), 0);

    app.world_mut().spawn(StatusTip);
    app.world_mut()
        .resource_mut::<HudSettings>()
        .character_info_minimal = true;
    app.update();
    app.update();

    assert_eq!(
        roots.single(app.world()).unwrap().flex_direction,
        FlexDirection::Row
    );
    assert_eq!(plates.iter(app.world()).count(), 1);
    assert_eq!(tips.iter(app.world()).count(), 0);
    assert_eq!(count(&mut app, Field::Zeny), 0);
    assert_eq!(count(&mut app, Field::Short(Gauge::Hp)), 1);
}
