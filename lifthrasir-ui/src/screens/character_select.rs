//! Character selection screen rendered as a full-width sprite lineup and codex rail.

use bevy::{color::Color, prelude::*, ui::ColorStop};
use game_engine::{
    core::state::GameState,
    domain::character::events::{
        CharacterInfoWithJobName, CharacterListReceivedEvent, DeleteCharacterRequestEvent,
        RequestCharacterListEvent, SelectCharacterEvent,
    },
};
use net_contract::state::UserSession;

use crate::{
    screens::{
        character_create::CreationSlot,
        character_preview::{COLUMN_PX, CharacterDiorama, ROW_PX},
        character_scene::{backdrop, rail, stage, tokens},
    },
    theme,
};

pub struct CharacterSelectScreenPlugin;

impl Plugin for CharacterSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelectionData>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
        app.init_resource::<SelectedSlot>();
        app.add_systems(
            OnEnter(GameState::CharacterSelection),
            show_character_select_screen,
        );
        app.add_systems(
            Update,
            (receive_character_list, normalize_selection, rebuild_screen)
                .chain()
                .run_if(in_state(GameState::CharacterSelection)),
        );
    }
}

#[derive(Resource, Default)]
struct CharacterSelectionData {
    characters: Vec<Option<CharacterInfoWithJobName>>,
    max_slots: u8,
}

#[derive(Resource, Default)]
struct CardsBuilt(bool);

#[derive(Resource, Default)]
struct PendingDeletion(Option<u32>);

#[derive(Resource, Default)]
struct SelectedSlot(usize);

#[derive(Component)]
struct ScreenRoot;

