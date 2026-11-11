use super::*;
use net_contract::dto::{ServerInfo, ServerType};
use net_contract::state::SessionTokens;

#[test]
fn status_buckets_by_fill_ratio() {
    assert_eq!(server_status(0.10), ServerStatus::Online);
    assert_eq!(server_status(0.60), ServerStatus::High);
    assert_eq!(server_status(0.95), ServerStatus::Full);
}

#[test]
fn fill_ratio_clamps_to_unit_range() {
    assert_eq!(fill_ratio(0), 0.0);
    assert!((fill_ratio(POP_SOFT_CAP) - 1.0).abs() < f32::EPSILON);
    assert_eq!(fill_ratio(u16::MAX), 1.0); // clamped
}

#[test]
fn status_label_maps_each_variant() {
    assert_eq!(status_label(ServerStatus::Online), "Online");
    assert_eq!(status_label(ServerStatus::High), "Busy");
    assert_eq!(status_label(ServerStatus::Full), "Full");
}

#[test]
fn pop_word_buckets() {
    assert_eq!(pop_word(0.0), "Low pop.");
    assert_eq!(pop_word(0.4), "Healthy pop.");
    assert_eq!(pop_word(0.7), "High pop.");
}

fn server(name: &str, users: u16) -> ServerInfo {
    ServerInfo {
        ip: 0,
        port: 0,
        name: name.to_string(),
        users,
        server_type: ServerType::Normal,
        new_server: 0,
        max_users: 0,
        region: String::new(),
        description: String::new(),
        heroes: Vec::new(),
        rates: None,
    }
}

fn user_session(servers: Vec<ServerInfo>) -> UserSession {
    UserSession {
        username: "tester".to_string(),
        tokens: SessionTokens {
            login_id1: 0,
            account_id: 0,
            login_id2: 0,
            character_server_info: None,
        },
        login_timestamp: std::time::SystemTime::UNIX_EPOCH,
        last_login_ip: 0,
        sex: 0,
        server_list: servers,
        selected_server: None,
        auth_token: String::new(),
        server_time: 0,
    }
}

fn server_app(session: UserSession) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
    ));
    app.init_asset::<Font>();
    app.add_message::<ServerSelectedEvent>();
    app.init_resource::<ServerListPopulated>();
    app.insert_resource(session);
    app.world_mut().spawn(ServerList);
    app.add_systems(Update, populate_server_list);
    app
}

fn row_count(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<ServerRow>>()
        .iter(app.world())
        .count()
}

#[test]
fn populate_spawns_one_row_per_server() {
    let mut app = server_app(user_session(vec![
        server("Valhalla", 7),
        server("Asgard", 3),
    ]));

    app.update();

    assert_eq!(row_count(&mut app), 2);

    let names: Vec<String> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect();
    assert!(names.iter().any(|n| n == "Valhalla"));
    assert!(names.iter().any(|n| n == "Asgard"));
    assert!(app.world().resource::<ServerListPopulated>().0);
}

#[test]
fn populate_is_idempotent_across_frames() {
    let mut app = server_app(user_session(vec![server("Valhalla", 7)]));

    app.update();
    app.update();
    app.update();

    assert_eq!(row_count(&mut app), 1);
}

#[test]
fn maintenance_is_offline_whatever_the_population() {
    let mut closed = server("Alfheim", POP_SOFT_CAP);
    closed.server_type = ServerType::Maintenance;
    assert_eq!(status_of(&closed), ServerStatus::Offline);
    assert_eq!(
        status_of(&server("Valhalla", POP_SOFT_CAP)),
        ServerStatus::Full
    );
}

#[test]
fn reported_capacity_replaces_the_soft_cap() {
    let mut world = server("Valhalla", 950);
    assert_eq!(status_of(&world), ServerStatus::Online);
    world.max_users = 1000;
    assert_eq!(status_of(&world), ServerStatus::Full);
}

#[test]
fn thousands_groups_digits() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(999), "999");
    assert_eq!(thousands(4220), "4,220");
    assert_eq!(thousands(1_310_700), "1,310,700");
}

#[test]
fn step_selection_clamps_to_the_list() {
    assert_eq!(step_selection(0, -1, 3), 0);
    assert_eq!(step_selection(0, 1, 3), 1);
    assert_eq!(step_selection(2, 1, 3), 2);
    assert_eq!(step_selection(0, 1, 0), 0);
}

#[test]
fn rates_label_shows_multipliers() {
    let rates = ServerRates {
        base_exp: 300,
        job_exp: 150,
        drop: 100,
    };
    assert_eq!(rates_label(rates), "3× · 1.5× · 1×");
}

#[test]
fn ago_uses_the_largest_whole_unit() {
    assert_eq!(ago(59), "just now");
    assert_eq!(ago(60), "1 minute ago");
    assert_eq!(ago(7_200), "2 hours ago");
    assert_eq!(ago(86_400 * 12), "12 days ago");
    assert_eq!(ago(86_400 * 45), "1 month ago");
}

