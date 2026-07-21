//! Status-effect icon bar: a top-right, wrapping row of the local player's active
//! statuses. Icons are stable, diff-synced children (so blink/hover survive frames),
//! rendered from the real RO TGA when the catalog resolves one and a themed placeholder
//! tile otherwise. Timed icons blink in their final seconds; hovering shows a live
//! name + countdown tooltip.

mod scene;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::states::status_icons::{ActiveStatus, LocalStatuses};
use game_engine::infrastructure::status::StatusIconCatalog;

use crate::theme;
use scene::{ICON_SIZE, blink_alpha, diff_efsts, format_remaining};

/// Marks the absolute-positioned container that holds the icon children.
#[derive(Component)]
pub struct StatusIconBar;

/// One status icon, tagged with its EFST so systems can rejoin it to `LocalStatuses`.
#[derive(Component)]
pub struct StatusIcon {
    pub efst: u32,
}

/// Marks a hover tooltip so it can be despawned on hover-out.
#[derive(Component)]
pub struct StatusTooltip;

/// The countdown line inside a tooltip, refreshed each frame from `LocalStatuses`.
#[derive(Component)]
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
                blink_expiring_icons,
                update_tooltip_countdown,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Spawns the icon bar below the minimap (which occupies top:16..~214, right:16).
pub fn spawn_status_bar(commands: &mut Commands, parent: Entity) {
    commands.spawn((
        StatusIconBar,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(220.0),
            right: Val::Px(16.0),
            width: Val::Px(6.0 * (ICON_SIZE + 4.0)),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::FlexEnd,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(parent),
    ));
}

/// Diffs `LocalStatuses` against the rendered `StatusIcon` children, spawning icons for
/// newly-active statuses and despawning icons for gone ones. No rebuild of unchanged
/// icons, so blink state and hover stay stable.
fn sync_status_icons(
    mut commands: Commands,
    local: Res<LocalStatuses>,
    catalog: Option<Res<StatusIconCatalog>>,
    asset_server: Res<AssetServer>,
    bar: Query<Entity, With<StatusIconBar>>,
    icons: Query<(Entity, &StatusIcon)>,
    mut logged: Local<HashSet<u32>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    let Ok(bar) = bar.single() else {
        return;
    };

    let existing_entities: HashMap<u32, Entity> = icons
        .iter()
        .map(|(entity, icon)| (icon.efst, entity))
        .collect();
    let active: HashSet<u32> = local.active.keys().copied().collect();
    let existing: HashSet<u32> = existing_entities.keys().copied().collect();

    let (to_add, to_remove) = diff_efsts(&active, &existing);

    for efst in to_remove {
        if let Some(entity) = existing_entities.get(&efst) {
            commands.entity(*entity).despawn();
        }
    }
    for efst in to_add {
        spawn_status_icon(
            &mut commands,
            bar,
            efst,
            &catalog,
            &asset_server,
            &mut logged,
        );
    }
}

/// Spawns a single 32x32 icon under the bar: the real TGA when the catalog resolves a
/// path, otherwise a themed placeholder tile. Hover observers attach either way.
fn spawn_status_icon(
    commands: &mut Commands,
    bar: Entity,
    efst: u32,
    catalog: &StatusIconCatalog,
    asset_server: &AssetServer,
    logged: &mut HashSet<u32>,
) {
    let mut icon = commands.spawn((StatusIcon { efst }, ChildOf(bar)));

    match catalog.icon_path(efst) {
        Some(path) => {
            icon.insert((
                ImageNode::new(asset_server.load(&path)),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    ..default()
                },
            ));
        }
        None => {
            icon.insert((
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme::FIELD),
                BorderColor::all(theme::STROKE),
            ));
            if catalog.name(efst).is_none() && logged.insert(efst) {
                warn!("Status effect EFST {efst} has no catalog entry (no icon, no name)");
            }
        }
    }

    icon.observe(on_icon_hover_over);
    icon.observe(on_icon_hover_out);
}

/// Oscillates a timed icon's alpha in its final seconds. Applies a blink *factor* over
/// the icon's resting alpha (white for images, `theme::FIELD` for placeholders) so a
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
            let target = theme::FIELD.with_alpha(theme::FIELD.alpha() * factor);
            if background.0 != target {
                background.0 = target;
            }
        }
    }
}

/// Remaining-time label for a status, empty for permanent / expiry-less statuses.
fn remaining_label(status: &ActiveStatus, now: Duration) -> String {
    if status.permanent {
        return String::new();
    }
    match status.expires_at {
        Some(at) => format_remaining(at.saturating_sub(now)),
        None => String::new(),
    }
}

/// Hovering an icon spawns a tooltip child: the status name (or `#efst` when unnamed)
/// plus a live remaining-time line.
fn on_icon_hover_over(
    over: On<Pointer<Over>>,
    icons: Query<&StatusIcon>,
    local: Res<LocalStatuses>,
    catalog: Option<Res<StatusIconCatalog>>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok(icon) = icons.get(over.entity) else {
        return;
    };
    let efst = icon.efst;

    let name = catalog
        .as_ref()
        .and_then(|c| c.name(efst))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{efst}"));
    let time_text = local
        .active
        .get(&efst)
        .map(|status| remaining_label(status, time.elapsed()))
        .unwrap_or_default();
    let font = asset_server.load(theme::FONT_BODY);

    let tooltip = commands
        .spawn((
            StatusTooltip,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(ICON_SIZE + 4.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(over.entity),
        ))
        .id();

    commands.spawn((
        theme::label(name, font.clone(), 11.0, theme::TEXT),
        ChildOf(tooltip),
    ));
    commands.spawn((
        StatusTooltipTime { efst },
        theme::label(time_text, font, 11.0, theme::TEXT_DIM),
        ChildOf(tooltip),
    ));
}

fn on_icon_hover_out(
    _: On<Pointer<Out>>,
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
