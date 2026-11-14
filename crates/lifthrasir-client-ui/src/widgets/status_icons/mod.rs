//! Status-effect icons: a wrapping row of the local player's active statuses, mounted
//! by the status bar under its plate (beside it when minified). Icons are stable,
//! diff-synced children (so blink/hover survive frames), rendered from retail status
//! textures or the active performance's skill icon, with a placeholder tile when
//! unavailable. A timed icon counts down under its tile and blinks in its final
//! seconds; hovering shows its name and the time left.

mod presentation;
mod scene;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::skill::SkillCatalog;
use lifthrasir_assets::catalogs::status::catalog::StatusIconCatalog;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::character::states::status_icons::{
    ActiveStatus, LocalStatuses,
};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};

use crate::screens::shared::{clock, tooltip_frame};
use crate::widgets::status_bar::HudSettings;
use crate::widgets::status_icons::presentation::{StatusPresentation, resolve_status_presentation};
use crate::widgets::status_icons::scene::{
    BLINK_THRESHOLD, TIMER_ROOM, blink_alpha, diff_efsts, format_remaining, icon_size,
};

/// Ink of a countdown in its final seconds.
const ENDING: Color = Color::srgb_u8(0xe8, 0x89, 0x7e);

/// Marks the row that holds the icon children.
#[derive(Component, Default, Clone)]
pub struct StatusIconBar;

/// One status icon, tagged with its EFST so systems can rejoin it to `LocalStatuses`.
#[derive(Component)]
pub struct StatusIcon {
    pub efst: u32,
    presentation: StatusPresentation,
}

/// The countdown under an icon, refreshed each frame from `LocalStatuses`.
#[derive(Component)]
pub struct StatusTimer {
    pub efst: u32,
}

/// Marks a hover tooltip so it can be despawned on hover-out.
#[derive(Component, Default, Clone)]
pub struct StatusTooltip;

/// The countdown line inside a tooltip, refreshed each frame from `LocalStatuses`.
#[derive(Component, Default, Clone)]
pub struct StatusTooltipTime {
    pub efst: u32,
}

pub struct StatusIconsPlugin;

impl Plugin for StatusIconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                sync_status_icons,
                add_icon_timers,
                blink_expiring_icons,
                update_icon_timers,
                update_tooltip_countdown,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// The row the icons are spawned into. The status bar places it.
pub fn icon_row() -> impl Scene {
    bsn! {
        StatusIconBar
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: px(6),
            row_gap: px(6),
        }
        @ignore_picking()
    }
}

/// What decides an icon's artwork, name and size. The HUD settings are optional so
/// the icons also work on a host without a status bar.
type IconSources<'w> = (
    Option<Res<'w, StatusIconCatalog>>,
    Option<Res<'w, SkillCatalog>>,
    Option<Res<'w, HudSettings>>,
);

/// Diffs `LocalStatuses` against the rendered `StatusIcon` children, spawning icons for
/// newly-active statuses and despawning icons for gone ones. Rebuild only when the
/// resolved artwork or name changes, including song changes and late catalogs, or
/// when the status bar changes size and the icons with it.
fn sync_status_icons(
    mut commands: Commands,
    local: Res<LocalStatuses>,
    sources: IconSources,
    asset_server: Res<AssetServer>,
    bar: Query<Entity, With<StatusIconBar>>,
    icons: Query<(Entity, &StatusIcon)>,
    mut logged: Local<HashSet<u32>>,
) {
    let (Some(catalog), skills, settings) = sources else {
        return;
    };
    let Ok(bar) = bar.single() else {
        return;
    };
    let size = icon_size(
        settings
            .as_ref()
            .is_some_and(|settings| settings.character_info_minimal),
    );
    let resized = settings.is_some_and(|settings| settings.is_changed());
    if resized {
        for (entity, _) in &icons {
            commands.entity(entity).despawn();
        }
    }

    let existing_entities: HashMap<u32, Entity> = icons
        .iter()
        .filter(|_| !resized)
        .map(|(entity, icon)| (icon.efst, entity))
        .collect();
    let active: HashSet<u32> = local.active.keys().copied().collect();
    let existing: HashSet<u32> = existing_entities.keys().copied().collect();

    let (mut to_add, to_remove) = diff_efsts(&active, &existing);
    for (entity, icon) in icons.iter().filter(|_| !resized) {
        let Some(status) = local.active.get(&icon.efst) else {
            continue;
        };
        let presentation = resolve_status_presentation(
            icon.efst,
            status.source_skill_id,
            &catalog,
            skills.as_deref(),
        );
        if icon.presentation == presentation {
            continue;
        }
        commands.entity(entity).despawn();
        to_add.push(icon.efst);
    }
    to_add.sort_unstable();

    for efst in to_remove {
        if let Some(entity) = existing_entities.get(&efst) {
            commands.entity(*entity).despawn();
        }
    }
    for efst in to_add {
        spawn_status_icon(
            &mut commands,
            bar,
            (efst, size),
            resolve_status_presentation(
                efst,
                local
                    .active
                    .get(&efst)
                    .and_then(|status| status.source_skill_id),
                &catalog,
                skills.as_deref(),
            ),
            &asset_server,
            &mut logged,
        );
    }
}

