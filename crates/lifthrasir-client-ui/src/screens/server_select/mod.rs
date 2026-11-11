//! Server selection screen ("Choose a World").
//!
//! One BSN scene ([`scene`]) built from the RO-dark primitives over the blurred menu
//! background: the world list on the left, the selected world's details on the right,
//! and the Back / Enter actions below. Rows are spawned once the session's server list
//! is available. [`SelectedServer`] drives the row highlight, the details panel and the
//! Enter button. Entering writes `ServerSelectedEvent`; the engine connects to the char
//! server and drives the transition. No contract event reports a failed char
//! connection, so [`ConnectingModal`] covers the wait and times out into a system
//! dialog. Log Out and Back return to `GameState::Login`, where the engine ends the
//! session.

mod scene;
#[cfg(test)]
mod tests;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::presentation::ui::events::{
    DialogSeverity, ServerSelectedEvent, ShowSystemDialog, SystemDialogKind,
};
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{HeroSummary, ServerInfo, ServerRates, ServerType};
use net_contract::state::UserSession;

use crate::widgets::system_dialog::SystemDialogRoot;

/// How long the char server may stay silent before the attempt is reported as failed.
const CONNECT_TIMEOUT_SECS: f32 = 10.0;

/// Online-population bucket for a server row's status pill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ServerStatus {
    Online,
    High,
    Full,
    Offline,
}

/// Assumed capacity for a server that reports none (`ServerInfo::max_users == 0`).
const POP_SOFT_CAP: u16 = 6000;

fn fill_ratio(users: u16) -> f32 {
    (users as f32 / POP_SOFT_CAP as f32).clamp(0.0, 1.0)
}

/// How full a server is, against its reported capacity when it has one.
fn server_fill(server: &ServerInfo) -> f32 {
    match server.max_users {
        0 => fill_ratio(server.users),
        capacity => (server.users as f32 / capacity as f32).clamp(0.0, 1.0),
    }
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

/// A server's status: maintenance worlds are offline whatever their population.
fn status_of(server: &ServerInfo) -> ServerStatus {
    if server.server_type == ServerType::Maintenance {
        return ServerStatus::Offline;
    }
    server_status(server_fill(server))
}

/// Display label for a server status pill.
fn status_label(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Online => "Online",
        ServerStatus::High => "Busy",
        ServerStatus::Full => "Full",
        ServerStatus::Offline => "Offline",
    }
}

/// Accent color for a status (dot, label, and population fill).
fn status_color(status: ServerStatus) -> Color {
    match status {
        ServerStatus::Online => ro_dark::STATUS_ONLINE,
        ServerStatus::High => ro_dark::STATUS_BUSY,
        ServerStatus::Full => ro_dark::STATUS_FULL,
        ServerStatus::Offline => ro_dark::STATUS_OFFLINE,
    }
}

/// Coarse population word, the row subtitle's second half for a server with no region.
fn pop_word(ratio: f32) -> &'static str {
    if ratio >= 0.6 {
        "High pop."
    } else if ratio >= 0.3 {
        "Healthy pop."
    } else {
        "Low pop."
    }
}

/// "No heroes", "1 hero", "3 heroes".
fn hero_count(count: usize) -> String {
    match count {
        0 => "No heroes".to_string(),
        1 => "1 hero".to_string(),
        n => format!("{n} heroes"),
    }
}

/// The primary button's label for the selected world.
fn enter_label(server: &ServerInfo) -> String {
    if server.server_type == ServerType::Maintenance {
        return "Closed for Update".to_string();
    }
    if server.heroes.is_empty() {
        return "Make a Hero".to_string();
    }
    format!("Enter {}", server.name)
}

/// Chip label and tint for a server type.
fn type_chip(server_type: ServerType) -> (&'static str, Color) {
    match server_type {
        ServerType::Normal => ("NORMAL", ro_dark::ACCENT),
        ServerType::Maintenance => ("MAINTENANCE", ro_dark::MUTED),
        ServerType::PvP => ("PVP", ro_dark::BRONZE_LIGHT),
        ServerType::PK => ("PK", ro_dark::DANGER_GLOW),
        ServerType::Special(_) => ("SPECIAL", ro_dark::SELECT_RING),
    }
}

/// `n` with `,` between thousands groups.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// "3× · 1.5× · 2×" for the base EXP, job EXP and drop rates (percentages, 100 = 1×).
fn rates_label(rates: ServerRates) -> String {
    let times = |percent: u16| format!("{}×", f32::from(percent) / 100.0);
    format!(
        "{} · {} · {}",
        times(rates.base_exp),
        times(rates.job_exp),
        times(rates.drop)
    )
}

