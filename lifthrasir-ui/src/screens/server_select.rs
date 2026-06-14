use bevy::prelude::*;
use bevy_extended_ui::html::HtmlSource;
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use bevy_extended_ui::styles::{CssClass, CssID, CssSource};
use bevy_extended_ui::widgets::Button;
use game_engine::core::state::GameState;
use game_engine::infrastructure::networking::protocol::login::types::ServerInfo;
use game_engine::infrastructure::networking::session::UserSession;
use game_engine::presentation::ui::events::ServerSelectedEvent;

const SERVER_SELECT_UI: &str = "server_select";
/// `AssetServer` path, relative to `assets/`. CSS `<link>` hrefs inside the HTML
/// resolve relative to this file's location (so `theme.css` -> `ui/theme.css`).
const SERVER_SELECT_HTML: &str = "ui/server_select.html";
/// `id` of the `<div>` that holds the runtime-spawned server rows.
const SERVER_LIST_CONTAINER_ID: &str = "server-list";
/// CSS class applied to each spawned server-row button.
const SERVER_ROW_CLASS: &str = "server-row";

pub struct ServerSelectScreenPlugin;

impl Plugin for ServerSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerListPopulated>();
        app.add_systems(OnEnter(GameState::ServerSelection), show_server_select_screen);
        app.add_systems(OnExit(GameState::ServerSelection), hide_server_select_screen);
        app.add_systems(
            Update,
            populate_server_list.run_if(in_state(GameState::ServerSelection)),
        );
    }
}

/// Guards the one-shot server-row spawn. extended_ui's list widgets build their
/// option children once and never rebuild, so the dynamic server list is built by
/// spawning `Button` entities under the template's container instead. Reset on
/// every screen entry so re-entering `ServerSelection` repopulates a fresh tree.
#[derive(Resource, Default)]
struct ServerListPopulated(bool);

#[allow(deprecated)]
fn show_server_select_screen(
    mut registry: ResMut<UiRegistry>,
    asset_server: Res<AssetServer>,
    mut populated: ResMut<ServerListPopulated>,
) {
    populated.0 = false;
    let handle: Handle<HtmlAsset> = asset_server.load(SERVER_SELECT_HTML);
    registry.add_and_use(SERVER_SELECT_UI.into(), HtmlSource::from_handle(handle));
}

#[allow(deprecated)]
fn hide_server_select_screen(mut registry: ResMut<UiRegistry>) {
    registry.remove(SERVER_SELECT_UI);
}

/// Display label for a server row: name plus its current population.
fn format_server_label(server: &ServerInfo) -> String {
    format!("{} ({} online)", server.name, server.users)
}

/// Spawns one clickable `Button` per server under the template's `#server-list`
/// container, once the container exists. The container's `CssSource` is cloned onto
/// each button so the stylesheet (`.server-row`) applies via `css_service`. Each
/// button carries a Bevy click observer that emits the engine's `ServerSelectedEvent`
/// for its server; the engine connects to the char server and drives the state
/// transition, so no UI-side transition is needed here.
fn populate_server_list(
    mut commands: Commands,
    mut populated: ResMut<ServerListPopulated>,
    containers: Query<(Entity, &CssSource, &CssID)>,
    session: Option<Res<UserSession>>,
) {
    if populated.0 {
        return;
    }
    let Some(session) = session else {
        return;
    };
    let Some((container, css_source, _)) = containers
        .iter()
        .find(|(_, _, id)| id.0 == SERVER_LIST_CONTAINER_ID)
    else {
        return;
    };

    let css_source = css_source.clone();
    commands.entity(container).with_children(|parent| {
        for (index, server) in session.server_list.iter().enumerate() {
            let server = server.clone();
            parent
                .spawn((
                    Button {
                        text: format_server_label(&server),
                        ..default()
                    },
                    CssClass(vec![SERVER_ROW_CLASS.to_string()]),
                    css_source.clone(),
                ))
                .observe(
                    move |_: On<Pointer<Click>>, mut writer: MessageWriter<ServerSelectedEvent>| {
                        writer.write(ServerSelectedEvent {
                            server: server.clone(),
                            server_index: Some(index),
                        });
                    },
                );
        }
    });

    populated.0 = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::infrastructure::networking::protocol::login::types::ServerType;
    use game_engine::infrastructure::networking::session::SessionTokens;

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
        }
    }

    #[test]
    fn label_includes_name_and_population() {
        assert_eq!(
            format_server_label(&server("Valhalla", 1234)),
            "Valhalla (1234 online)"
        );
    }

    #[test]
    fn label_handles_empty_server() {
        assert_eq!(format_server_label(&server("Asgard", 0)), "Asgard (0 online)");
    }

    #[test]
    fn populate_spawns_one_styled_button_per_server() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<ServerSelectedEvent>();
        app.init_resource::<ServerListPopulated>();
        app.insert_resource(user_session(vec![server("Valhalla", 7), server("Asgard", 3)]));
        app.add_systems(Update, populate_server_list);

        let container = app
            .world_mut()
            .spawn((CssID(SERVER_LIST_CONTAINER_ID.to_string()), CssSource::default()))
            .id();

        app.update();

        let world = app.world_mut();
        let mut rows: Vec<(String, Vec<String>)> = world
            .query::<(&Button, &ChildOf, &CssClass)>()
            .iter(world)
            .filter(|(_, parent, _)| parent.parent() == container)
            .map(|(button, _, class)| (button.text.clone(), class.0.clone()))
            .collect();
        rows.sort();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "Asgard (3 online)");
        assert_eq!(rows[1].0, "Valhalla (7 online)");
        assert!(rows.iter().all(|(_, class)| class.contains(&SERVER_ROW_CLASS.to_string())));
        assert!(world.resource::<ServerListPopulated>().0);
    }

    #[test]
    fn populate_is_idempotent_across_frames() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<ServerSelectedEvent>();
        app.init_resource::<ServerListPopulated>();
        app.insert_resource(user_session(vec![server("Valhalla", 7)]));
        app.add_systems(Update, populate_server_list);

        app.world_mut()
            .spawn((CssID(SERVER_LIST_CONTAINER_ID.to_string()), CssSource::default()));

        app.update();
        app.update();
        app.update();

        let world = app.world_mut();
        let count = world.query::<&Button>().iter(world).count();
        assert_eq!(count, 1);
    }
}