#[derive(Component)]
struct SceneContent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotKind {
    Occupied,
    NewHero,
    Vacant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteAction {
    Armed,
    Confirmed,
}

fn show_character_select_screen(
    mut commands: Commands,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut selected: ResMut<SelectedSlot>,
    mut requests: MessageWriter<RequestCharacterListEvent>,
) {
    built.0 = false;
    pending.0 = None;
    selected.0 = 0;

    let root = commands
        .spawn((
            ScreenRoot,
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            DespawnOnExit(GameState::CharacterSelection),
        ))
        .id();
    commands
        .entity(root)
        .observe(|_: On<Pointer<Click>>, mut pending: ResMut<PendingDeletion>| pending.0 = None);
    requests.write(RequestCharacterListEvent);
}

fn receive_character_list(
    mut events: MessageReader<CharacterListReceivedEvent>,
    mut data: ResMut<CharacterSelectionData>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
) {
    let Some(event) = events.read().last() else {
        return;
    };
    data.characters = event.characters.clone();
    data.max_slots = event.max_slots;
    built.0 = false;
    pending.0 = None;
}

fn normalize_selection(
    data: Res<CharacterSelectionData>,
    mut selected: ResMut<SelectedSlot>,
    mut pending: ResMut<PendingDeletion>,
) {
    if !data.is_changed() {
        return;
    }
    let selection_still_occupied = data.characters.get(selected.0).is_some_and(Option::is_some);
    if !selection_still_occupied {
        selected.0 = data
            .characters
            .iter()
            .take(data.max_slots as usize)
            .position(Option::is_some)
            .unwrap_or(0);
    }
    pending.0 = None;
}

#[allow(clippy::too_many_arguments)]
fn rebuild_screen(
    mut commands: Commands,
    assets: Res<AssetServer>,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    session: Option<Res<UserSession>>,
    selected: Res<SelectedSlot>,
    pending: Res<PendingDeletion>,
    mut built: ResMut<CardsBuilt>,
    root: Query<Entity, With<ScreenRoot>>,
    old_content: Query<Entity, With<SceneContent>>,
) {
    if data.max_slots == 0 {
        return;
    }
    let has_occupied = occupied_count(&data) > 0;
    if has_occupied && diorama.target.is_none() {
        return;
    }
    if built.0 && !selected.is_changed() && !pending.is_changed() && !diorama.is_changed() {
        return;
    }
    let Ok(root) = root.single() else {
        return;
    };
    for entity in &old_content {
        commands.entity(entity).despawn();
    }

    let featured = featured(&data.characters, selected.0);
    let hue = featured
        .map(|info| tokens::class_hue(info.base.class))
        .unwrap_or(theme::EMERALD);
    let realm = session
        .as_ref()
        .and_then(|session| session.selected_server.as_ref())
        .map(|server| server.name.as_str())
        .unwrap_or("Endurnir");

    let content = commands
        .spawn((
            SceneContent,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((backdrop::key_light(hue), ChildOf(content)));
    commands.spawn((backdrop::gold_rim(), ChildOf(content)));
    commands.spawn((backdrop::grade(), ChildOf(content)));
    commands.spawn((backdrop::vignette(), ChildOf(content)));
    commands.spawn((backdrop::grain(&assets), ChildOf(content)));

    spawn_header(&mut commands, &assets, content);
    spawn_identity(&mut commands, &assets, content, &data, featured, realm);
    commands.spawn((stage::horizon_line(), ChildOf(content)));
    spawn_lineup(&mut commands, &assets, content, &data, &diorama, selected.0);
    spawn_codex(
        &mut commands,
        &assets,
        content,
        featured,
        selected.0 as u8,
        pending.0,
        realm,
        hue,
    );
    built.0 = true;
}

fn spawn_header(commands: &mut Commands, assets: &AssetServer, parent: Entity) {
    let header = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(44),
                top: px(38),
                align_items: AlignItems::FlexStart,
                column_gap: px(22),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let back = commands
        .spawn((
            Pickable::default(),
            Node {
                height: px(32),
                align_items: AlignItems::Center,
                column_gap: px(8),
                padding: UiRect::horizontal(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.04)),
            BorderColor::all(theme::STROKE),
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        theme::icon(assets, "back", 15.0, theme::TEXT_DIM),
        ChildOf(back),
    ));
    commands.spawn((
        mono_text(assets, "Realms", 10.0, theme::TEXT_DIM),
        ChildOf(back),
    ));
    commands.entity(back).observe(
        |mut click: On<Pointer<Click>>, mut next: ResMut<NextState<GameState>>| {
            click.propagate(false);
            next.set(GameState::ServerSelection);
        },
    );

    let title = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        mono_text(assets, &tokens::mono_label("Endurnir"), 9.5, theme::GOLD),
        ChildOf(title),
    ));
    commands.spawn((
        title_text(assets, "CHOOSE YOUR HERO", 23.0, theme::TEXT),
        ChildOf(title),
    ));
}

fn spawn_identity(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    data: &CharacterSelectionData,
    featured: Option<&CharacterInfoWithJobName>,
    realm: &str,
) {
    commands.spawn((
        mono_text(
            assets,
            &roster_hint(occupied_count(data), data.max_slots),
            11.0,
            theme::TEXT_DIM,
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(48),
            top: px(170),
            ..default()
        },
        ChildOf(parent),
    ));
    let block = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(48),
                top: px(212),
                flex_direction: FlexDirection::Column,
                row_gap: px(7),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    if let Some(info) = featured {
        commands.spawn((
            mono_text(
                assets,
                &tokens::mono_label(&info.job_name),
                10.0,
                theme::GOLD,
            ),
            ChildOf(block),
        ));
        commands.spawn((
            title_text(assets, &info.base.name, 58.0, theme::TEXT),
            ChildOf(block),
        ));
        commands.spawn((
            mono_text(
                assets,
                &format!(
                    "{}  ·  LEVEL {}  ·  {realm}",
                    info.job_name.to_uppercase(),
                    info.base.base_level
                ),
                11.0,
                tokens::class_hue(info.base.class),
            ),
            ChildOf(block),
        ));
    } else {
        commands.spawn((
            mono_text(assets, "YOUR STORY AWAITS", 10.0, theme::GOLD),
            ChildOf(block),
        ));
        commands.spawn((
            title_text(assets, "Forge your first hero", 42.0, theme::TEXT),
            ChildOf(block),
        ));
        commands.spawn((
            mono_text(
                assets,
                "CHOOSE A VACANT SLOT TO BEGIN",
                11.0,
                theme::TEXT_DIM,
            ),
            ChildOf(block),
        ));
    }
}

fn spawn_lineup(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    data: &CharacterSelectionData,
    diorama: &CharacterDiorama,
    selected: usize,
) {
    let slots = data.max_slots.max(1);
    let lineup = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(34),
                right: px(470),
                bottom: px(38),
                height: px(250),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(slots as u16, 1.0),
                column_gap: px(4),
                align_items: AlignItems::End,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let first_vacant = (0..slots as usize).find(|&slot| featured(&data.characters, slot).is_none());
    for slot in 0..slots as usize {
        match slot_kind(data, slot) {
            SlotKind::Occupied => spawn_occupied_slot(
                commands,
                assets,
                lineup,
                slot,
                data.characters[slot]
                    .as_ref()
                    .expect("occupied slot has data"),
                diorama,
                slot == selected,
            ),
            SlotKind::NewHero | SlotKind::Vacant => spawn_vacant_slot(
                commands,
                assets,
                lineup,
                slot as u8,
                first_vacant == Some(slot),
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_occupied_slot(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    slot: usize,
    info: &CharacterInfoWithJobName,
    diorama: &CharacterDiorama,
    selected: bool,
) {
    let hue = tokens::class_hue(info.base.class);
    let card = commands
        .spawn((
            Pickable::default(),
            Node {
                min_width: px(0),
                height: px(250),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.entity(card).observe(
        move |mut click: On<Pointer<Click>>,
              mut selected_slot: ResMut<SelectedSlot>,
              mut pending: ResMut<PendingDeletion>| {
            click.propagate(false);
            selected_slot.0 = slot;
            pending.0 = None;
        },
    );

    let decor_visibility = if selected {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.spawn((
        stage::spot_beam(assets, hue),
        decor_visibility,
        ChildOf(card),
    ));
    commands.spawn((stage::spot_glow(hue), decor_visibility, ChildOf(card)));
    commands.spawn((
        stage::spot_ring(assets, hue),
        decor_visibility,
        ChildOf(card),
    ));
    commands.spawn((
        stage::spot_ring_thin(assets, hue),
        decor_visibility,
        ChildOf(card),
    ));
    commands.spawn((stage::ground_shadow(), ChildOf(card)));

    let pending_delete = info.base.delete_date != 0;
    let mut image = ImageNode {
        image: diorama.target.clone().unwrap_or_default(),
        rect: diorama.columns.get(&(slot as u8)).copied(),
        ..default()
    };
    if pending_delete {
        image.color = Color::srgba(0.48, 0.52, 0.50, 0.58);
    }
    commands.spawn((
        image,
        Node {
            width: px(if selected { COLUMN_PX as f32 } else { 100.0 }),
            height: px(if selected { ROW_PX as f32 } else { 156.0 }),
            margin: UiRect::bottom(px(-8)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(card),
    ));
    spawn_nameplate(commands, assets, card, info, selected, pending_delete);
}

fn spawn_nameplate(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    info: &CharacterInfoWithJobName,
    selected: bool,
    pending_delete: bool,
) {
    let plate = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(48),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(px(9)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.016, 0.031, 0.027, 0.62)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        title_text(
            assets,
            &info.base.name,
            13.5,
            if selected {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            },
        ),
        Node {
            max_width: percent(100),
            overflow: Overflow::clip(),
            ..default()
        },
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
        ChildOf(plate),
    ));
    let caption = if pending_delete {
        "DELETION PENDING".to_string()
    } else {
        format!("LV {}", info.base.base_level)
    };
    commands.spawn((
        mono_text(
            assets,
            &caption,
            10.0,
            if pending_delete {
                theme::BAD
            } else {
                theme::GOLD
            },
        ),
        ChildOf(plate),
    ));
}

fn spawn_vacant_slot(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    slot: u8,
    is_new: bool,
) {
    let card = commands
        .spawn((
            Pickable::default(),
            Node {
                min_width: px(0),
                height: px(160),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.entity(card).observe(
        move |mut click: On<Pointer<Click>>,
              mut commands: Commands,
              mut pending: ResMut<PendingDeletion>,
              mut next: ResMut<NextState<GameState>>| {
            click.propagate(false);
            pending.0 = None;
            commands.insert_resource(CreationSlot(slot));
            next.set(GameState::CharacterCreation);
        },
    );
    commands.spawn((
        ImageNode {
            image: assets.load(tokens::VACANT_PAD),
            color: if is_new {
                theme::GOLD.with_alpha(0.5)
            } else {
                theme::TEXT_FAINT.with_alpha(0.35)
            },
            ..default()
        },
        Node {
            width: px(64),
            height: px(64),
            margin: UiRect::bottom(px(6)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(card),
    ));
    let plus = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(74),
                width: px(32),
                height: px(32),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(if is_new {
                theme::GOLD_FAINT
            } else {
                theme::STROKE
            }),
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        theme::icon(
            assets,
            "plus",
            16.0,
            if is_new {
                theme::GOLD
            } else {
                theme::TEXT_FAINT
            },
        ),
        ChildOf(plus),
    ));
    commands.spawn((
        mono_text(
            assets,
            if is_new { "NEW HERO" } else { "EMPTY" },
            10.5,
            if is_new {
                theme::GOLD
            } else {
                theme::TEXT_FAINT
            },
        ),
        ChildOf(card),
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_codex(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    featured: Option<&CharacterInfoWithJobName>,
    slot: u8,
    armed: Option<u32>,
    realm: &str,
    hue: Color,
) {
    let rail_host = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                top: px(0),
                width: px(452),
                height: percent(100),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let rail_entity = commands
        .spawn((rail::rail_container(), ChildOf(rail_host)))
        .id();
    commands.spawn((
        rail::rail_header(assets, "Hero Codex", realm),
        ChildOf(rail_entity),
    ));
    let Some(info) = featured else {
        spawn_empty_codex(commands, assets, rail_entity);
        return;
    };

    spawn_crest(commands, assets, rail_entity, info, hue);
    commands.spawn((rail::gold_rule(), ChildOf(rail_entity)));
    spawn_progress(
        commands,
        assets,
        rail_entity,
        "JOB LEVEL",
        info.base.job_level,
        tokens::job_level_cap(info.base.class),
        hue,
    );
    commands.spawn((
        rail::section_label(assets, "Attributes"),
        ChildOf(rail_entity),
    ));
    for (name, value) in [
        ("STR", info.base.str),
        ("AGI", info.base.agi),
        ("VIT", info.base.vit),
        ("INT", info.base.int),
        ("DEX", info.base.dex),
        ("LUK", info.base.luk),
    ] {
        spawn_stat(commands, assets, rail_entity, name, value, hue);
    }
    spawn_footer(commands, assets, rail_entity, info, slot, armed);
}

fn spawn_empty_codex(commands: &mut Commands, assets: &AssetServer, parent: Entity) {
    let crest = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                margin: UiRect::top(px(55)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        title_text(assets, "◇", 54.0, theme::GOLD_FAINT),
        ChildOf(crest),
    ));
    commands.spawn((
        title_text(assets, "AN EMPTY CHAPTER", 18.0, theme::TEXT_DIM),
        ChildOf(crest),
    ));
    commands.spawn((
        mono_text(
            assets,
            "FORGE A HERO TO FILL THE CODEX",
            10.0,
            theme::TEXT_FAINT,
        ),
        ChildOf(crest),
    ));
}

fn spawn_crest(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    info: &CharacterInfoWithJobName,
    hue: Color,
) {
    let crest = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(10),
                margin: UiRect::bottom(px(22)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let mark = commands
        .spawn((
            Node {
                width: px(72),
                height: px(72),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            UiTransform {
                rotation: Rot2::degrees(45.0),
                ..default()
            },
            BackgroundColor(hue.with_alpha(0.06)),
            BorderColor::all(hue.with_alpha(0.38)),
            ChildOf(crest),
        ))
        .id();
    let glyph = info
        .job_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    commands.spawn((
        title_text(assets, &glyph, 36.0, hue),
        UiTransform {
            rotation: Rot2::degrees(-45.0),
            ..default()
        },
        ChildOf(mark),
    ));
    commands.spawn((
        title_text(assets, &info.job_name.to_uppercase(), 19.0, theme::TEXT),
        ChildOf(crest),
    ));
    commands.spawn((
        mono_text(
            assets,
            &format!(
                "BASE LV {}  ·  JOB LV {}",
                info.base.base_level, info.base.job_level
            ),
            10.0,
            theme::TEXT_DIM,
        ),
        ChildOf(crest),
    ));
}

fn spawn_progress(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    label: &str,
    value: u32,
    cap: u32,
    hue: Color,
) {
    let block = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ChildOf(block),
        ))
        .id();
    commands.spawn((mono_text(assets, label, 9.5, theme::TEXT_DIM), ChildOf(row)));
    commands.spawn((
        mono_text(assets, &format!("{value} / {cap}"), 10.0, theme::GOLD),
        ChildOf(row),
    ));
    spawn_track(commands, block, job_level_fraction(value, cap), 5.0, hue);
}

fn spawn_stat(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    name: &str,
    value: u8,
    hue: Color,
) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(25),
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        mono_text(assets, name, 10.0, theme::TEXT_DIM),
        Node {
            width: px(34),
            ..default()
        },
        ChildOf(row),
    ));
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: px(4),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.07)),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        Node {
            width: percent((value as f32 / 99.0).clamp(0.0, 1.0) * 100.0),
            height: percent(100),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::new(hue.with_alpha(0.42), percent(0)),
            ColorStop::new(hue, percent(100)),
        ])),
        ChildOf(track),
    ));
    commands.spawn((
        mono_text(assets, &value.to_string(), 11.0, theme::TEXT),
        Node {
            width: px(25),
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        ChildOf(row),
    ));
}

fn spawn_track(commands: &mut Commands, parent: Entity, fraction: f32, height: f32, hue: Color) {
    let track = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(height),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.07)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            width: percent(fraction * 100.0),
            height: percent(100),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::new(hue.with_alpha(0.45), percent(0)),
            ColorStop::new(hue, percent(100)),
        ])),
        ChildOf(track),
    ));
}