/// How long ago `seconds` was, in the largest whole unit (a month is 30 days).
fn ago(seconds: u64) -> String {
    let (count, unit) = match seconds {
        0..60 => return "just now".to_string(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        86_400..2_592_000 => (seconds / 86_400, "day"),
        _ => (seconds / 2_592_000, "month"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// "Last played 2 hours ago with Ygor", from the most recently played hero. `None`
/// when no hero was ever played or the server clock `now` is unknown.
fn last_played(heroes: &[HeroSummary], now: Option<u64>) -> Option<String> {
    let now = now?;
    let hero = heroes
        .iter()
        .filter(|hero| hero.last_login > 0)
        .max_by_key(|hero| hero.last_login)?;
    let elapsed = now.saturating_sub(hero.last_login);
    Some(format!("Last played {} with {}", ago(elapsed), hero.name))
}

/// "Server time 20:58" (UTC) for a Unix time in seconds.
fn clock_label(now: u64) -> String {
    format!("Server time {:02}:{:02}", now / 3_600 % 24, now / 60 % 60)
}

/// Moves the selection by `step` rows, clamped to a list of `len` servers.
fn step_selection(current: usize, step: isize, len: usize) -> usize {
    current
        .saturating_add_signed(step)
        .min(len.saturating_sub(1))
}

/// Row fill and border for its selection/hover state.
fn row_style(selected: bool, hovered: bool) -> (Color, Color) {
    match (selected, hovered) {
        (true, _) => (ro_dark::SLOT_HOVER, ro_dark::SELECT_RING),
        (false, true) => (ro_dark::SLOT_HOVER, ro_dark::MUTED_BRONZE),
        (false, false) => (ro_dark::SLOT, ro_dark::INSET_RULE),
    }
}

pub struct ServerSelectScreenPlugin;

impl Plugin for ServerSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerListPopulated>();
        app.init_resource::<SelectedServer>();
        app.add_observer(enter_world);
        app.add_systems(
            OnEnter(GameState::ServerSelection),
            show_server_select_screen,
        );
        app.add_systems(
            Update,
            (
                populate_server_list,
                handle_keys,
                refresh_selection,
                style_rows,
                tick_connecting,
                tick_clock,
            )
                .run_if(in_state(GameState::ServerSelection)),
        );
    }
}

/// Guards the one-shot server-row spawn. Reset on every screen entry so re-entering
/// `ServerSelection` repopulates a fresh tree.
#[derive(Resource, Default)]
struct ServerListPopulated(bool);

/// Index into `UserSession::server_list` of the highlighted world.
#[derive(Resource, Default)]
struct SelectedServer(usize);

/// A runtime-spawned server row, holding its index into the server list.
#[derive(Component, Default, Clone)]
struct ServerRow(usize);

/// Marks the container that holds the runtime-spawned server rows.
#[derive(Component, Default, Clone)]
struct ServerList;

/// Marks the panel [`refresh_selection`] fills with the selected world's details.
#[derive(Component, Default, Clone)]
struct DetailsPanel;

/// Marks the primary button, relabelled for the selected world.
#[derive(Component, Default, Clone)]
struct EnterButton;

/// Marks the "Server time" label [`tick_clock`] keeps current.
#[derive(Component, Default, Clone)]
struct ServerClock;

/// The modal shown while the char server is being reached. `timer` is the wait budget.
#[derive(Component, Clone)]
struct ConnectingModal {
    world: String,
    timer: Timer,
}

/// Requests entering the selected world (double-click, Enter key or the primary button).
#[derive(Event)]
struct EnterWorld;

fn show_server_select_screen(
    mut commands: Commands,
    session: Res<UserSession>,
    mut populated: ResMut<ServerListPopulated>,
    mut selected: ResMut<SelectedServer>,
) {
    populated.0 = false;
    selected.0 = 0;
    let online = session.server_list.iter().map(|s| u32::from(s.users)).sum();
    commands
        .spawn_scene(scene::server_select_screen(
            session.username.clone(),
            online,
        ))
        .insert(DespawnOnExit(GameState::ServerSelection));
}

/// Spawns one row per server under the list container once the session's server list
/// is available.
fn populate_server_list(
    mut commands: Commands,
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

    for (index, server) in session.server_list.iter().enumerate() {
        commands
            .spawn_scene(scene::server_row(index, server))
            .insert(ChildOf(container));
    }

    populated.0 = true;
}

/// Log Out and Back. The engine ends the session on entering `GameState::Login`.
fn back_to_login(_: On<Activate>, mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Login);
}

/// Starts the connection to the selected world, unless one is already under way or
/// the world is closed.
fn enter_world(
    _: On<EnterWorld>,
    selected: Res<SelectedServer>,
    session: Res<UserSession>,
    connecting: Query<(), With<ConnectingModal>>,
    mut writer: MessageWriter<ServerSelectedEvent>,
    mut commands: Commands,
) {
    if !connecting.is_empty() {
        return;
    }
    let Some(server) = session.server_list.get(selected.0) else {
        return;
    };
    if server.server_type == ServerType::Maintenance {
        return;
    }
    writer.write(ServerSelectedEvent {
        server: server.clone(),
    });
    commands
        .spawn_scene(scene::connecting_modal(server.name.clone()))
        .insert(DespawnOnExit(GameState::ServerSelection));
}

/// Arrow keys move the selection and Enter plays, unless a modal owns the keyboard.
fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<UserSession>,
    mut selected: ResMut<SelectedServer>,
    connecting: Query<(), With<ConnectingModal>>,
    dialogs: Query<(), With<SystemDialogRoot>>,
    mut commands: Commands,
) {
    if !connecting.is_empty() || !dialogs.is_empty() {
        return;
    }
    let step = keys.just_pressed(KeyCode::ArrowDown) as isize
        - keys.just_pressed(KeyCode::ArrowUp) as isize;
    let next = step_selection(selected.0, step, session.server_list.len());
    if next != selected.0 {
        selected.0 = next;
    }
    if keys.just_pressed(KeyCode::Enter) {
        commands.trigger(EnterWorld);
    }
}

