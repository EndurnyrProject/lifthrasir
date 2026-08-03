//! Character-info status frame (top-left HUD): avatar, identity, HP/SP/AP,
//! Base/Job EXP, Zeny and Weight for the local player. The persisted size toggle
//! switches to the mockup's minimal layout without hiding live resource bars.
//! Built as raw `bevy_ui` by [`spawn_status_frame`] (called from the HUD root);
//! [`update_character_info`] reflects the `LocalPlayer`'s status into the marked
//! elements, writing only when a value actually changed so it doesn't churn
//! change detection every frame. Mirrors the Endurnir `.status-frame` design.

use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup};
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::job::player_jobs::is_fourth_job;
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
    Ap,
    BaseExp,
    JobExp,
    Zeny,
    Weight,
}

/// Tags a bar fill node so its width tracks the matching ratio.
#[derive(Component, Clone, Copy)]
enum HudBar {
    Hp,
    Sp,
    Ap,
    BaseExp,
    JobExp,
}

/// Layout pieces whose dimensions or display change in minimal mode.
#[derive(Component, Clone, Copy)]
enum HudLayout {
    Frame,
    Top,
    Avatar,
    AvatarInitial,
    Name,
    Bars,
    BarTag,
    BarTrack,
    BarValue,
    ExpandedOnly,
    MinimalOnly,
}

#[derive(Component)]
struct HudApRow;

/// HUD preferences persisted by Bevy's native settings plugin.
#[derive(Resource, SettingsGroup, Reflect, Default)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "hud")]
struct HudSettings {
    character_info_minimal: bool,
}

pub struct CharacterInfoPlugin;

impl Plugin for CharacterInfoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudSettings>().add_systems(
            Update,
            (
                update_character_info
                    .run_if(in_state(GameState::InGame).and_then(character_info_changed)),
                sync_character_info_mode.run_if(character_info_mode_changed),
            ),
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
            BackgroundColor(Color::srgb(0.043, 0.067, 0.059)),
            BorderColor {
                left: theme::EMERALD_DEEP,
                top: theme::GOLD_FAINT,
                right: theme::GOLD_FAINT,
                bottom: theme::GOLD_FAINT,
            },
            HudLayout::Frame,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    spawn_mode_toggle(commands, frame, asset_server);
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
        font_body.clone(),
    );
    spawn_meta(commands, frame, asset_server, font_body);
}

fn spawn_mode_toggle(commands: &mut Commands, frame: Entity, asset_server: &AssetServer) {
    let button = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.30)),
            BorderColor::all(theme::STROKE),
            Name::new("Toggle character info size"),
            ChildOf(frame),
        ))
        .observe(on_toggle_minimal)
        .id();
    commands.spawn((
        theme::icon(asset_server, "minus", 13.0, theme::TEXT_FAINT),
        HudLayout::ExpandedOnly,
        ChildOf(button),
    ));
    commands.spawn((
        theme::icon(asset_server, "plus", 13.0, theme::TEXT_FAINT),
        HudLayout::MinimalOnly,
        ChildOf(button),
    ));
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
            HudLayout::Top,
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
            HudLayout::Avatar,
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
            font: font_title.clone().into(),
            font_size: 19.0.into(),
            ..default()
        },
        TextColor(theme::GOLD),
        HudText::Avatar,
        HudLayout::AvatarInitial,
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
            font: font_title.into(),
            font_size: 18.0.into(),
            ..default()
        },
        TextColor(theme::EMERALD_BRI),
        HudText::Name,
        HudLayout::Name,
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
        HudLayout::ExpandedOnly,
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
        HudLayout::ExpandedOnly,
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
            font: font.clone().into(),
            font_size: 10.5.into(),
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
            HudLayout::Bars,
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
        font_body.clone(),
    );
    let ap = spawn_bar(commands, bars, "AP", HudBar::Ap, theme::GOLD, font_body);
    commands.entity(ap).insert(HudApRow);
}

fn spawn_bar(
    commands: &mut Commands,
    parent: Entity,
    tag: &str,
    kind: HudBar,
    fill_color: Color,
    font: Handle<Font>,
) -> Entity {
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
            font: font.clone().into(),
            font_size: 9.5.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        TextLayout {
            justify: Justify::Center,
            ..default()
        },
        Node {
            width: Val::Px(24.0),
            ..default()
        },
        HudLayout::BarTag,
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
            HudLayout::BarTrack,
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
        TextLayout {
            justify: Justify::Right,
            ..default()
        },
        HudText::matching(kind),
        Node {
            min_width: Val::Px(56.0),
            ..default()
        },
        HudLayout::BarValue,
        ChildOf(bar),
    ));
    bar
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
            HudLayout::ExpandedOnly,
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
            font: font.clone().into(),
            font_size: 9.0.into(),
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