fn spawn_footer(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    info: &CharacterInfoWithJobName,
    slot: u8,
    armed: Option<u32>,
) {
    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                margin: UiRect::top(auto()),
                padding: UiRect::top(px(26)),
                column_gap: px(10),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    let deletion_pending = info.base.delete_date != 0;
    let enter = commands
        .spawn((
            Pickable {
                is_hoverable: !deletion_pending,
                should_block_lower: true,
            },
            Node {
                height: px(56),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(9),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::new(
                    if deletion_pending {
                        theme::GLASS_2
                    } else {
                        Color::srgb_u8(0x1f, 0x91, 0x59)
                    },
                    percent(0),
                ),
                ColorStop::new(
                    if deletion_pending {
                        theme::GLASS
                    } else {
                        Color::srgb_u8(0x0b, 0x5c, 0x36)
                    },
                    percent(100),
                ),
            ])),
            BorderColor::all(if deletion_pending {
                theme::STROKE
            } else {
                theme::GOLD_FAINT
            }),
            ChildOf(footer),
        ))
        .id();
    commands.spawn((
        theme::icon(
            assets,
            "play",
            15.0,
            if deletion_pending {
                theme::TEXT_FAINT
            } else {
                theme::TEXT
            },
        ),
        ChildOf(enter),
    ));
    commands.spawn((
        mono_text(
            assets,
            if deletion_pending {
                "DELETION PENDING"
            } else {
                "ENTER GAME"
            },
            12.0,
            if deletion_pending {
                theme::TEXT_FAINT
            } else {
                theme::TEXT
            },
        ),
        ChildOf(enter),
    ));
    if !deletion_pending {
        commands.entity(enter).observe(
            move |mut click: On<Pointer<Click>>,
                  mut writer: MessageWriter<SelectCharacterEvent>| {
                click.propagate(false);
                writer.write(SelectCharacterEvent { slot });
            },
        );
    }

    let character_id = info.base.char_id;
    let is_armed = armed == Some(character_id);
    let delete = commands
        .spawn((
            Pickable::default(),
            Node {
                width: px(56),
                height: px(56),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(if is_armed {
                theme::BAD.with_alpha(0.28)
            } else {
                theme::BAD.with_alpha(0.10)
            }),
            BorderColor::all(theme::BAD),
            ChildOf(footer),
        ))
        .id();
    if is_armed {
        commands.spawn((
            mono_text(assets, "CONFIRM?", 8.0, theme::BAD),
            ChildOf(delete),
        ));
    } else {
        commands.spawn((
            theme::icon(assets, "trash", 17.0, theme::BAD),
            ChildOf(delete),
        ));
    }
    commands.entity(delete).observe(
        move |mut click: On<Pointer<Click>>,
              mut pending: ResMut<PendingDeletion>,
              mut writer: MessageWriter<DeleteCharacterRequestEvent>| {
            click.propagate(false);
            if arm_delete(&mut pending.0, character_id) == DeleteAction::Confirmed {
                writer.write(DeleteCharacterRequestEvent { character_id });
            }
        },
    );
}

