//! Character-info status frame (top-left HUD): avatar, name, job/level sub-row,
//! HP/SP bars and Base/Job EXP slivers for the local player. Built as raw
//! `bevy_ui` by [`spawn_status_frame`] (called from the HUD root);
//! [`update_character_info`] reflects the `LocalPlayer`'s status into the marked
//! elements, writing only when a value actually changed so it doesn't churn
//! change detection every frame. Mirrors the Endurnir `.status-frame` design.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::job::registry::JobSpriteRegistry;

use crate::theme;

const AVATAR_BG: Color = Color::srgb_u8(0x1f, 0x2b, 0x25);
const AVATAR_RING: Color = Color::srgba(0.184, 0.824, 0.478, 0.35);
const BAR_TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.42);
const EXP_TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.40);

/// Tags a text element so [`update_character_info`] can write the matching value.
#[derive(Component, Clone, Copy)]
enum HudText {
    Avatar,
    Name,
    Job,
    BaseLevel,
    JobLevel,
    Hp,
    Sp,
    BaseExp,
    JobExp,
}

/// Tags a bar fill node so its width tracks the matching ratio.
#[derive(Component, Clone, Copy)]
enum HudBar {
    Hp,
    Sp,
    BaseExp,
    JobExp,
}

pub struct CharacterInfoPlugin;

impl Plugin for CharacterInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_character_info
                .run_if(in_state(GameState::InGame).and(character_info_changed)),
        );
    }
}

type ChangedCharacterInfo = (
    With<LocalPlayer>,
    Or<(
        Changed<CharacterStatus>,
        Changed<CharacterData>,
        Changed<EntityName>,
    )>,
);

/// Gates `update_character_info`: run only when the local player's status, data,
/// or name change, when the job registry loads (so "Unknown" resolves to a real
/// job name), or when the HUD elements are freshly spawned. Skips the per-frame
/// string formatting otherwise.
fn character_info_changed(
    player: Query<(), ChangedCharacterInfo>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    added: Query<(), Added<HudText>>,
) -> bool {
    !player.is_empty()
        || job_registry.is_some_and(|registry| registry.is_changed())
        || !added.is_empty()
}

/// Builds the status frame under `parent`. Pickable-ignored throughout so clicks
/// pass through to the world behind it.
pub fn spawn_status_frame(commands: &mut Commands, parent: Entity, asset_server: &AssetServer) {
    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                width: Val::Px(286.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(13.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(11.0),
                border: UiRect {
                    left: Val::Px(2.0),
                    top: Val::Px(1.0),
                    right: Val::Px(1.0),
                    bottom: Val::Px(1.0),
                },
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.043, 0.067, 0.059, 0.93)),
            BorderColor {
                left: theme::EMERALD_DEEP,
                top: theme::GOLD_FAINT,
                right: theme::GOLD_FAINT,
                bottom: theme::GOLD_FAINT,
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    spawn_top(commands, frame, font_title, font_body.clone());
    spawn_bars(commands, frame, font_body.clone());
    spawn_exp(
        commands,
        frame,
        "BASE EXP",
        HudBar::BaseExp,
        HudText::BaseExp,
        theme::GOLD,
        font_body.clone(),
    );
    spawn_exp(
        commands,
        frame,
        "JOB EXP",
        HudBar::JobExp,
        HudText::JobExp,
        theme::EMERALD_BRI,
        font_body,
    );
}

/// Avatar + name + job/level sub-row.
fn spawn_top(
    commands: &mut Commands,
    frame: Entity,
    font_title: Handle<Font>,
    font_body: Handle<Font>,
) {
    let top = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(11.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(frame),
        ))
        .id();

    let avatar = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(AVATAR_BG),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(top),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(-3.0),
            left: Val::Px(-3.0),
            right: Val::Px(-3.0),
            bottom: Val::Px(-3.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BorderColor::all(AVATAR_RING),
        Pickable::IGNORE,
        ChildOf(avatar),
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
            font: font_title.clone(),
            font_size: 19.0,
            ..default()
        },
        TextColor(theme::GOLD),
        HudText::Avatar,
        Pickable::IGNORE,
        ChildOf(avatar),
    ));

    let id_col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                min_width: Val::Px(0.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(top),
        ))
        .id();
    commands.spawn((
        Text::new(""),
        TextFont {
            font: font_title,
            font_size: 18.0,
            ..default()
        },
        TextColor(theme::EMERALD_BRI),
        HudText::Name,
        Pickable::IGNORE,
        ChildOf(id_col),
    ));

    let sub = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(id_col),
        ))
        .id();
    commands.spawn((
        theme::label("", font_body.clone(), 11.5, theme::GOLD),
        HudText::Job,
        ChildOf(sub),
    ));
    commands.spawn((
        Node {
            width: Val::Px(3.0),
            height: Val::Px(3.0),
            border_radius: BorderRadius::all(Val::Percent(50.0)),
            ..default()
        },
        BackgroundColor(theme::TEXT_FAINT),
        Pickable::IGNORE,
        ChildOf(sub),
    ));
    lv_chip(commands, sub, "Base", HudText::BaseLevel, font_body.clone());
    lv_chip(commands, sub, "Job", HudText::JobLevel, font_body);
}

/// A "Base 1" / "Job 1" pair: faint label + bright number.
fn lv_chip(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    kind: HudText,
    font: Handle<Font>,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: font.clone(),
            font_size: 10.5,
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        theme::label("", font, 10.5, theme::TEXT),
        kind,
        ChildOf(row),
    ));
}

