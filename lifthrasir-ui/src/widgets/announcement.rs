//! Server announcement rendering: echoes every announcement to the chat box as a
//! colored line and drives per-style on-screen overlays — a fading Top banner and a
//! fading Center flash — through independent arrival-order queues.
//!
//! An [`AnnouncementReceived`] event fans out in [`ingest_announcements`] to the
//! chat echo (all styles) and the matching overlay queue (Top/Center only; Local is
//! chat-only). Two queues let a Top banner and a Center flash be on screen at once
//! while same-style announcements serialize. Overlays parent under the
//! [`AnnouncementLayer`] node the HUD mounts, so they despawn with the HUD on exit.

use std::collections::VecDeque;

use bevy::prelude::*;
use game_engine::core::state::GameState;
use net_contract::events::{AnnouncementReceived, AnnouncementStyle};

use crate::rich_text::spawn_colored_text;
use crate::theme;
use crate::widgets::chat_box::{append_colored_line, ChatHistory};

const TOP_DEFAULT: Color = Color::srgb_u8(0xff, 0xff, 0x00);
const CENTER_DEFAULT: Color = Color::WHITE;
const LOCAL_DEFAULT: Color = Color::srgb_u8(0x2f, 0xd2, 0x7a);

const CENTER_DURATION_S: f32 = 4.0;
const TOP_DURATION_S: f32 = 4.0;
const TOP_FADE_S: f32 = 0.6;
const TOP_OFFSET_PX: f32 = 28.0;
const CENTER_TOP_PCT: f32 = 38.0;
const TOP_FONT_SIZE: f32 = 20.0;
const CENTER_FONT_SIZE: f32 = 34.0;
const OVERLAY_Z: i32 = 500;

pub struct AnnouncementPlugin;

impl Plugin for AnnouncementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnnouncementQueues>();
        app.add_systems(
            Update,
            (
                ingest_announcements,
                drive_top,
                drive_center,
                fade_top_banner,
                fade_center_flash,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), clear_queues);
    }
}

/// A resolved announcement waiting for its style's overlay slot to free up.
struct QueuedOverlay {
    text: String,
    color: Color,
}

/// Per-style overlay backlog. A resource (not world data) since it is transient
/// scheduling state; cleared on `OnExit(InGame)` so a backlog never leaks into a
/// fresh login.
#[derive(Resource, Default)]
pub struct AnnouncementQueues {
    top: VecDeque<QueuedOverlay>,
    center: VecDeque<QueuedOverlay>,
}

/// Full-screen container the overlays parent under. Mounted by the HUD.
#[derive(Component)]
pub struct AnnouncementLayer;

/// The active Top banner and its fade-in/out lifetime timer (at most one; the next
/// Top waits in the queue).
#[derive(Component)]
struct TopBanner {
    timer: Timer,
}

/// The active Center flash and its lifetime timer.
#[derive(Component)]
struct CenterFlash {
    timer: Timer,
}

/// Spawns the overlay layer under `parent`. Absolutely-positioned overlays offset
/// from this layer's top-left, which fills the HUD root.
pub fn spawn_announcement_layer(commands: &mut Commands, parent: Entity) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(OVERLAY_Z),
        Pickable::IGNORE,
        AnnouncementLayer,
        ChildOf(parent),
    ));
}

/// The proto `color` (`0xRRGGBB`) when non-zero, else the per-style default. Per
/// aesir, `0` is the common Top path, so the default is the primary case there, not
/// a rare guard.
pub fn resolve_color(color: u32, style: AnnouncementStyle) -> Color {
    if color != 0 {
        let [_, r, g, b] = color.to_be_bytes();
        return Color::srgb_u8(r, g, b);
    }
    match style {
        AnnouncementStyle::Top => TOP_DEFAULT,
        AnnouncementStyle::Center => CENTER_DEFAULT,
        AnnouncementStyle::Local => LOCAL_DEFAULT,
    }
}

/// The rendered line: `[source] text` when `source_name` is set, else `text`. `None`
/// when `text` is blank (avoids an empty chat line or banner).
fn format_line(source_name: &str, text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    if source_name.is_empty() {
        Some(text.to_string())
    } else {
        Some(format!("[{source_name}] {text}"))
    }
}

fn ingest_announcements(
    mut events: MessageReader<AnnouncementReceived>,
    container: Query<Entity, With<ChatHistory>>,
    mut queues: ResMut<AnnouncementQueues>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if events.is_empty() {
        return;
    }
    let Ok(container) = container.single() else {
        return;
    };
    let font = asset_server.load(theme::FONT_BODY);
    for event in events.read() {
        let Some(line) = format_line(&event.source_name, &event.text) else {
            continue;
        };
        let color = resolve_color(event.color, event.style);
        append_colored_line(&mut commands, container, &line, color, font.clone());
        match event.style {
            AnnouncementStyle::Top => queues.top.push_back(QueuedOverlay { text: line, color }),
            AnnouncementStyle::Center => {
                queues.center.push_back(QueuedOverlay { text: line, color })
            }
            AnnouncementStyle::Local => {}
        }
    }
}

