//! Server selection screen.
//!
//! A raw `bevy_ui` panel with a server-list container; the rows are spawned at runtime
//! under it, styled from [`theme`]. Each row shows a flag badge, name + server type,
//! online count, a status dot, and a population bar. Clicking a row writes
//! `ServerSelectedEvent`; the engine connects to the char server and drives the
//! transition.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::infrastructure::networking::server_info::ServerInfo;
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::ServerSelectedEvent;

use crate::theme::{self, label};

/// Online-population bucket for a server row's status pill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ServerStatus {
    Online,
    High,
    Full,
}

// NOTE: ServerInfo has no capacity field, only `users`. Assume a soft cap for
// the population bar/status. Replace with a real capacity if the protocol gains one.
const POP_SOFT_CAP: u16 = 6000;

fn fill_ratio(users: u16) -> f32 {
    (users as f32 / POP_SOFT_CAP as f32).clamp(0.0, 1.0)
}

fn server_status(ratio: f32) -> ServerStatus {
    if ratio >= 0.9 {
        ServerStatus::Full
    } else if ratio >= 0.6 {
        ServerStatus::High
    } else {
        ServerStatus::Online
    }
}

/// Display label for a server status pill.
fn status_label(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Online => "Online",
        ServerStatus::High => "Busy",
        ServerStatus::Full => "Full",
    }
}

/// Accent color for a status (dot, label, and population fill).
fn status_color(status: ServerStatus) -> Color {
    match status {
        ServerStatus::Online => theme::EMERALD,
        ServerStatus::High => theme::WARN,
        ServerStatus::Full => theme::BAD,
    }
}

/// Coarse population word shown beside the bar.
fn pop_word(ratio: f32) -> &'static str {
    if ratio >= 0.6 {
        "High pop."
    } else if ratio >= 0.3 {
        "Healthy pop."
    } else {
        "Low pop."
    }
}

pub struct ServerSelectScreenPlugin;

impl Plugin for ServerSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerListPopulated>();
        app.add_systems(
            OnEnter(GameState::ServerSelection),
            show_server_select_screen,
        );
        app.add_systems(
            Update,
            populate_server_list.run_if(in_state(GameState::ServerSelection)),
        );
    }
}

/// Guards the one-shot server-row spawn. Reset on every screen entry so re-entering
/// `ServerSelection` repopulates a fresh tree.
#[derive(Resource, Default)]
struct ServerListPopulated(bool);

/// Marks a runtime-spawned server row so they can be counted / cleared.
#[derive(Component)]
struct ServerRow;

/// Marks the container that holds the runtime-spawned server rows.
#[derive(Component)]
struct ServerList;

