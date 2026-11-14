//! Server announcement rendering, after the `Announcements & Toasts` artboard: echoes
//! every announcement to the chat box as a colored line and drives per-style
//! on-screen overlays, a Top banner and a Center flash, through independent
//! arrival-order queues.
//!
//! An [`AnnouncementReceived`] event fans out in [`ingest_announcements`] to the
//! chat echo (all styles) and the matching overlay queue (Top/Center only; Local is
//! chat-only). Two queues let a Top banner and a Center flash be on screen at once
//! while same-style announcements serialize. Overlays parent under the
//! [`AnnouncementLayer`] node the HUD mounts, so they despawn with the HUD on exit.
//!
//! The banner is a dark band between two hairlines in the announcement's colour,
//! with whoever speaks as its kicker. The flash is a headline in the title font.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::scene::EntityScene;
use lifthrasir_client::core::state::GameState;
use lifthrasir_ui::rich_text::parse_color_codes;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::ignore_picking;
use net_contract::events::{AnnouncementReceived, AnnouncementStyle};

use crate::screens::shared::{colored_text, overline};
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

const TOP_DEFAULT: Color = ro_dark::BRONZE_LIGHT;
const CENTER_DEFAULT: Color = ro_dark::HERO;
const LOCAL_DEFAULT: Color = Color::srgb_u8(0x2f, 0xd2, 0x7a);

const CENTER_DURATION_S: f32 = 3.4;
const CENTER_FADE_S: f32 = 0.4;
const TOP_DURATION_S: f32 = 6.0;
const TOP_FADE_S: f32 = 0.5;
const TOP_OFFSET_PX: f32 = 18.0;
const CENTER_TOP_PCT: f32 = 34.0;
const TOP_FONT_SIZE: f32 = 16.0;
const CENTER_FONT_SIZE: f32 = 36.0;
const OVERLAY_Z: i32 = 500;
/// How dark the banner's band is at full strength.
const BAND_ALPHA: f32 = 0.82;
/// The kicker of an announcement nobody is named for.
const SERVER_KICKER: &str = "ANNOUNCEMENT";

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
    /// Who speaks, in capitals. Empty when nobody is named.
    kicker: String,
    text: String,
    color: Color,
}

impl QueuedOverlay {
    fn new(source_name: &str, text: &str, color: Color) -> Self {
        Self {
            kicker: source_name.to_uppercase(),
            text: text.to_string(),
            color,
        }
    }
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

/// The dark band behind the banner's text.
#[derive(Component, Default, Clone)]
struct BannerBand;

/// A hairline along the band's edge: its colour and how strong it is.
#[derive(Component, Default, Clone)]
struct BannerRule(Color, f32);

/// The band's fill at `alpha`: dark in the middle, clear at both ends.
fn band(alpha: f32) -> BackgroundGradient {
    let ink = ro_dark::WINDOW_OUTLINE.with_alpha(BAND_ALPHA * alpha);
    BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(ink.with_alpha(0.0), percent(0)),
        ColorStop::new(ink, percent(14)),
        ColorStop::new(ink, percent(86)),
        ColorStop::new(ink.with_alpha(0.0), percent(100)),
    ]))
}

/// A hairline in `color` at `alpha`, fading out towards both ends.
fn rule(color: Color, alpha: f32) -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(color.with_alpha(0.0), percent(0)),
        ColorStop::new(color.with_alpha(alpha), percent(50)),
        ColorStop::new(color.with_alpha(0.0), percent(100)),
    ]))
}

/// `text` split at its `^RRGGBB` codes, every run invisible until the overlay fades in.
fn hidden_runs(text: &str, color: Color) -> Vec<(Color, String)> {
    parse_color_codes(text, color)
        .into_iter()
        .map(|(color, run)| (color.with_alpha(0.0), run))
        .collect()
}

/// The Top banner: the kicker over the text, on a band between two hairlines.
fn banner(overlay: QueuedOverlay) -> impl Scene {
    let QueuedOverlay {
        kicker,
        text,
        color,
    } = overlay;
    let kicker = if kicker.is_empty() {
        SERVER_KICKER.to_string()
    } else {
        kicker
    };
    let veil = band(0.0);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(TOP_OFFSET_PX),
            left: px(0),
            width: percent(100),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            BannerBand
            Node {
                min_width: px(520),
                max_width: px(760),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: {UiRect::new(px(26), px(26), px(9), px(10))},
                position_type: PositionType::Relative,
            }
            veil
            @ignore_picking()
            Children [
                @banner_rule(color, 1.0, true)
                --
                @banner_rule(color, 0.6, false)
                --
                @overline(kicker, color.with_alpha(0.0))
                --
                @colored_text(hidden_runs(&text, color), theme::FONT_BODY, TOP_FONT_SIZE)
                TextShadow { offset: {Vec2::splat(1.0)}, color: {Color::BLACK.with_alpha(0.0)} }
            ]
        ]
    }
}