fn drive_top(
    mut queues: ResMut<AnnouncementQueues>,
    layer: Query<Entity, With<AnnouncementLayer>>,
    active: Query<(), With<TopBanner>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if !active.is_empty() {
        return;
    }
    let Ok(layer) = layer.single() else {
        return;
    };
    let Some(overlay) = queues.top.pop_front() else {
        return;
    };

    let font = asset_server.load(theme::FONT_BODY);
    let banner = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(TOP_OFFSET_PX),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            TopBanner {
                timer: Timer::from_seconds(TOP_DURATION_S, TimerMode::Once),
            },
            ChildOf(layer),
        ))
        .id();
    spawn_colored_text(
        &mut commands,
        banner,
        &overlay.text,
        font,
        TOP_FONT_SIZE,
        overlay.color,
    );
}

fn drive_center(
    mut queues: ResMut<AnnouncementQueues>,
    layer: Query<Entity, With<AnnouncementLayer>>,
    active: Query<(), With<CenterFlash>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if !active.is_empty() {
        return;
    }
    let Ok(layer) = layer.single() else {
        return;
    };
    let Some(overlay) = queues.center.pop_front() else {
        return;
    };

    let font = asset_server.load(theme::FONT_BODY);
    let flash = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(CENTER_TOP_PCT),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            CenterFlash {
                timer: Timer::from_seconds(CENTER_DURATION_S, TimerMode::Once),
            },
            ChildOf(layer),
        ))
        .id();
    spawn_colored_text(
        &mut commands,
        flash,
        &overlay.text,
        font,
        CENTER_FONT_SIZE,
        overlay.color,
    );
}

fn fade_top_banner(
    time: Res<Time>,
    mut banners: Query<(Entity, &mut TopBanner)>,
    children: Query<&Children>,
    mut colors: Query<&mut TextColor>,
    mut commands: Commands,
) {
    for (entity, mut banner) in &mut banners {
        banner.timer.tick(time.delta());
        if banner.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = fade_in_out_alpha(banner.timer.elapsed_secs(), TOP_DURATION_S, TOP_FADE_S);
        fade_text_alpha(entity, alpha, &children, &mut colors);
    }
}

/// A trapezoidal envelope: ramps 0→1 over the first `fade` seconds, holds at 1, then
/// ramps 1→0 over the last `fade` seconds.
fn fade_in_out_alpha(elapsed: f32, total: f32, fade: f32) -> f32 {
    if elapsed < fade {
        return (elapsed / fade).clamp(0.0, 1.0);
    }
    if elapsed > total - fade {
        return ((total - elapsed) / fade).clamp(0.0, 1.0);
    }
    1.0
}

fn fade_center_flash(
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut CenterFlash)>,
    children: Query<&Children>,
    mut colors: Query<&mut TextColor>,
    mut commands: Commands,
) {
    for (entity, mut flash) in &mut flashes {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        fade_text_alpha(
            entity,
            flash.timer.fraction_remaining(),
            &children,
            &mut colors,
        );
    }
}

/// Sets alpha on every `TextColor` under `root` (the flash node's `Text` root plus
/// its per-run `TextSpan` children), so a multi-colored flash fades as one.
fn fade_text_alpha(
    root: Entity,
    alpha: f32,
    children: &Query<&Children>,
    colors: &mut Query<&mut TextColor>,
) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(mut color) = colors.get_mut(entity) {
            color.0.set_alpha(alpha);
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
}