fn spawn_meta(
    commands: &mut Commands,
    frame: Entity,
    asset_server: &AssetServer,
    font: Handle<Font>,
) {
    let meta = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::top(Val::Px(9.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(theme::STROKE),
            HudLayout::ExpandedOnly,
            Pickable::IGNORE,
            ChildOf(frame),
        ))
        .id();
    spawn_meta_cell(
        commands,
        meta,
        asset_server,
        "coin",
        "ZENY",
        HudText::Zeny,
        theme::GOLD,
        font.clone(),
    );
    spawn_meta_cell(
        commands,
        meta,
        asset_server,
        "bag",
        "WEIGHT",
        HudText::Weight,
        theme::TEXT_DIM,
        font,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_meta_cell(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    icon: &str,
    label: &str,
    text_kind: HudText,
    color: Color,
    font: Handle<Font>,
) {
    let cell = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((theme::icon(asset_server, icon, 12.0, color), ChildOf(cell)));
    commands.spawn((
        theme::label(label, font.clone(), 8.0, theme::TEXT_FAINT),
        ChildOf(cell),
    ));
    commands.spawn((
        theme::label("", font, 10.5, color),
        text_kind,
        ChildOf(cell),
    ));
}

impl HudText {
    fn matching(bar: HudBar) -> Self {
        match bar {
            HudBar::Hp => HudText::Hp,
            HudBar::Sp => HudText::Sp,
            HudBar::Ap => HudText::Ap,
            HudBar::BaseExp => HudText::BaseExp,
            HudBar::JobExp => HudText::JobExp,
        }
    }
}

fn character_info_mode_changed(
    settings: Res<HudSettings>,
    added: Query<(), Added<HudLayout>>,
) -> bool {
    settings.is_changed() || !added.is_empty()
}

fn sync_character_info_mode(
    settings: Res<HudSettings>,
    mut parts: Query<(&HudLayout, Option<&mut Node>, Option<&mut TextFont>)>,
) {
    let minimal = settings.character_info_minimal;
    for (part, node, font) in &mut parts {
        match part {
            HudLayout::Frame => {
                let Some(mut node) = node else { continue };
                node.padding = UiRect::axes(
                    Val::Px(if minimal { 10.0 } else { 14.0 }),
                    Val::Px(if minimal { 9.0 } else { 13.0 }),
                );
                node.row_gap = Val::Px(if minimal { 8.0 } else { 11.0 });
            }
            HudLayout::Top => {
                let Some(mut node) = node else { continue };
                node.padding.right = Val::Px(if minimal { 26.0 } else { 0.0 });
            }
            HudLayout::Avatar => {
                let Some(mut node) = node else { continue };
                let size = Val::Px(if minimal { 32.0 } else { 44.0 });
                node.width = size;
                node.height = size;
                node.border_radius = BorderRadius::all(Val::Px(if minimal { 8.0 } else { 10.0 }));
            }
            HudLayout::AvatarInitial => {
                let Some(mut font) = font else { continue };
                font.font_size = (if minimal { 14.0 } else { 19.0 }).into();
            }
            HudLayout::Name => {
                let Some(mut font) = font else { continue };
                font.font_size = (if minimal { 14.0 } else { 18.0 }).into();
            }
            HudLayout::Bars => {
                let Some(mut node) = node else { continue };
                node.row_gap = Val::Px(if minimal { 4.0 } else { 7.0 });
                node.margin.top = Val::Px(if minimal { 0.0 } else { 1.0 });
            }
            HudLayout::BarTag => {
                let (Some(mut node), Some(mut font)) = (node, font) else {
                    continue;
                };
                node.width = Val::Px(if minimal { 18.0 } else { 24.0 });
                font.font_size = (if minimal { 8.5 } else { 9.5 }).into();
            }
            HudLayout::BarTrack => {
                let Some(mut node) = node else { continue };
                node.height = Val::Px(if minimal { 7.0 } else { 11.0 });
            }
            HudLayout::BarValue => {
                let (Some(mut node), Some(mut font)) = (node, font) else {
                    continue;
                };
                node.min_width = Val::Px(if minimal { 0.0 } else { 56.0 });
                font.font_size = (if minimal { 9.5 } else { 11.0 }).into();
            }
            HudLayout::ExpandedOnly => {
                let Some(mut node) = node else { continue };
                node.display = if minimal {
                    Display::None
                } else {
                    Display::Flex
                };
            }
            HudLayout::MinimalOnly => {
                let Some(mut node) = node else { continue };
                node.display = if minimal {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

fn on_toggle_minimal(
    _: On<Pointer<Click>>,
    mut settings: ResMut<HudSettings>,
    mut commands: Commands,
) {
    settings.character_info_minimal = !settings.character_info_minimal;
    commands.queue(SaveSettings::IfChanged);
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
    mut bars: Query<(&mut Node, &HudBar), Without<HudApRow>>,
    mut ap_rows: Query<&mut Node, (With<HudApRow>, Without<HudBar>)>,
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
            HudText::Ap => format!("{} / {}", status.ap, status.max_ap),
            HudText::BaseExp => format!("{base_exp_pct:.1}%"),
            HudText::JobExp => format!("{job_exp_pct:.1}%"),
            HudText::Zeny => format!("{}z", status.zeny),
            HudText::Weight => format!("{} / {}", status.weight, status.max_weight),
        };
        if text.0 != value {
            *text = Text::new(value);
        }
    }

    for (mut node, kind) in &mut bars {
        let width = match kind {
            HudBar::Hp => Val::Percent(percentage(status.hp, status.max_hp)),
            HudBar::Sp => Val::Percent(percentage(status.sp, status.max_sp)),
            HudBar::Ap => Val::Percent(percentage(status.ap, status.max_ap)),
            HudBar::BaseExp => Val::Percent(base_exp_pct),
            HudBar::JobExp => Val::Percent(job_exp_pct),
        };
        if node.width != width {
            node.width = width;
        }
    }

    let ap_display = if is_fourth_job(data.job_id as u32) && status.max_ap > 0 {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut ap_rows {
        if node.display != ap_display {
            node.display = ap_display;
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
    fn hud_settings_use_the_persisted_hud_group() {
        assert_eq!(HudSettings::settings_group_name(), "hud");
        assert_eq!(HudSettings::settings_source(), None);
        assert!(!HudSettings::default().character_info_minimal);
    }

    #[test]
    fn minimal_mode_compacts_the_frame_and_hides_expanded_content() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(HudSettings {
            character_info_minimal: true,
        });

        let frame = app
            .world_mut()
            .spawn((Node::default(), HudLayout::Frame))
            .id();
        let expanded = app
            .world_mut()
            .spawn((Node::default(), HudLayout::ExpandedOnly))
            .id();
        let minimal = app
            .world_mut()
            .spawn((Node::default(), HudLayout::MinimalOnly))
            .id();

        app.add_systems(Update, sync_character_info_mode);
        app.update();

        let frame = app.world().get::<Node>(frame).unwrap();
        assert_eq!(frame.padding, UiRect::axes(Val::Px(10.0), Val::Px(9.0)));
        assert_eq!(frame.row_gap, Val::Px(8.0));
        assert_eq!(
            app.world().get::<Node>(expanded).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(minimal).unwrap().display,
            Display::Flex
        );
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

    #[test]
    fn ap_appears_only_for_a_fourth_job_with_an_ap_pool() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let fill = app.world_mut().spawn((Node::default(), HudBar::Ap)).id();
        let row = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                HudApRow,
            ))
            .id();
        let value = app.world_mut().spawn((Text::new(""), HudText::Ap)).id();
        let player = app
            .world_mut()
            .spawn((
                CharacterStatus {
                    ap: 50,
                    max_ap: 100,
                    ..default()
                },
                CharacterData {
                    name: "Hero".to_string(),
                    job_id: 4001,
                    level: 1,
                    experience: 0,
                    stats: CharacterStats::default(),
                    slot: 0,
                },
                LocalPlayer,
            ))
            .id();

        app.add_systems(Update, update_character_info);
        app.update();
        assert_eq!(app.world().get::<Node>(row).unwrap().display, Display::None);

        app.world_mut()
            .get_mut::<CharacterData>(player)
            .unwrap()
            .job_id = 4252;
        app.update();

        assert_eq!(app.world().get::<Node>(row).unwrap().display, Display::Flex);
        assert_eq!(
            app.world().get::<Node>(fill).unwrap().width,
            Val::Percent(50.0)
        );
        assert_eq!(app.world().get::<Text>(value).unwrap().0, "50 / 100");
    }

    #[test]
    fn resource_text_reflects_zeny_and_weight() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let zeny = app.world_mut().spawn((Text::new(""), HudText::Zeny)).id();
        let weight = app.world_mut().spawn((Text::new(""), HudText::Weight)).id();
        app.world_mut().spawn((
            CharacterStatus {
                zeny: 3420,
                weight: 214,
                max_weight: 800,
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

        assert_eq!(app.world().get::<Text>(zeny).unwrap().0, "3420z");
        assert_eq!(app.world().get::<Text>(weight).unwrap().0, "214 / 800");
    }
}