fn show_server_select_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut populated: ResMut<ServerListPopulated>,
) {
    populated.0 = false;

    let font_title = asset_server.load(theme::FONT_TITLE);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            DespawnOnExit(GameState::ServerSelection),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(480.0),
                padding: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(12.0), Val::Px(30.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        Text::new("Select Server"),
        TextFont {
            font: font_title,
            font_size: 25.0,
            ..default()
        },
        TextColor(theme::DISPLAY_GOLD),
        Node {
            margin: UiRect::bottom(Val::Px(16.0)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(panel),
    ));

    commands.spawn((
        ServerList,
        Node {
            width: Val::Px(416.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        ChildOf(panel),
    ));
}

/// Spawns one rich, clickable server row per server under the list container once the
/// session's server list is available. Rows are raw `bevy_ui` for full layout control
/// over the population bar and alignment.
fn populate_server_list(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut populated: ResMut<ServerListPopulated>,
    container: Query<Entity, With<ServerList>>,
    session: Option<Res<UserSession>>,
) {
    if populated.0 {
        return;
    }
    let Some(session) = session else {
        return;
    };
    let Ok(container) = container.single() else {
        return;
    };

    let font_body = asset_server.load(theme::FONT_BODY);
    let font_bold = asset_server.load(theme::FONT_BODY_BOLD);

    for server in session.server_list.iter() {
        spawn_server_row(
            &mut commands,
            container,
            server,
            font_body.clone(),
            font_bold.clone(),
        );
    }

    populated.0 = true;
}

/// Builds one server row matching the mockup: flag badge + name/type on the left,
/// online count + status dot on the right, and a population bar spanning the bottom.
fn spawn_server_row(
    commands: &mut Commands,
    container: Entity,
    server: &ServerInfo,
    font_body: Handle<Font>,
    font_bold: Handle<Font>,
) {
    let ratio = fill_ratio(server.users);
    let status = server_status(ratio);
    let color = status_color(status);
    let glyph = server
        .name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let subtitle = format!("{:?}", server.server_type).to_uppercase();
    let server_clone = server.clone();

    let row = commands
        .spawn((
            ServerRow,
            Pickable::default(),
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(14.0)),
                margin: UiRect::bottom(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::STROKE),
            ChildOf(container),
        ))
        .id();
    commands.entity(row).observe(
        move |_: On<Pointer<Click>>, mut writer: MessageWriter<ServerSelectedEvent>| {
            writer.write(ServerSelectedEvent {
                server: server_clone.clone(),
            });
        },
    );
    // Green border on hover (children are Pickable::IGNORE, so the row is the single
    // hover target — Over/Out fire once for the whole row).
    commands
        .entity(row)
        .observe(
            move |_: On<Pointer<Over>>, mut borders: Query<&mut BorderColor>| {
                if let Ok(mut border) = borders.get_mut(row) {
                    *border = BorderColor::all(theme::EMERALD);
                }
            },
        )
        .observe(
            move |_: On<Pointer<Out>>, mut borders: Query<&mut BorderColor>| {
                if let Ok(mut border) = borders.get_mut(row) {
                    *border = BorderColor::all(theme::STROKE);
                }
            },
        );

    // ---- top sub-row: id (left) + meta (right) ----
    let top = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();

    let id_group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(11.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(top),
        ))
        .id();
    let flag = commands
        .spawn((
            Node {
                width: Val::Px(30.0),
                height: Val::Px(30.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(id_group),
        ))
        .id();
    commands.spawn((
        label(glyph, font_bold.clone(), 14.0, theme::GOLD),
        ChildOf(flag),
    ));

    let name_block = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(id_group),
        ))
        .id();
    commands.spawn((
        label(server.name.clone(), font_bold.clone(), 15.5, theme::TEXT),
        ChildOf(name_block),
    ));
    commands.spawn((
        label(subtitle, font_body.clone(), 10.0, theme::TEXT_FAINT),
        ChildOf(name_block),
    ));

    let meta = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(top),
        ))
        .id();
    let stat = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(meta),
        ))
        .id();
    commands.spawn((
        label(
            server.users.to_string(),
            font_body.clone(),
            13.0,
            theme::TEXT,
        ),
        ChildOf(stat),
    ));
    commands.spawn((
        label("ONLINE", font_body.clone(), 8.5, theme::TEXT_FAINT),
        ChildOf(stat),
    ));

    let status_group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(meta),
        ))
        .id();
    commands.spawn((
        Node {
            width: Val::Px(7.0),
            height: Val::Px(7.0),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(color),
        Pickable::IGNORE,
        ChildOf(status_group),
    ));
    commands.spawn((
        label(status_label(status), font_body.clone(), 11.5, color),
        ChildOf(status_group),
    ));

    // ---- population bar ----
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.07)),
            Pickable::IGNORE,
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Node {
            width: Val::Percent(ratio * 100.0),
            height: Val::Percent(100.0),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(color),
        Pickable::IGNORE,
        ChildOf(track),
    ));
    commands.spawn((
        label(pop_word(ratio), font_body, 10.5, theme::TEXT_FAINT),
        ChildOf(bar),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::infrastructure::networking::server_info::{ServerInfo, ServerType};
    use game_engine::infrastructure::networking::session::SessionTokens;

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
        }
    }

    fn server_app(session: UserSession) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
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
}
