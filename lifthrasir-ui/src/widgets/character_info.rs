//! Character-info panel: name, job, base/job level, and HP/SP bars for the local
//! player. Built as raw `bevy_ui` by [`spawn_status_frame`] (called from the HUD
//! root); [`update_character_info`] reflects the `LocalPlayer`'s status into the
//! marked elements, writing only when a value actually changed so it doesn't churn
//! change detection every frame.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::lua_scripts::job::registry::JobSpriteRegistry;

use crate::theme;

/// Tags a text element so [`update_character_info`] can write the matching value.
#[derive(Component, Clone, Copy)]
enum HudText {
    Name,
    Job,
    Level,
    Hp,
    Sp,
}

/// Tags a bar fill node so its width tracks the matching ratio.
#[derive(Component, Clone, Copy)]
enum HudBar {
    Hp,
    Sp,
}

pub struct CharacterInfoPlugin;

impl Plugin for CharacterInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_character_info.run_if(in_state(GameState::InGame)),
        );
    }
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
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.043, 0.067, 0.059, 0.93)),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    let top = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(frame),
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
        ChildOf(top),
    ));
    let id_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(top),
        ))
        .id();
    commands.spawn((
        text(font_body.clone(), 11.5, theme::GOLD),
        HudText::Job,
        ChildOf(id_row),
    ));
    commands.spawn((
        text(font_body.clone(), 10.5, theme::TEXT_FAINT),
        HudText::Level,
        ChildOf(id_row),
    ));

    let bars = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
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
                border_radius: BorderRadius::all(Val::Px(6.0)),
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
            width: Val::Percent(0.0),
            height: Val::Percent(100.0),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(fill_color),
        kind,
        Pickable::IGNORE,
        ChildOf(track),
    ));
    commands.spawn((
        text(font, 11.0, theme::TEXT_DIM),
        HudText::matching(kind),
        Node {
            min_width: Val::Px(56.0),
            ..default()
        },
        ChildOf(bar),
    ));
}

impl HudText {
    fn matching(bar: HudBar) -> Self {
        match bar {
            HudBar::Hp => HudText::Hp,
            HudBar::Sp => HudText::Sp,
        }
    }
}

fn text(font: Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(""),
        TextFont {
            font,
            font_size: size,
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    )
}

/// `current/max` as a 0..=100 percentage for the fill node's width.
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

    for (mut text, kind) in &mut texts {
        let value = match kind {
            HudText::Name => name.clone(),
            HudText::Job => job_name.to_string(),
            HudText::Level => format!(
                "Base Lv. {} / Job Lv. {}",
                status.base_level, status.job_level
            ),
            HudText::Hp => format!("HP {} / {}", status.hp, status.max_hp),
            HudText::Sp => format!("SP {} / {}", status.sp, status.max_sp),
        };
        if text.0 != value {
            *text = Text::new(value);
        }
    }

    for (mut node, kind) in &mut bars {
        let width = match kind {
            HudBar::Hp => Val::Percent(percentage(status.hp, status.max_hp)),
            HudBar::Sp => Val::Percent(percentage(status.sp, status.max_sp)),
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