fn hero(name: &str, last_login: u64) -> HeroSummary {
    HeroSummary {
        name: name.into(),
        class: 0,
        base_level: 99,
        last_login,
    }
}

#[test]
fn last_played_names_the_most_recent_hero() {
    let heroes = [hero("Sigyn", 0), hero("Ygor", 9_000), hero("Tove", 5_000)];

    assert_eq!(
        last_played(&heroes, Some(16_200)).as_deref(),
        Some("Last played 2 hours ago with Ygor")
    );
    assert_eq!(last_played(&heroes, None), None);
    assert_eq!(last_played(&[hero("Sigyn", 0)], Some(16_200)), None);
}

#[test]
fn clock_label_wraps_to_the_utc_day() {
    assert_eq!(clock_label(1_794_430_680), "Server time 20:58");
    assert_eq!(clock_label(86_400 + 65), "Server time 00:01");
}

fn screen_app(servers: Vec<ServerInfo>) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
    ));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.add_message::<ServerSelectedEvent>();
    app.add_message::<ShowSystemDialog>();
    app.init_resource::<ServerListPopulated>();
    app.init_resource::<SelectedServer>();
    app.insert_resource(user_session(servers));
    app.add_observer(enter_world);
    app.add_systems(Startup, show_server_select_screen);
    app.add_systems(
        Update,
        (
            populate_server_list,
            refresh_selection,
            tick_connecting,
            tick_clock,
        ),
    );
    app.update();
    app.update();
    app
}

fn texts(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect()
}

#[test]
fn screen_shows_rows_details_and_enter_label_for_the_selection() {
    let mut valhalla = server("Valhalla", 4220);
    valhalla.region = "EU West".into();
    valhalla.description = "The main world.".into();
    valhalla.heroes = vec![hero("Ygor", 0)];
    valhalla.rates = Some(ServerRates {
        base_exp: 300,
        job_exp: 300,
        drop: 200,
    });
    let mut app = screen_app(vec![valhalla, server("Asgard", 3)]);

    assert_eq!(row_count(&mut app), 2);
    let shown = texts(&mut app);
    for expected in [
        "4,223 heroes online",
        "tester",
        "1 hero · EU West",
        "No heroes · Low pop.",
        "The main world.",
        "4,220 · Busy",
        "3× · 3× · 2×",
        "Ygor",
        "Lv 99",
        "Enter Valhalla",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }

    app.world_mut().resource_mut::<SelectedServer>().0 = 1;
    app.update();
    app.update();

    let shown = texts(&mut app);
    assert!(shown.iter().any(|t| t == "Make a Hero"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "No heroes yet"));
    assert!(!shown.iter().any(|t| t == "Lv 99"));
}

#[test]
fn screen_shows_the_server_clock_and_last_played_once_the_time_is_known() {
    let mut valhalla = server("Valhalla", 7);
    valhalla.heroes = vec![hero("Ygor", 1_794_430_680 - 7_200)];
    let mut app = screen_app(vec![valhalla]);
    let shown = texts(&mut app);
    assert!(!shown.iter().any(|t| t.starts_with("Server time")));
    assert!(!shown.iter().any(|t| t.starts_with("Last played")));

    let mut session = app.world_mut().resource_mut::<UserSession>();
    session.server_time = 1_794_430_680;
    session.login_timestamp = std::time::SystemTime::now();
    app.world_mut().resource_mut::<SelectedServer>().0 = 0;
    app.update();
    app.update();

    let shown = texts(&mut app);
    for expected in ["Server time 20:58", "Last played 2 hours ago with Ygor"] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
}

#[test]
fn entering_selects_the_server_and_times_out_into_a_dialog() {
    let mut app = screen_app(vec![server("Valhalla", 7)]);

    app.world_mut().trigger(EnterWorld);
    app.update();
    app.world_mut().trigger(EnterWorld);
    app.update();

    let selected = app.world().resource::<Messages<ServerSelectedEvent>>();
    assert_eq!(selected.len(), 1);
    let mut modals = app.world_mut().query::<&mut ConnectingModal>();
    let mut modal = modals.single_mut(app.world_mut()).unwrap();
    let budget = modal.timer.duration();
    modal
        .timer
        .set_elapsed(budget - std::time::Duration::from_nanos(1));

    app.update();
    app.update();

    assert_eq!(modals.iter(app.world()).count(), 0);
    let dialogs = app.world().resource::<Messages<ShowSystemDialog>>();
    assert_eq!(dialogs.len(), 1);
}

#[test]
fn a_closed_world_cannot_be_entered() {
    let mut closed = server("Alfheim", 0);
    closed.server_type = ServerType::Maintenance;
    let mut app = screen_app(vec![closed]);

    app.world_mut().trigger(EnterWorld);
    app.update();

    assert!(texts(&mut app).iter().any(|t| t == "Closed for Update"));
    assert!(
        app.world()
            .resource::<Messages<ServerSelectedEvent>>()
            .is_empty()
    );
}