/// Spawns a single `size` px icon under the bar using the resolved retail image or a
/// placeholder tile. Hover observers attach either way.
fn spawn_status_icon(
    commands: &mut Commands,
    bar: Entity,
    (efst, size): (u32, f32),
    presentation: StatusPresentation,
    asset_server: &AssetServer,
    logged: &mut HashSet<u32>,
) {
    let mut icon = commands.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            margin: UiRect::bottom(Val::Px(TIMER_ROOM)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BorderColor::all(ro_dark::DIVIDER.with_alpha(0.7)),
        ChildOf(bar),
    ));

    match &presentation.image_path {
        Some(path) => {
            icon.insert(ImageNode::new(asset_server.load(path)));
        }
        None => {
            icon.insert(BackgroundColor(ro_dark::INSET));
            if presentation.name == format!("#{efst}") && logged.insert(efst) {
                warn!("Status effect EFST {efst} has no catalog entry (no icon, no name)");
            }
        }
    }

    icon.insert(StatusIcon { efst, presentation });
    icon.observe(on_icon_hover_over);
    icon.observe(on_icon_hover_out);
}

/// Hangs the countdown under each freshly spawned icon.
fn add_icon_timers(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    icons: Query<(Entity, &StatusIcon), Added<StatusIcon>>,
) {
    for (entity, icon) in &icons {
        commands.spawn((
            StatusTimer { efst: icon.efst },
            theme::label("", asset_server.load(theme::FONT_BODY), 10.0, ro_dark::BODY),
            TextLayout {
                justify: Justify::Center,
                linebreak: LineBreak::NoWrap,
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(-1.0),
                right: Val::Px(-1.0),
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            ChildOf(entity),
        ));
    }
}

/// Oscillates a timed icon's alpha in its final seconds. Applies a blink *factor* over
/// the icon's resting alpha (white for images, the inset fill for placeholders) so a
/// non-blinking icon keeps its normal look.
fn blink_expiring_icons(
    time: Res<Time>,
    local: Res<LocalStatuses>,
    mut icons: Query<(
        &StatusIcon,
        Option<&mut ImageNode>,
        Option<&mut BackgroundColor>,
    )>,
) {
    let now = time.elapsed();
    let elapsed_secs = time.elapsed_secs();
    for (icon, image, background) in &mut icons {
        let Some(status) = local.active.get(&icon.efst) else {
            continue;
        };
        let remaining = status.expires_at.map(|at| at.saturating_sub(now));
        let factor = blink_alpha(remaining, status.permanent, elapsed_secs);

        if let Some(mut image) = image {
            let target = Color::WHITE.with_alpha(factor);
            if image.color != target {
                image.color = target;
            }
        } else if let Some(mut background) = background {
            let target = ro_dark::INSET.with_alpha(factor);
            if background.0 != target {
                background.0 = target;
            }
        }
    }
}

/// Time a status has left, or `None` for a permanent / expiry-less one.
fn remaining(status: &ActiveStatus, now: Duration) -> Option<Duration> {
    status
        .expires_at
        .filter(|_| !status.permanent)
        .map(|at| at.saturating_sub(now))
}

/// Remaining-time label for a status, empty for permanent / expiry-less statuses.
fn remaining_label(status: &ActiveStatus, now: Duration) -> String {
    remaining(status, now)
        .map(format_remaining)
        .unwrap_or_default()
}

/// Refreshes the countdown under each icon; it turns red in the final seconds.
fn update_icon_timers(
    time: Res<Time>,
    local: Res<LocalStatuses>,
    mut timers: Query<(&StatusTimer, &mut Text, &mut TextColor)>,
) {
    let now = time.elapsed();
    for (timer, mut text, mut ink) in &mut timers {
        let left = local
            .active
            .get(&timer.efst)
            .and_then(|status| remaining(status, now));
        let label = left.map(clock).unwrap_or_default();
        if text.0 != label {
            text.0 = label;
        }
        let tone = match left {
            Some(left) if left < BLINK_THRESHOLD => ENDING,
            _ => ro_dark::BODY,
        };
        if ink.0 != tone {
            ink.0 = tone;
        }
    }
}

/// Hovering an icon hangs a tooltip under it: the status name (or `#efst` when
/// unnamed) plus a live remaining-time line.
fn on_icon_hover_over(
    over: On<PointerOver>,
    icons: Query<&StatusIcon>,
    local: Res<LocalStatuses>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok(icon) = icons.get(over.entity) else {
        return;
    };
    let name = icon.presentation.name.clone();
    let time_text = local
        .active
        .get(&icon.efst)
        .map(|status| remaining_label(status, time.elapsed()))
        .unwrap_or_default();
    let countdown = StatusTooltipTime { efst: icon.efst };

    commands
        .spawn_scene(bsn! {
            StatusTooltip
            @tooltip_frame()
            Node {
                position_type: PositionType::Absolute,
                top: percent(100),
                left: px(0),
                margin: {UiRect::top(px(TIMER_ROOM + 4.0))},
            }
            Children [
                @chrome_text(name, 12.0, ro_dark::STRONG)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                countdown
                @chrome_text(time_text, 11.0, ro_dark::LABEL)
            ]
        })
        .insert(ChildOf(over.entity));
}

fn on_icon_hover_out(
    _: On<PointerOut>,
    tooltips: Query<Entity, With<StatusTooltip>>,
    mut commands: Commands,
) {
    for tooltip in &tooltips {
        commands.entity(tooltip).despawn();
    }
}

/// Refreshes the visible tooltip's countdown each frame; a permanent status shows no time.
fn update_tooltip_countdown(
    time: Res<Time>,
    local: Res<LocalStatuses>,
    mut lines: Query<(&StatusTooltipTime, &mut Text)>,
) {
    let now = time.elapsed();
    for (line, mut text) in &mut lines {
        let label = local
            .active
            .get(&line.efst)
            .map(|status| remaining_label(status, now))
            .unwrap_or_default();
        if text.0 != label {
            text.0 = label;
        }
    }
}