fn clear_queues(mut queues: ResMut<AnnouncementQueues>) {
    queues.top.clear();
    queues.center.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::chat_box::ChatHistory;

    #[test]
    fn resolve_color_honors_nonzero_proto_color() {
        assert_eq!(
            resolve_color(0x1234ab, AnnouncementStyle::Center),
            Color::srgb_u8(0x12, 0x34, 0xab)
        );
    }

    #[test]
    fn resolve_color_defaults_per_style_when_zero() {
        assert_eq!(resolve_color(0, AnnouncementStyle::Top), TOP_DEFAULT);
        assert_eq!(resolve_color(0, AnnouncementStyle::Center), CENTER_DEFAULT);
        assert_eq!(resolve_color(0, AnnouncementStyle::Local), LOCAL_DEFAULT);
    }

    #[test]
    fn fade_in_out_alpha_ramps_up_holds_and_ramps_down() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(fade_in_out_alpha(0.0, 4.0, 0.6), 0.0));
        assert!(close(fade_in_out_alpha(0.3, 4.0, 0.6), 0.5));
        assert!(close(fade_in_out_alpha(2.0, 4.0, 0.6), 1.0));
        assert!(close(fade_in_out_alpha(3.7, 4.0, 0.6), 0.5));
        assert!(close(fade_in_out_alpha(4.0, 4.0, 0.6), 0.0));
    }

    #[test]
    fn format_line_skips_blank_text() {
        assert_eq!(format_line("GM", "   "), None);
        assert_eq!(format_line("", ""), None);
    }

    #[test]
    fn format_line_prefixes_only_with_source() {
        assert_eq!(format_line("", "hello").as_deref(), Some("hello"));
        assert_eq!(format_line("GM", "hello").as_deref(), Some("[GM] hello"));
    }

    fn ingest_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<AnnouncementReceived>();
        app.init_resource::<AnnouncementQueues>();
        app.world_mut().spawn(ChatHistory);
        app.add_systems(Update, ingest_announcements);
        app
    }

    fn announce(style: AnnouncementStyle, text: &str, source: &str) -> AnnouncementReceived {
        AnnouncementReceived {
            text: text.to_string(),
            color: 0,
            style,
            source_name: source.to_string(),
        }
    }

    #[test]
    fn ingest_enqueues_top_and_center_but_not_local() {
        let mut app = ingest_app();
        app.world_mut()
            .resource_mut::<Messages<AnnouncementReceived>>()
            .write(announce(AnnouncementStyle::Top, "top", ""));
        app.world_mut()
            .resource_mut::<Messages<AnnouncementReceived>>()
            .write(announce(AnnouncementStyle::Center, "center", ""));
        app.world_mut()
            .resource_mut::<Messages<AnnouncementReceived>>()
            .write(announce(AnnouncementStyle::Local, "local", ""));
        app.update();

        let queues = app.world().resource::<AnnouncementQueues>();
        assert_eq!(queues.top.len(), 1);
        assert_eq!(queues.center.len(), 1);
    }

    #[test]
    fn ingest_skips_blank_text() {
        let mut app = ingest_app();
        app.world_mut()
            .resource_mut::<Messages<AnnouncementReceived>>()
            .write(announce(AnnouncementStyle::Top, "  ", ""));
        app.update();
        assert_eq!(app.world().resource::<AnnouncementQueues>().top.len(), 0);
    }

    fn drive_center_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.init_resource::<AnnouncementQueues>();
        app.world_mut().spawn(AnnouncementLayer);
        app.add_systems(Update, drive_center);
        app
    }

    #[test]
    fn drive_center_spawns_only_one_active_overlay() {
        let mut app = drive_center_app();
        {
            let mut queues = app.world_mut().resource_mut::<AnnouncementQueues>();
            queues.center.push_back(QueuedOverlay {
                text: "first".to_string(),
                color: Color::WHITE,
            });
            queues.center.push_back(QueuedOverlay {
                text: "second".to_string(),
                color: Color::WHITE,
            });
        }

        app.update();
        let active = app
            .world_mut()
            .query_filtered::<Entity, With<CenterFlash>>()
            .iter(app.world())
            .count();
        assert_eq!(active, 1);
        assert_eq!(app.world().resource::<AnnouncementQueues>().center.len(), 1);

        app.update();
        let still_active = app
            .world_mut()
            .query_filtered::<Entity, With<CenterFlash>>()
            .iter(app.world())
            .count();
        assert_eq!(still_active, 1);
        assert_eq!(app.world().resource::<AnnouncementQueues>().center.len(), 1);
    }

    fn drive_top_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.init_resource::<AnnouncementQueues>();
        app.world_mut().spawn(AnnouncementLayer);
        app.add_systems(Update, drive_top);
        app
    }

    #[test]
    fn drive_top_spawns_only_one_active_overlay() {
        let mut app = drive_top_app();
        {
            let mut queues = app.world_mut().resource_mut::<AnnouncementQueues>();
            queues.top.push_back(QueuedOverlay {
                text: "first".to_string(),
                color: Color::WHITE,
            });
            queues.top.push_back(QueuedOverlay {
                text: "second".to_string(),
                color: Color::WHITE,
            });
        }

        app.update();
        let active = app
            .world_mut()
            .query_filtered::<Entity, With<TopBanner>>()
            .iter(app.world())
            .count();
        assert_eq!(active, 1);
        assert_eq!(app.world().resource::<AnnouncementQueues>().top.len(), 1);

        app.update();
        let still_active = app
            .world_mut()
            .query_filtered::<Entity, With<TopBanner>>()
            .iter(app.world())
            .count();
        assert_eq!(still_active, 1);
        assert_eq!(app.world().resource::<AnnouncementQueues>().top.len(), 1);
    }

    #[test]
    fn clear_queues_empties_both() {
        let mut app = App::new();
        app.init_resource::<AnnouncementQueues>();
        {
            let mut queues = app.world_mut().resource_mut::<AnnouncementQueues>();
            queues.top.push_back(QueuedOverlay {
                text: "t".to_string(),
                color: Color::WHITE,
            });
            queues.center.push_back(QueuedOverlay {
                text: "c".to_string(),
                color: Color::WHITE,
            });
        }
        app.add_systems(Update, clear_queues);
        app.update();
        let queues = app.world().resource::<AnnouncementQueues>();
        assert!(queues.top.is_empty());
        assert!(queues.center.is_empty());
    }
}
