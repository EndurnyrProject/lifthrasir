//! The status bar in the HUD's top-left corner, after the `Status Bar · Full` and
//! `Status Bar · Minified` artboards of the Lifthrasir Widgets design: the hero's
//! portrait, identity, HP/SP/AP, EXP, zeny and weight, with the status effects under
//! it (beside it when minified).
//!
//! The plate is respawned only when its size is toggled. [`refresh`] writes the
//! values into the marked nodes when the hero's status changes, so a button is never
//! respawned mid-click, and [`animate_vitals`] moves what changes every frame.

use std::f32::consts::TAU;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup};
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::job::player_jobs::is_fourth_job;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::components::core::CharacterData;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_ui::theme::ro_dark;

mod portrait;
mod scene;
mod view;

#[cfg(test)]
mod tests;

use crate::widgets::character_window::CharacterWindowState;
use crate::widgets::hud_layout::HudLayout;
use view::{Field, Gauge, LevelUp, StatusView, Tip};

/// How long a spent resource's ghost stays put before it drains, in seconds.
const TRAIL_HOLD: f32 = 0.25;
/// How fast the ghost drains, in percent of the bar per second.
const TRAIL_SPEED: f32 = 60.0;
/// Length of one low-HP pulse, in seconds.
const PULSE_SECS: f32 = 0.9;
/// How long the level-up banner stays, and how long the plate keeps glowing.
const BANNER_SECS: f32 = 2.4;
const GLOW_SECS: f32 = 1.6;
/// How far right of the bar's left edge a tooltip starts, relative to the pointer.
const TIP_LEAD: f32 = 24.0;

/// HUD preferences persisted by Bevy's native settings plugin.
#[derive(Resource, SettingsGroup, Reflect, Default)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "hud")]
pub(crate) struct HudSettings {
    /// Whether the status bar is minified.
    pub(crate) character_info_minimal: bool,
    /// Whether the Homunculus plate shows only the portrait.
    pub(crate) homunculus_minimal: bool,
}

/// The bar's corner of the HUD.
#[derive(Component, Default, Clone)]
struct StatusBarRoot;

/// Mount of the plate, refilled when the bar changes size.
#[derive(Component, Default, Clone)]
struct PlateHost;

#[derive(Component, Default, Clone)]
struct Plate;

/// The part of a track whose width follows its gauge.
#[derive(Component, Default, Clone)]
struct Fill(Gauge);

/// The ghost behind a vital bar: it shows `shown` percent and, after a drop, waits
/// `hold` seconds before draining down to the bar.
#[derive(Component, Default, Clone)]
struct Trail {
    gauge: Gauge,
    shown: f32,
    hold: f32,
}

/// The AP row, shown only to jobs that have AP.
#[derive(Component, Default, Clone)]
struct ApRow;

/// The wash over the HP bar that pulses while HP is low.
#[derive(Component, Default, Clone)]
struct LowPulse;

/// The portrait's ring, red while HP is low.
#[derive(Component, Default, Clone)]
struct PortraitRing;

/// The tooltip of the hovered part.
#[derive(Component, Default, Clone)]
struct StatusTip;

/// The level-up banner and the seconds it has been up.
#[derive(Component, Default, Clone)]
struct Banner {
    age: f32,
}

/// What a button of the bar does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum BarAction {
    #[default]
    OpenCharacter,
    ToggleSize,
}

pub struct StatusBarPlugin;

impl Plugin for StatusBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudSettings>().add_systems(
            Update,
            (
                rebuild_plate.run_if(size_changed),
                (
                    refresh.run_if(status_changed),
                    animate_vitals,
                    watch_levels,
                    fade_banner,
                    portrait::paint_portrait,
                )
                    .run_if(in_state(GameState::InGame)),
            ),
        );
    }
}