fn title_text(assets: &AssetServer, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont {
            font: assets.load(theme::FONT_TITLE).into(),
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
    )
}

fn mono_text(assets: &AssetServer, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont {
            font: assets.load(theme::FONT_MONO).into(),
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
    )
}

fn featured(
    characters: &[Option<CharacterInfoWithJobName>],
    selected: usize,
) -> Option<&CharacterInfoWithJobName> {
    characters.get(selected).and_then(Option::as_ref)
}

fn occupied_count(data: &CharacterSelectionData) -> usize {
    data.characters
        .iter()
        .take(data.max_slots as usize)
        .filter(|entry| entry.is_some())
        .count()
}

fn slot_kind(data: &CharacterSelectionData, slot: usize) -> SlotKind {
    if featured(&data.characters, slot).is_some() {
        SlotKind::Occupied
    } else if (0..slot).all(|prior| featured(&data.characters, prior).is_some()) {
        SlotKind::NewHero
    } else {
        SlotKind::Vacant
    }
}

fn arm_delete(pending: &mut Option<u32>, character_id: u32) -> DeleteAction {
    if *pending == Some(character_id) {
        *pending = None;
        DeleteAction::Confirmed
    } else {
        *pending = Some(character_id);
        DeleteAction::Armed
    }
}