fn banner_rule(color: Color, strength: f32, top: bool) -> impl Scene {
    let (top, bottom) = if top {
        (px(0), Val::Auto)
    } else {
        (Val::Auto, px(0))
    };
    let line = rule(color, 0.0);
    bsn! {
        BannerRule(color, strength)
        Node {
            position_type: PositionType::Absolute,
            left: percent(14),
            right: percent(14),
            top: {top},
            bottom: {bottom},
            height: px(1),
        }
        line
        @ignore_picking()
    }
}

/// The Center flash: who speaks over the text as a headline.
fn flash(overlay: QueuedOverlay) -> impl Scene {
    let QueuedOverlay {
        kicker,
        text,
        color,
    } = overlay;
    let kicker = (!kicker.is_empty()).then(|| EntityScene(overline(kicker, color.with_alpha(0.0))));
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: {Val::Percent(CENTER_TOP_PCT)},
            left: px(0),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
        }
        @ignore_picking()
        Children [
            {kicker}
            --
            @colored_text(hidden_runs(&text, color), theme::FONT_TITLE, CENTER_FONT_SIZE)
            TextShadow { offset: {Vec2::new(0.0, 2.0)}, color: {Color::BLACK.with_alpha(0.0)} }
        ]
    }
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
    let font = asset_server.load(lifthrasir_ui::theme::FONT_BODY);
    for event in events.read() {
        let Some(line) = format_line(&event.source_name, &event.text) else {
            continue;
        };
        let color = resolve_color(event.color, event.style);
        append_colored_line(&mut commands, container, &line, color, font.clone());
        let overlay = QueuedOverlay::new(&event.source_name, &event.text, color);
        match event.style {
            AnnouncementStyle::Top => queues.top.push_back(overlay),
            AnnouncementStyle::Center => queues.center.push_back(overlay),
            AnnouncementStyle::Local => {}
        }
    }
}

fn drive_top(
    mut queues: ResMut<AnnouncementQueues>,
    layer: Query<Entity, With<AnnouncementLayer>>,
    active: Query<(), With<TopBanner>>,
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
    commands.spawn_scene(banner(overlay)).insert((
        TopBanner {
            timer: Timer::from_seconds(TOP_DURATION_S, TimerMode::Once),
        },
        ChildOf(layer),
    ));
}

fn drive_center(
    mut queues: ResMut<AnnouncementQueues>,
    layer: Query<Entity, With<AnnouncementLayer>>,
    active: Query<(), With<CenterFlash>>,
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
    commands.spawn_scene(flash(overlay)).insert((
        CenterFlash {
            timer: Timer::from_seconds(CENTER_DURATION_S, TimerMode::Once),
        },
        ChildOf(layer),
    ));
}

/// The text colours of an overlay, and the shadow under a text that has one.
type Inks<'w, 's> = Query<'w, 's, (&'static mut TextColor, Option<&'static mut TextShadow>)>;