/// Mounts the status bar, and the status effects with it, on the HUD.
pub fn spawn_status_bar(commands: &mut Commands, hud: Entity) {
    commands.spawn_scene(scene::root()).insert(ChildOf(hud));
}

pub(crate) use portrait::Portrait;

/// The local hero's status as the bar reads it.
#[derive(SystemParam)]
pub(crate) struct Hero<'w, 's> {
    player: Query<
        'w,
        's,
        (
            &'static CharacterStatus,
            &'static CharacterData,
            Option<&'static EntityName>,
        ),
        With<LocalPlayer>,
    >,
    jobs: Option<Res<'w, JobSpriteRegistry>>,
}

impl Hero<'_, '_> {
    /// The hero's name, job and base level, for the widgets that name them.
    pub(crate) fn identity(&self) -> Option<(String, String, u32)> {
        let view = self.view()?;
        Some((view.name, view.job, view.base_level))
    }

    fn view(&self) -> Option<StatusView> {
        let (status, data, name) = self.player.single().ok()?;
        let job_id = u32::from(data.job_id);
        let job = self
            .jobs
            .as_deref()
            .and_then(|jobs| jobs.get_display_name(job_id))
            .unwrap_or("Unknown");
        Some(StatusView {
            name: name.map_or_else(|| data.name.clone(), |name| name.name.clone()),
            job: job.to_string(),
            base_level: status.base_level,
            job_level: status.job_level,
            hp: (status.hp, status.max_hp),
            sp: (status.sp, status.max_sp),
            ap: (status.ap, status.max_ap),
            base_exp: (status.base_exp, status.next_base_exp),
            job_exp: (status.job_exp, status.next_job_exp),
            weight: (status.weight, status.max_weight),
            zeny: status.zeny,
            has_ap: is_fourth_job(job_id) && status.max_ap > 0,
        })
    }
}

fn size_changed(settings: Res<HudSettings>, added: Query<(), Added<PlateHost>>) -> bool {
    settings.is_changed() || !added.is_empty()
}

/// Lays the corner out for the bar's size and mounts that size's plate. Minified, the
/// status effects sit beside the plate instead of under it.
fn rebuild_plate(
    mut commands: Commands,
    settings: Res<HudSettings>,
    mut roots: Query<&mut Node, With<StatusBarRoot>>,
    hosts: Query<(Entity, Option<&Children>), With<PlateHost>>,
    tips: Query<Entity, With<StatusTip>>,
) {
    let minimal = settings.character_info_minimal;
    for mut root in &mut roots {
        (root.flex_direction, root.align_items) = if minimal {
            (FlexDirection::Row, AlignItems::Center)
        } else {
            (FlexDirection::Column, AlignItems::FlexStart)
        };
    }
    for (host, children) in &hosts {
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        let plate = if minimal {
            commands.spawn_scene(scene::mini()).id()
        } else {
            commands.spawn_scene(scene::full()).id()
        };
        commands.entity(plate).insert(ChildOf(host));
    }
    for tip in &tips {
        commands.entity(tip).despawn();
    }
}

type ShownStatusChanged = (
    With<LocalPlayer>,
    Or<(
        Changed<CharacterStatus>,
        Changed<CharacterData>,
        Changed<EntityName>,
    )>,
);

/// Gates [`refresh`]: the hero's status, data or name changed, the job registry
/// loaded (so "Unknown" becomes a job name), or a plate was just mounted.
fn status_changed(
    player: Query<(), ShownStatusChanged>,
    jobs: Option<Res<JobSpriteRegistry>>,
    mounted: Query<(), Added<Field>>,
) -> bool {
    !player.is_empty() || jobs.is_some_and(|jobs| jobs.is_changed()) || !mounted.is_empty()
}