fn roster_hint(occupied: usize, max_slots: u8) -> String {
    format!("ROSTER · {occupied} OF {max_slots} SLOTS")
}

fn job_level_fraction(level: u32, cap: u32) -> f32 {
    if cap == 0 {
        0.0
    } else {
        (level as f32 / cap as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::CharacterInfo;

    #[test]
    fn slot_layout_marks_first_vacancy_as_new_hero() {
        let data = CharacterSelectionData {
            characters: vec![
                Some(with_job("Hero", 0)),
                None,
                Some(with_job("Mage", 2)),
                None,
            ],
            max_slots: 4,
        };
        assert_eq!(slot_kind(&data, 0), SlotKind::Occupied);
        assert_eq!(slot_kind(&data, 1), SlotKind::NewHero);
        assert_eq!(slot_kind(&data, 2), SlotKind::Occupied);
        assert_eq!(slot_kind(&data, 3), SlotKind::Vacant);
    }

    #[test]
    fn delete_arms_confirms_and_rearms_for_another_character() {
        let mut pending = None;
        assert_eq!(arm_delete(&mut pending, 7), DeleteAction::Armed);
        assert_eq!(pending, Some(7));
        assert_eq!(arm_delete(&mut pending, 8), DeleteAction::Armed);
        assert_eq!(pending, Some(8));
        assert_eq!(arm_delete(&mut pending, 8), DeleteAction::Confirmed);
        assert_eq!(pending, None);
    }

    #[test]
    fn roster_label_formats_counts() {
        assert_eq!(roster_hint(3, 12), "ROSTER · 3 OF 12 SLOTS");
    }

    #[test]
    fn job_level_progress_is_clamped() {
        assert_eq!(job_level_fraction(25, 50), 0.5);
        assert_eq!(job_level_fraction(80, 50), 1.0);
        assert_eq!(job_level_fraction(1, 0), 0.0);
    }

    fn with_job(name: &str, slot: u8) -> CharacterInfoWithJobName {
        CharacterInfoWithJobName {
            base: CharacterInfo {
                name: name.into(),
                char_num: slot,
                ..default()
            },
            job_name: "Novice".into(),
            body_sprite_path: String::new(),
            hair_sprite_path: String::new(),
            hair_palette_path: None,
        }
    }
}