fn fade_top_banner(
    time: Res<Time>,
    mut banners: Query<(Entity, &mut TopBanner)>,
    children: Query<&Children>,
    mut inks: Inks,
    mut bands: Query<&mut BackgroundGradient, (With<BannerBand>, Without<BannerRule>)>,
    mut rules: Query<(&BannerRule, &mut BackgroundGradient)>,
    mut commands: Commands,
) {
    for (entity, mut banner) in &mut banners {
        banner.timer.tick(time.delta());
        if banner.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = fade_in_out_alpha(banner.timer.elapsed_secs(), TOP_DURATION_S, TOP_FADE_S);
        fade_text_alpha(entity, alpha, &children, &mut inks);
        // Only one banner is up at a time, so every band and hairline is its own.
        for mut fill in &mut bands {
            *fill = band(alpha);
        }
        for (line, mut fill) in &mut rules {
            *fill = rule(line.0, alpha * line.1);
        }
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
    mut inks: Inks,
    mut commands: Commands,
) {
    for (entity, mut flash) in &mut flashes {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = fade_in_out_alpha(flash.timer.elapsed_secs(), CENTER_DURATION_S, CENTER_FADE_S);
        fade_text_alpha(entity, alpha, &children, &mut inks);
    }
}

/// Sets alpha on every text under `root` (each `Text` root, its per-run `TextSpan`
/// children and its shadow), so a multi-colored overlay fades as one.
fn fade_text_alpha(root: Entity, alpha: f32, children: &Query<&Children>, inks: &mut Inks) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((mut color, shadow)) = inks.get_mut(entity) {
            color.0.set_alpha(alpha);
            if let Some(mut shadow) = shadow {
                shadow.color.set_alpha(alpha);
            }
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

    /// An app that can spawn the overlays, with each update a quarter second long.
    fn overlay_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(250),
        ));
        app.init_resource::<AnnouncementQueues>();
        app.world_mut().spawn(AnnouncementLayer);
        app
    }

    fn drive_center_app() -> App {
        let mut app = overlay_app();
        app.add_systems(Update, drive_center);
        app
    }

    /// Every text on screen, sorted: queries do not follow the hierarchy's order.
    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut texts: Vec<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        texts.sort();
        texts
    }

    #[test]
    fn the_banner_names_who_speaks_and_the_server_when_nobody_does() {
        let mut app = overlay_app();
        app.add_systems(Update, drive_top);
        app.world_mut()
            .resource_mut::<AnnouncementQueues>()
            .top
            .push_back(QueuedOverlay::new("Sigrun", "recruiting", Color::WHITE));
        app.update();
        assert_eq!(texts(&mut app), ["SIGRUN", "recruiting"]);

        let mut app = overlay_app();
        app.add_systems(Update, drive_top);
        app.world_mut()
            .resource_mut::<AnnouncementQueues>()
            .top
            .push_back(QueuedOverlay::new("", "maintenance", Color::WHITE));
        app.update();
        assert_eq!(texts(&mut app), [SERVER_KICKER, "maintenance"]);
    }

    #[test]
    fn the_flash_has_no_kicker_when_nobody_is_named() {
        let mut app = drive_center_app();
        app.world_mut()
            .resource_mut::<AnnouncementQueues>()
            .center
            .push_back(QueuedOverlay::new("", "Dark Lord Appears", Color::WHITE));
        app.update();
        assert_eq!(texts(&mut app), ["Dark Lord Appears"]);
    }

    #[test]
    fn the_banner_fades_in_with_its_text_shadow_and_band() {
        let mut app = overlay_app();
        app.add_systems(Update, (drive_top, fade_top_banner).chain());
        app.world_mut()
            .resource_mut::<AnnouncementQueues>()
            .top
            .push_back(QueuedOverlay::new("", "maintenance", Color::WHITE));
        app.update();
        let world = app.world_mut();
        let hidden = world
            .query::<&TextColor>()
            .iter(world)
            .all(|color| color.0.alpha() == 0.0);
        assert!(hidden, "the banner is spawned invisible");

        for _ in 0..4 {
            app.update();
        }
        let world = app.world_mut();
        let shown = world
            .query::<&TextColor>()
            .iter(world)
            .all(|color| color.0.alpha() == 1.0);
        assert!(shown, "past the fade the text is opaque");
        let shadow = world.query::<&TextShadow>().single(world).unwrap();
        assert_eq!(shadow.color.alpha(), 1.0);
        let fill = world
            .query_filtered::<&BackgroundGradient, With<BannerBand>>()
            .single(world)
            .unwrap();
        assert_eq!(*fill, band(1.0));
    }

    #[test]
    fn drive_center_spawns_only_one_active_overlay() {
        let mut app = drive_center_app();
        {
            let mut queues = app.world_mut().resource_mut::<AnnouncementQueues>();
            queues
                .center
                .push_back(QueuedOverlay::new("", "first", Color::WHITE));
            queues
                .center
                .push_back(QueuedOverlay::new("", "second", Color::WHITE));
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
        let mut app = overlay_app();
        app.add_systems(Update, drive_top);
        app
    }

    #[test]
    fn drive_top_spawns_only_one_active_overlay() {
        let mut app = drive_top_app();
        {
            let mut queues = app.world_mut().resource_mut::<AnnouncementQueues>();
            queues
                .top
                .push_back(QueuedOverlay::new("", "first", Color::WHITE));
            queues
                .top
                .push_back(QueuedOverlay::new("", "second", Color::WHITE));
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
            queues
                .top
                .push_back(QueuedOverlay::new("", "t", Color::WHITE));
            queues
                .center
                .push_back(QueuedOverlay::new("", "c", Color::WHITE));
        }
        app.add_systems(Update, clear_queues);
        app.update();
        let queues = app.world().resource::<AnnouncementQueues>();
        assert!(queues.top.is_empty());
        assert!(queues.center.is_empty());
    }
}