/// Writes the hero's status into the plate, touching only what differs.
fn refresh(
    hero: Hero,
    mut texts: Query<(&Field, &mut Text, &mut TextColor)>,
    mut fills: Query<(&Fill, &mut Node, &mut BackgroundColor), Without<ApRow>>,
    mut ap_rows: Query<&mut Node, (With<ApRow>, Without<Fill>)>,
) {
    let Some(view) = hero.view() else {
        return;
    };
    let load_tone = view.load_tone();

    for (field, mut text, mut ink) in &mut texts {
        let value = view.text(*field);
        if text.0 != value {
            text.0 = value;
        }
        if *field == Field::Short(Gauge::Weight) && ink.0 != load_tone {
            ink.0 = load_tone;
        }
    }

    for (fill, mut node, mut paint) in &mut fills {
        let width = Val::Percent(view.percent(fill.0));
        if node.width != width {
            node.width = width;
        }
        if fill.0 == Gauge::Weight && paint.0 != load_tone {
            paint.0 = load_tone;
        }
    }

    let display = if view.has_ap {
        Display::Flex
    } else {
        Display::None
    };
    for mut row in &mut ap_rows {
        if row.display != display {
            row.display = display;
        }
    }
}

/// Where a ghost stands `dt` seconds later, given the bar is at `target` percent: it
/// jumps up with the bar, and after a drop it holds, then drains down to it.
fn step_trail(trail: &Trail, target: f32, dt: f32) -> (f32, f32) {
    if target >= trail.shown {
        return (target, TRAIL_HOLD);
    }
    if trail.hold > 0.0 {
        return (trail.shown, trail.hold - dt);
    }
    ((trail.shown - TRAIL_SPEED * dt).max(target), 0.0)
}

/// The per-frame motion of the vitals: the ghosts behind the bars, the pulse over a
/// low HP bar and the portrait ring turning red with it.
fn animate_vitals(
    time: Res<Time>,
    player: Query<&CharacterStatus, With<LocalPlayer>>,
    mut trails: Query<(&mut Trail, &mut Node)>,
    mut pulses: Query<&mut BackgroundColor, With<LowPulse>>,
    mut rings: Query<&mut BorderColor, With<PortraitRing>>,
) {
    let Ok(status) = player.single() else {
        return;
    };

    for (mut trail, mut node) in &mut trails {
        let target = match trail.gauge {
            Gauge::Hp => view::percent(status.hp, status.max_hp),
            Gauge::Sp => view::percent(status.sp, status.max_sp),
            _ => view::percent(status.ap, status.max_ap),
        };
        let (shown, hold) = step_trail(&trail, target, time.delta_secs());
        if (trail.shown, trail.hold) != (shown, hold) {
            (trail.shown, trail.hold) = (shown, hold);
        }
        let width = Val::Percent(shown);
        if node.width != width {
            node.width = width;
        }
    }

    let low = view::low_hp(status.hp, status.max_hp);
    let beat = (time.elapsed_secs() * TAU / PULSE_SECS).sin() * 0.5 + 0.5;
    let wash = Color::WHITE.with_alpha(if low { 0.3 * beat } else { 0.0 });
    for mut pulse in &mut pulses {
        if pulse.0 != wash {
            pulse.0 = wash;
        }
    }
    let ring = BorderColor::all(if low {
        ro_dark::HP
    } else {
        ro_dark::PRIMARY_STROKE
    });
    for mut border in &mut rings {
        if *border != ring {
            *border = ring;
        }
    }
}

type LevelsChanged = (With<LocalPlayer>, Changed<CharacterStatus>);