fn spawn_bars(commands: &mut Commands, frame: Entity, font_body: Handle<Font>) {
    let bars = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                margin: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(frame),
        ))
        .id();
    spawn_bar(
        commands,
        bars,
        "HP",
        HudBar::Hp,
        theme::EMERALD_BRI,
        font_body.clone(),
    );
    spawn_bar(
        commands,
        bars,
        "SP",
        HudBar::Sp,
        theme::MANA_BLUE,
        font_body,
    );
}

fn spawn_bar(
    commands: &mut Commands,
    parent: Entity,
    tag: &str,
    kind: HudBar,
    fill_color: Color,
    font: Handle<Font>,
) {
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(tag),
        TextFont {
            font: font.clone(),
            font_size: 9.5,
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            width: Val::Px(24.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(bar),
    ));
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(11.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(BAR_TRACK),
            BorderColor::all(theme::STROKE),
            Pickable::IGNORE,
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Node {
            width: Val::Percent(0.0),
            height: Val::Percent(100.0),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(fill_color),
        kind,
        Pickable::IGNORE,
        ChildOf(track),
    ));
    commands.spawn((
        theme::label("", font, 11.0, theme::TEXT_DIM),
        TextLayout::new_with_justify(Justify::Right),
        HudText::matching(kind),
        Node {
            min_width: Val::Px(56.0),
            ..default()
        },
        ChildOf(bar),
    ));
}

/// A thin EXP sliver: track + fill, with a `LABEL ... 12.4%` row beneath.
fn spawn_exp(
    commands: &mut Commands,
    frame: Entity,
    label: &str,
    bar_kind: HudBar,
    text_kind: HudText,
    fill_color: Color,
    font: Handle<Font>,
) {
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(frame),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(EXP_TRACK),
            Pickable::IGNORE,
            ChildOf(col),
        ))
        .id();
    commands.spawn((
        Node {
            width: Val::Percent(0.0),
            height: Val::Percent(100.0),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(fill_color),
        bar_kind,
        Pickable::IGNORE,
        ChildOf(track),
    ));
    let lbl = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(col),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: font.clone(),
            font_size: 9.0,
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Pickable::IGNORE,
        ChildOf(lbl),
    ));
    commands.spawn((
        theme::label("", font, 9.0, fill_color),
        text_kind,
        ChildOf(lbl),
    ));
}

impl HudText {
    fn matching(bar: HudBar) -> Self {
        match bar {
            HudBar::Hp => HudText::Hp,
            HudBar::Sp => HudText::Sp,
            HudBar::BaseExp => HudText::BaseExp,
            HudBar::JobExp => HudText::JobExp,
        }
    }
}

/// `current/max` as a 0..=100 percentage for a fill node's width.
fn percentage(current: u32, max: u32) -> f32 {
    if max == 0 {
        0.0
    } else {
        (current as f32 / max as f32) * 100.0
    }
}

fn update_character_info(
    player: Query<(&CharacterStatus, &CharacterData, Option<&EntityName>), With<LocalPlayer>>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut texts: Query<(&mut Text, &HudText)>,
    mut bars: Query<(&mut Node, &HudBar)>,
) {
    let Ok((status, data, entity_name)) = player.single() else {
        return;
    };

    let name = entity_name
        .map(|n| n.name.clone())
        .unwrap_or_else(|| data.name.clone());
    let job_name = job_registry
        .as_deref()
        .and_then(|registry| registry.get_display_name(data.job_id as u32))
        .unwrap_or("Unknown");
    let base_exp_pct = percentage(status.base_exp, status.next_base_exp);
    let job_exp_pct = percentage(status.job_exp, status.next_job_exp);

    for (mut text, kind) in &mut texts {
        let value = match kind {
            HudText::Avatar => job_name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
            HudText::Name => name.clone(),
            HudText::Job => job_name.to_string(),
            HudText::BaseLevel => status.base_level.to_string(),
            HudText::JobLevel => status.job_level.to_string(),
            HudText::Hp => format!("{} / {}", status.hp, status.max_hp),
            HudText::Sp => format!("{} / {}", status.sp, status.max_sp),
            HudText::BaseExp => format!("{base_exp_pct:.1}%"),
            HudText::JobExp => format!("{job_exp_pct:.1}%"),
        };
        if text.0 != value {
            *text = Text::new(value);
        }
    }

    for (mut node, kind) in &mut bars {
        let width = match kind {
            HudBar::Hp => Val::Percent(percentage(status.hp, status.max_hp)),
            HudBar::Sp => Val::Percent(percentage(status.sp, status.max_sp)),
            HudBar::BaseExp => Val::Percent(base_exp_pct),
            HudBar::JobExp => Val::Percent(job_exp_pct),
        };
        if node.width != width {
            node.width = width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::entities::character::components::core::CharacterStats;

    #[test]
    fn percentage_basic_ratios() {
        assert_eq!(percentage(50, 100), 50.0);
        assert_eq!(percentage(0, 0), 0.0);
        assert_eq!(percentage(100, 100), 100.0);
    }

    #[test]
    fn hp_bar_reflects_half_health() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let fill = app.world_mut().spawn((Node::default(), HudBar::Hp)).id();

        app.world_mut().spawn((
            CharacterStatus {
                hp: 50,
                max_hp: 100,
                ..default()
            },
            CharacterData {
                name: "Hero".to_string(),
                job_id: 0,
                level: 1,
                experience: 0,
                stats: CharacterStats::default(),
                slot: 0,
            },
            LocalPlayer,
        ));

        app.add_systems(Update, update_character_info);
        app.update();

        let width = app.world().get::<Node>(fill).unwrap().width;
        assert_eq!(width, Val::Percent(50.0));
    }
}