/// Rebuilds the details card and relabels the Enter button when the selection changes.
/// A closed world dims the button, which [`enter_world`] also refuses.
fn refresh_selection(
    selected: Res<SelectedServer>,
    session: Res<UserSession>,
    jobs: Option<Res<JobSpriteRegistry>>,
    panel: Single<Entity, With<DetailsPanel>>,
    enter: Single<(&Children, &mut ImageNode), With<EnterButton>>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    if !selected.is_changed() {
        return;
    }
    let Some(server) = session.server_list.get(selected.0) else {
        return;
    };
    commands.entity(*panel).despawn_children();
    commands
        .spawn_scene(scene::details(
            server,
            jobs.as_deref(),
            session.server_now(),
        ))
        .insert(ChildOf(*panel));

    let open = server.server_type != ServerType::Maintenance;
    let label = enter_label(server);
    let (children, mut art) = enter.into_inner();
    art.color = if open {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };
    for child in children.iter() {
        if let Ok(mut text) = texts.get_mut(child) {
            text.0 = label.clone();
        }
    }
}

/// Recolours rows whose hover state changed, or all of them when the selection moved.
fn style_rows(
    selected: Res<SelectedServer>,
    mut rows: Query<(
        &ServerRow,
        Ref<Hovered>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BoxShadow,
    )>,
) {
    for (row, hovered, mut background, mut border, mut glow) in &mut rows {
        if !selected.is_changed() && !hovered.is_changed() {
            continue;
        }
        let is_selected = row.0 == selected.0;
        let (fill, ring) = row_style(is_selected, hovered.get());
        *background = BackgroundColor(fill);
        *border = BorderColor::all(ring);
        *glow = if is_selected {
            BoxShadow::new(
                ro_dark::FROST_GLOW.with_alpha(0.22),
                Val::ZERO,
                Val::ZERO,
                Val::ZERO,
                px(10),
            )
        } else {
            BoxShadow::default()
        };
    }
}

/// Closes the wait modal and reports the failure once the char server has stayed
/// silent for [`CONNECT_TIMEOUT_SECS`]. Success needs no handling: the state exit
/// despawns the modal.
fn tick_connecting(
    time: Res<Time>,
    mut modals: Query<(Entity, &mut ConnectingModal)>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
    mut commands: Commands,
) {
    for (entity, mut modal) in &mut modals {
        if !modal.timer.tick(time.delta()).just_finished() {
            continue;
        }
        commands.entity(entity).despawn();
        dialogs.write(ShowSystemDialog {
            severity: DialogSeverity::Error,
            kind: SystemDialogKind::Generic,
            kicker: "Connection".into(),
            title: "Couldn't Connect".into(),
            message: format!(
                "{} didn't answer in {CONNECT_TIMEOUT_SECS:.0} seconds. Try again, or pick another world.",
                modal.world
            ),
            code: String::new(),
            button_label: "OK".into(),
            secondary_label: String::new(),
            confirm_state: None,
            correlation: None,
        });
    }
}

/// Keeps the "Server time" label on the current minute. It stays empty when the
/// server reported no time.
fn tick_clock(session: Res<UserSession>, mut clock: Single<&mut Text, With<ServerClock>>) {
    let Some(now) = session.server_now() else {
        return;
    };
    let label = clock_label(now);
    if clock.0 != label {
        clock.0 = label;
    }
}