/// Raises the banner when the hero's base or job level goes up. The levels a hero
/// logs in with are remembered, not celebrated.
fn watch_levels(
    mut commands: Commands,
    player: Query<(Entity, &CharacterStatus), LevelsChanged>,
    settings: Res<HudSettings>,
    roots: Query<&ChildOf, With<StatusBarRoot>>,
    banners: Query<Entity, With<Banner>>,
    mut seen: Local<Option<(Entity, (u32, u32))>>,
) {
    let Ok((hero, status)) = player.single() else {
        return;
    };
    let levels = (status.base_level, status.job_level);
    let Some((was, before)) = seen.replace((hero, levels)) else {
        return;
    };
    let (Some(level), Ok(root)) = (
        LevelUp::between(before, levels).filter(|_| was == hero),
        roots.single(),
    ) else {
        return;
    };
    for banner in &banners {
        commands.entity(banner).despawn();
    }
    let top = scene::banner_top(settings.character_info_minimal);
    commands
        .spawn_scene(scene::banner(level, top))
        .insert(ChildOf(root.parent()));
}

/// How opaque the banner is `age` seconds in: it fades in fast and out at the end.
fn banner_alpha(age: f32) -> f32 {
    let (fade_in, fade_out) = (0.12 * BANNER_SECS, 0.85 * BANNER_SECS);
    if age < fade_in {
        return age / fade_in;
    }
    if age < fade_out {
        return 1.0;
    }
    ((BANNER_SECS - age) / (BANNER_SECS - fade_out)).clamp(0.0, 1.0)
}

/// Fades the banner in and out, lets the plate's gold flash die down, then removes
/// the banner.
fn fade_banner(
    time: Res<Time>,
    mut commands: Commands,
    mut banners: Query<(Entity, &mut Banner, &Children)>,
    mut lines: Query<(&mut TextColor, &mut TextShadow)>,
    mut plates: Query<(&mut BoxShadow, &mut BorderColor), With<Plate>>,
) {
    for (entity, mut banner, children) in &mut banners {
        banner.age += time.delta_secs();
        let alpha = banner_alpha(banner.age);
        for child in children.iter() {
            if let Ok((mut ink, mut shadow)) = lines.get_mut(child) {
                ink.0.set_alpha(alpha);
                shadow.color.set_alpha(alpha);
            }
        }
        let glow = (1.0 - banner.age / GLOW_SECS).clamp(0.0, 1.0);
        for (mut shadow, mut edge) in &mut plates {
            *shadow = scene::plate_shadow(glow);
            *edge = scene::plate_edge(glow);
        }
        if banner.age >= BANNER_SECS {
            commands.entity(entity).despawn();
        }
    }
}

/// Hangs the hovered part's tooltip under the plate, below the pointer.
fn show_tip(
    over: On<PointerOver>,
    mut commands: Commands,
    parts: Query<&Tip>,
    hero: Hero,
    (settings, layout): (Res<HudSettings>, Res<HudLayout>),
    roots: Query<Entity, With<StatusBarRoot>>,
    shown: Query<Entity, With<StatusTip>>,
) {
    let (Ok(part), Some(view), Ok(root)) = (parts.get(over.entity), hero.view(), roots.single())
    else {
        return;
    };
    for tip in &shown {
        commands.entity(tip).despawn();
    }
    let offset = layout.status_bar;
    let left = (over.pointer.position.x - scene::MARGIN - TIP_LEAD).max(offset.x);
    let top = scene::plate_height(settings.character_info_minimal) + 8.0 + offset.y;
    commands
        .spawn_scene(scene::tip(view.tip(*part), left, top))
        .insert(ChildOf(root));
}

fn hide_tip(_: On<PointerOut>, mut commands: Commands, shown: Query<Entity, With<StatusTip>>) {
    for tip in &shown {
        commands.entity(tip).despawn();
    }
}

fn on_bar_button(
    activate: On<Activate>,
    actions: Query<&BarAction>,
    mut window: ResMut<CharacterWindowState>,
    mut settings: ResMut<HudSettings>,
    mut commands: Commands,
) {
    match actions.get(activate.entity) {
        Ok(BarAction::OpenCharacter) => window.open = !window.open,
        Ok(BarAction::ToggleSize) => {
            settings.character_info_minimal = !settings.character_info_minimal;
            commands.queue(SaveSettings::IfChanged);
        }
        Err(_) => {}
    }
}
