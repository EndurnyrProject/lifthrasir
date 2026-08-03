//! Character creation screen rendered as the Forge stage and form rail.

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{prelude::*, text::EditableText, ui::ColorStop};
use game_engine::{
    core::state::GameState,
    domain::{
        character::{
            events::{
                CharacterCreatedEvent, CharacterCreationFailedEvent, CreateCharacterRequestEvent,
            },
            forms::CharacterCreationForm,
        },
        entities::character::{
            SpawnCharacterSpriteEvent,
            components::{
                CharacterAppearance, CharacterData, CharacterStats, Gender,
                visual::{CharacterDirection, CharacterSprite},
            },
            events::forward_character_sprite_events,
        },
    },
};
use net_contract::state::UserSession;

use crate::{
    screens::{
        character_preview::{PreviewCamera, spawn_preview_diorama},
        character_scene::{backdrop, rail, stage, tokens},
    },
    theme,
    widgets::placeholder::Placeholder,
};

const NAME_MIN: usize = 4;
const NAME_MAX: usize = 16;
const HAIR_STYLE_MIN: u16 = 1;
const HAIR_STYLE_MAX: u16 = 25;
const HAIR_COLOR_MIN: u16 = 0;
const HAIR_COLOR_MAX: u16 = 8;

// Entry 219 sampled from each retail style-1 male palette.
const HAIR_SWATCHES: [Color; 9] = [
    Color::srgb_u8(0xe9, 0x7e, 0x7a),
    Color::srgb_u8(0xeb, 0xa1, 0x71),
    Color::srgb_u8(0xa3, 0x87, 0x99),
    Color::srgb_u8(0xba, 0x8a, 0x75),
    Color::srgb_u8(0x80, 0xa3, 0x87),
    Color::srgb_u8(0xab, 0xa3, 0xbf),
    Color::srgb_u8(0xca, 0xb7, 0xaf),
    Color::srgb_u8(0x99, 0x72, 0x6f),
    Color::srgb_u8(0xe9, 0x7e, 0x7a),
];

pub struct CharacterCreateScreenPlugin;

impl Plugin for CharacterCreateScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreationSlot>();
        app.init_resource::<CreationForm>();
        app.init_resource::<CreatePreview>();
        app.init_resource::<NameServerError>();
        app.add_systems(
            OnEnter(GameState::CharacterCreation),
            show_character_create_screen,
        );
        app.add_systems(OnExit(GameState::CharacterCreation), teardown_preview);
        app.add_systems(
            Update,
            stage::breathe_spot_glow.run_if(in_state(GameState::CharacterCreation)),
        );
        app.add_systems(
            Update,
            (
                surface_creation_failure,
                reflect_form_values,
                return_to_character_select,
            )
                .chain()
                .run_if(in_state(GameState::CharacterCreation)),
        );
        // Keep this after forwarding so deferred preview components exist before
        // the sprite event is consumed on the following frame.
        app.add_systems(
            Update,
            rebuild_preview_character
                .after(forward_character_sprite_events)
                .run_if(in_state(GameState::CharacterCreation)),
        );
    }
}

#[derive(Resource, Default)]
pub struct CreationSlot(pub u8);

#[derive(Resource, Default)]
struct CreationForm(CharacterCreationForm);

#[derive(Resource, Default)]
struct CreatePreview {
    target: Option<Handle<Image>>,
}

#[derive(Resource, Default)]
struct NameServerError(Option<String>);

#[derive(Component)]
struct CreatePreviewCharacter;
#[derive(Component)]
struct NameField;
#[derive(Component)]
struct NameStatus;
#[derive(Component)]
struct NameTrailing;
#[derive(Component)]
struct CreateButton;
#[derive(Component)]
struct HairSwatch(u16);
#[derive(Component)]
struct SexOption(Gender);

#[derive(Component, Clone, Copy)]
enum FormValue {
    HairStyle,
    HairStyleCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameState {
    Idle,
    TooShort,
    Invalid,
    Ok,
    ServerError,
}

fn name_state(name: &str, server_error: Option<&str>) -> NameState {
    if server_error.is_some() {
        return NameState::ServerError;
    }
    if name.is_empty() {
        return NameState::Idle;
    }
    if name.chars().count() < NAME_MIN {
        return NameState::TooShort;
    }
    let valid = name.chars().all(|c| c.is_alphanumeric() || c == '_');
    if valid {
        NameState::Ok
    } else {
        NameState::Invalid
    }
}

fn show_character_create_screen(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut form: ResMut<CreationForm>,
    mut preview: ResMut<CreatePreview>,
    mut server_error: ResMut<NameServerError>,
    session: Option<Res<UserSession>>,
) {
    *form = CreationForm::default();
    server_error.0 = None;
    let target = spawn_preview_diorama(&mut commands, &mut images, 1, 2.0);
    preview.target = Some(target.clone());
    let realm = session
        .as_ref()
        .and_then(|session| session.selected_server.as_ref())
        .map(|server| server.name.as_str())
        .unwrap_or("Endurnir");

    let root = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            DespawnOnExit(GameState::CharacterCreation),
        ))
        .id();
    commands.spawn((backdrop::key_light(theme::EMERALD), ChildOf(root)));
    commands.spawn((backdrop::gold_rim(), ChildOf(root)));
    commands.spawn((backdrop::grade(), ChildOf(root)));
    commands.spawn((backdrop::vignette(), ChildOf(root)));
    commands.spawn((backdrop::grain(&assets), ChildOf(root)));

    spawn_header(&mut commands, &assets, root);
    spawn_stage(&mut commands, &assets, root, target);
    spawn_form_rail(&mut commands, &assets, root, realm, &form.0);
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
        mono_text(assets, "Characters", 10.0, theme::TEXT_DIM),
        tokens::scenic_text_shadow(),
        ChildOf(back),
    ));
    commands.entity(back).observe(return_on_click);

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
        tokens::scenic_text_shadow(),
        ChildOf(title),
    ));
    commands.spawn((
        title_text(assets, "FORGE A HERO", 23.0, theme::TEXT),
        tokens::scenic_text_shadow(),
        ChildOf(title),
    ));
}

fn spawn_stage(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    target: Handle<Image>,
) {
    let stage_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(80),
                right: px(500),
                top: px(100),
                bottom: px(30),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let preview = commands
        .spawn((
            Node {
                width: px(360),
                height: px(540),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(stage_root),
        ))
        .id();
    commands.spawn((stage::spot_glow(theme::EMERALD), ChildOf(preview)));
    commands.spawn((stage::spot_ring(assets, theme::EMERALD), ChildOf(preview)));
    commands.spawn((
        stage::spot_ring_thin(assets, theme::EMERALD),
        ChildOf(preview),
    ));
    commands.spawn((stage::spot_beam(assets, theme::EMERALD), ChildOf(preview)));
    commands.spawn((stage::ground_shadow(), ChildOf(preview)));
    commands.spawn((
        ImageNode::new(target),
        Node {
            width: px(288),
            height: px(448),
            margin: UiRect::bottom(px(22)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(preview),
    ));
    commands.spawn((
        mono_text(
            assets,
            "LIVE PREVIEW · SPRITE RENDER",
            9.0,
            theme::TEXT_FAINT,
        ),
        tokens::scenic_text_shadow(),
        ChildOf(preview),
    ));
}

fn spawn_form_rail(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    realm: &str,
    form: &CharacterCreationForm,
) {
    let rail_host = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                width: px(452),
                height: percent(100),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let rail = commands
        .spawn((rail::rail_container(), ChildOf(rail_host)))
        .id();
    commands.spawn((
        rail::rail_header(assets, "New Character", realm),
        ChildOf(rail),
    ));
    commands.spawn((rail::gold_rule(), ChildOf(rail)));

    commands.spawn((field_label(assets, "NAME"), ChildOf(rail)));
    let name_row = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(48),
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::horizontal(px(13)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        theme::icon(assets, "user", 15.0, theme::TEXT_FAINT),
        ChildOf(name_row),
    ));
    let name = commands
        .spawn((
            EditableText {
                max_characters: Some(NAME_MAX),
                ..default()
            },
            TextFont {
                font: assets.load(theme::FONT_BODY).into(),
                font_size: 14.0.into(),
                ..default()
            },
            TextColor(theme::TEXT),
            NameField,
            Node {
                flex_grow: 1.0,
                height: px(23),
                ..default()
            },
            ChildOf(name_row),
        ))
        .id();
    commands.spawn((
        Text::new("Name your hero"),
        TextFont {
            font: assets.load(theme::FONT_BODY).into(),
            font_size: 14.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Pickable::IGNORE,
        Placeholder(name),
        ChildOf(name),
    ));
    commands.spawn((
        mono_text(assets, "0/16", 9.0, theme::TEXT_FAINT),
        NameTrailing,
        ChildOf(name_row),
    ));
    commands.spawn((
        mono_text(assets, "", 10.0, theme::TEXT_FAINT),
        NameStatus,
        Node {
            min_height: px(18),
            margin: UiRect::top(px(6)),
            ..default()
        },
        ChildOf(rail),
    ));

    commands.spawn((rail::section_label(assets, "Sex"), ChildOf(rail)));
    spawn_sex_segment(commands, assets, rail, form.sex);
    commands.spawn((rail::section_label(assets, "Hair Style"), ChildOf(rail)));
    spawn_style_stepper(commands, assets, rail, form.hair_style);
    commands.spawn((rail::section_label(assets, "Hair Color"), ChildOf(rail)));
    spawn_swatches(commands, rail, form.hair_color);

    let random = commands
        .spawn((
            Pickable::default(),
            Node {
                height: px(34),
                margin: UiRect::top(px(18)),
                padding: UiRect::horizontal(px(13)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.03)),
            BorderColor::all(theme::STROKE),
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        mono_text(assets, "Random", 10.0, theme::TEXT_DIM),
        ChildOf(random),
    ));
    commands.entity(random).observe(randomize_form);

    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                margin: UiRect::top(auto()),
                padding: UiRect::top(px(20)),
                column_gap: px(10),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(rail),
        ))
        .id();
    let cancel = action_button(commands, footer, false);
    commands.spawn((
        mono_text(assets, "Cancel", 11.0, theme::TEXT_DIM),
        ChildOf(cancel),
    ));
    commands.entity(cancel).observe(return_on_click);
    let create = action_button(commands, footer, true);
    commands
        .entity(create)
        .insert((CreateButton, Pickable::IGNORE));
    commands.spawn((
        mono_text(assets, "Create Hero", 11.0, theme::EMERALD_INK),
        ChildOf(create),
    ));
    commands.entity(create).observe(create_character);
}

fn spawn_sex_segment(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    selected: Gender,
) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(42),
                padding: UiRect::all(px(3)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(parent),
        ))
        .id();
    for sex in [Gender::Male, Gender::Female] {
        let button = commands
            .spawn((
                Pickable::default(),
                SexOption(sex),
                Node {
                    flex_grow: 1.0,
                    height: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(7)),
                    ..default()
                },
                segment_gradient(sex == selected),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            mono_text(assets, sex_label(sex), 10.5, theme::TEXT),
            ChildOf(button),
        ));
        commands
            .entity(button)
            .observe(move |_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| form.0.sex = sex);
    }
}

fn spawn_style_stepper(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    initial: u16,
) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(48),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(parent),
        ))
        .id();
    let prev = step_button(commands, assets, row, "chevron-left");
    let value = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        title_text(assets, &format!("Style {initial}"), 15.0, theme::TEXT),
        FormValue::HairStyle,
        ChildOf(value),
    ));
    commands.spawn((
        mono_text(assets, &format!("{initial}/25"), 8.5, theme::TEXT_FAINT),
        FormValue::HairStyleCount,
        ChildOf(value),
    ));
    let next = step_button(commands, assets, row, "chevron-right");
    commands
        .entity(prev)
        .observe(|_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
            form.0.hair_style = cycle(form.0.hair_style, -1, HAIR_STYLE_MIN, HAIR_STYLE_MAX);
        });
    commands
        .entity(next)
        .observe(|_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
            form.0.hair_style = cycle(form.0.hair_style, 1, HAIR_STYLE_MIN, HAIR_STYLE_MAX);
        });
}

fn spawn_swatches(commands: &mut Commands, parent: Entity, selected: u16) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    for (color, hue) in HAIR_SWATCHES.iter().copied().enumerate() {
        let swatch = commands
            .spawn((
                Pickable::default(),
                HairSwatch(color as u16),
                Node {
                    width: px(30),
                    height: px(30),
                    border: UiRect::all(px(if color as u16 == selected { 3 } else { 1 })),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(hue),
                BorderColor::all(if color as u16 == selected {
                    theme::GOLD
                } else {
                    theme::STROKE
                }),
                ChildOf(row),
            ))
            .id();
        commands.entity(swatch).observe(
            move |_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
                form.0.hair_color = color as u16;
            },
        );
    }
}

fn action_button(commands: &mut Commands, parent: Entity, primary: bool) -> Entity {
    commands
        .spawn((
            Pickable::default(),
            Node {
                height: px(46),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(if primary { 0 } else { 1 })),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            if primary {
                primary_gradient(false)
            } else {
                BackgroundGradient::default()
            },
            BackgroundColor(if primary {
                Color::NONE
            } else {
                Color::WHITE.with_alpha(0.03)
            }),
            BorderColor::all(theme::STROKE),
            ChildOf(parent),
        ))
        .id()
}

fn step_button(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    icon: &str,
) -> Entity {
    let button = commands
        .spawn((
            Pickable::default(),
            Node {
                width: px(46),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(assets, icon, 14.0, theme::TEXT_DIM),
        ChildOf(button),
    ));
    button
}

fn field_label(assets: &AssetServer, text: &str) -> impl Bundle + use<> {
    (
        mono_text(assets, text, 9.0, theme::TEXT_FAINT),
        Node {
            margin: UiRect::bottom(px(8)),
            ..default()
        },
    )
}

fn mono_text(assets: &AssetServer, text: &str, size: f32, color: Color) -> impl Bundle + use<> {
    theme::label(text.to_string(), assets.load(theme::FONT_MONO), size, color)
}

fn title_text(assets: &AssetServer, text: &str, size: f32, color: Color) -> impl Bundle + use<> {
    theme::label(
        text.to_string(),
        assets.load(theme::FONT_TITLE),
        size,
        color,
    )
}

fn segment_gradient(selected: bool) -> BackgroundGradient {
    if !selected {
        return BackgroundGradient::default();
    }
    BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(theme::EMERALD_DEEP, percent(0)),
        ColorStop::new(theme::EMERALD_BRI, percent(100)),
    ]))
}

fn primary_gradient(enabled: bool) -> BackgroundGradient {
    let alpha = if enabled { 1.0 } else { 0.28 };
    BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(theme::EMERALD_DEEP.with_alpha(alpha), percent(0)),
        ColorStop::new(theme::EMERALD_BRI.with_alpha(alpha), percent(100)),
    ]))
}

fn teardown_preview(
    mut commands: Commands,
    mut preview: ResMut<CreatePreview>,
    characters: Query<Entity, With<CreatePreviewCharacter>>,
    cameras: Query<Entity, With<PreviewCamera>>,
) {
    for entity in characters.iter().chain(&cameras) {
        commands.entity(entity).despawn();
    }
    *preview = CreatePreview::default();
}

fn rebuild_preview_character(
    mut commands: Commands,
    form: Res<CreationForm>,
    preview: Res<CreatePreview>,
    mut sprite_events: MessageWriter<SpawnCharacterSpriteEvent>,
    existing: Query<Entity, With<CreatePreviewCharacter>>,
) {
    if preview.target.is_none() || !form.is_changed() {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let (data, appearance) = preview_components(&form.0);
    let entity = commands
        .spawn((
            data,
            appearance,
            CharacterSprite::default(),
            CharacterDirection::default(),
            Transform::default(),
            Visibility::default(),
            CreatePreviewCharacter,
            Name::new("CreatePreviewCharacter"),
        ))
        .id();
    sprite_events.write(SpawnCharacterSpriteEvent {
        character_entity: entity,
        spawn_position: Vec3::ZERO,
    });
}

fn preview_components(form: &CharacterCreationForm) -> (CharacterData, CharacterAppearance) {
    (
        CharacterData {
            name: String::new(),
            job_id: form.starting_job,
            level: 1,
            experience: 0,
            stats: CharacterStats::default(),
            slot: 0,
        },
        CharacterAppearance {
            gender: form.sex,
            hair_style: form.hair_style,
            hair_color: form.hair_color,
            clothes_color: 0,
        },
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reflect_form_values(
    form: Res<CreationForm>,
    mut server_error: ResMut<NameServerError>,
    names: Query<Ref<EditableText>, With<NameField>>,
    mut values: Query<(&mut Text, &FormValue), (Without<NameTrailing>, Without<NameStatus>)>,
    mut swatches: Query<(&HairSwatch, &mut BorderColor, &mut Node)>,
    mut sex_options: Query<(&SexOption, &mut BackgroundGradient), Without<CreateButton>>,
    mut trailing: Query<(&mut Text, &mut TextColor), (With<NameTrailing>, Without<NameStatus>)>,
    mut statuses: Query<(&mut Text, &mut TextColor), (With<NameStatus>, Without<NameTrailing>)>,
    mut create_buttons: Query<(&mut Pickable, &mut BackgroundGradient), With<CreateButton>>,
) {
    for (mut text, value) in &mut values {
        **text = match value {
            FormValue::HairStyle => format!("Style {}", form.0.hair_style),
            FormValue::HairStyleCount => format!("{}/25", form.0.hair_style),
        };
    }
    for (swatch, mut border, mut node) in &mut swatches {
        let selected = swatch.0 == form.0.hair_color;
        *border = BorderColor::all(if selected { theme::GOLD } else { theme::STROKE });
        node.border = UiRect::all(px(if selected { 3 } else { 1 }));
    }
    for (option, mut gradient) in &mut sex_options {
        *gradient = segment_gradient(option.0 == form.0.sex);
    }

    let Ok(name) = names.single() else { return };
    if name.is_changed() {
        server_error.0 = None;
    }
    let value = name.value().to_string();
    let state = name_state(&value, server_error.0.as_deref());
    let (trailing_text, status_text, color) = match state {
        NameState::Idle => (
            format!("{}/16", value.chars().count()),
            String::new(),
            theme::TEXT_FAINT,
        ),
        NameState::TooShort => (
            "!".into(),
            format!("At least {NAME_MIN} letters."),
            theme::WARN,
        ),
        NameState::Invalid => (
            "!".into(),
            "Letters, numbers, and underscores only.".into(),
            theme::WARN,
        ),
        NameState::Ok => (
            "OK".into(),
            "This name is yours to claim.".into(),
            theme::EMERALD,
        ),
        NameState::ServerError => (
            "!".into(),
            server_error.0.clone().unwrap_or_default(),
            theme::BAD,
        ),
    };
    for (mut text, mut text_color) in &mut trailing {
        **text = trailing_text.clone();
        text_color.0 = color;
    }
    for (mut text, mut text_color) in &mut statuses {
        **text = status_text.clone();
        text_color.0 = color;
    }
    let enabled = state == NameState::Ok;
    for (mut pickable, mut gradient) in &mut create_buttons {
        *pickable = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
        *gradient = primary_gradient(enabled);
    }
}

fn surface_creation_failure(
    mut failures: MessageReader<CharacterCreationFailedEvent>,
    mut server_error: ResMut<NameServerError>,
) {
    if let Some(failure) = failures.read().last() {
        server_error.0 = Some(failure.error.clone());
    }
}

fn return_to_character_select(
    mut events: MessageReader<CharacterCreatedEvent>,
    mut next: ResMut<NextState<GameState>>,
) {
    if events.read().next().is_some() {
        next.set(GameState::CharacterSelection);
    }
}

fn return_on_click(mut click: On<Pointer<Click>>, mut next: ResMut<NextState<GameState>>) {
    click.propagate(false);
    next.set(GameState::CharacterSelection);
}

fn create_character(
    _click: On<Pointer<Click>>,
    form: Res<CreationForm>,
    slot: Res<CreationSlot>,
    names: Query<&EditableText, With<NameField>>,
    mut writer: MessageWriter<CreateCharacterRequestEvent>,
) {
    let Ok(name) = names.single() else { return };
    let submitted = submitted_form(&form.0, name.value().to_string(), slot.0);
    if submitted.validate().is_ok() {
        writer.write(CreateCharacterRequestEvent { form: submitted });
    }
}

fn randomize_form(_click: On<Pointer<Click>>, mut form: ResMut<CreationForm>) {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let (sex, hair_style, hair_color) = randomized_appearance(seed);
    form.0.sex = sex;
    form.0.hair_style = hair_style;
    form.0.hair_color = hair_color;
}

fn randomized_appearance(seed: u64) -> (Gender, u16, u16) {
    let sex = if seed & 1 == 0 {
        Gender::Male
    } else {
        Gender::Female
    };
    let hair_style = HAIR_STYLE_MIN + ((seed >> 1) % HAIR_STYLE_MAX as u64) as u16;
    let hair_color = HAIR_COLOR_MIN + ((seed >> 9) % (HAIR_COLOR_MAX + 1) as u64) as u16;
    (sex, hair_style, hair_color)
}

fn sex_label(sex: Gender) -> &'static str {
    match sex {
        Gender::Male => "Male",
        Gender::Female => "Female",
    }
}

fn cycle(value: u16, delta: i32, min: u16, max: u16) -> u16 {
    let span = (max - min + 1) as i32;
    let pos = (value as i32 - min as i32 + delta).rem_euclid(span);
    (min as i32 + pos) as u16
}

fn submitted_form(base: &CharacterCreationForm, name: String, slot: u8) -> CharacterCreationForm {
    CharacterCreationForm {
        name,
        slot,
        ..base.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_state_transitions_and_edit_clear_server_error() {
        assert_eq!(name_state("", None), NameState::Idle);
        assert_eq!(name_state("abc", None), NameState::TooShort);
        assert_eq!(name_state("Valkyrie", None), NameState::Ok);
        assert_eq!(name_state("bad name", None), NameState::Invalid);

        let mut server_error = Some("Name already exists".to_string());
        assert_eq!(
            name_state("Valkyrie", server_error.as_deref()),
            NameState::ServerError
        );
        server_error = None;
        assert_eq!(
            name_state("Valkyrie", server_error.as_deref()),
            NameState::Ok
        );
    }

    #[test]
    fn cycle_wraps_both_directions() {
        assert_eq!(
            cycle(HAIR_STYLE_MIN, -1, HAIR_STYLE_MIN, HAIR_STYLE_MAX),
            HAIR_STYLE_MAX
        );
        assert_eq!(
            cycle(HAIR_STYLE_MAX, 1, HAIR_STYLE_MIN, HAIR_STYLE_MAX),
            HAIR_STYLE_MIN
        );
    }

    #[test]
    fn submitted_form_keeps_appearance_and_sets_name_slot() {
        let base = CharacterCreationForm {
            hair_style: 7,
            hair_color: 3,
            sex: Gender::Female,
            ..default()
        };
        let form = submitted_form(&base, "Valkyrie".into(), 2);
        assert_eq!((form.name.as_str(), form.slot), ("Valkyrie", 2));
        assert_eq!(
            (form.hair_style, form.hair_color, form.sex),
            (7, 3, Gender::Female)
        );
        assert!(form.validate().is_ok());
    }

    #[test]
    fn preview_appearance_carries_hair_color() {
        let form = CharacterCreationForm {
            hair_color: 6,
            ..default()
        };
        let (_, appearance) = preview_components(&form);
        assert_eq!(appearance.hair_color, 6);
    }

    #[test]
    fn randomized_appearance_stays_in_valid_ranges() {
        for seed in [0, 1, 25, 512, u64::MAX] {
            let (_, style, color) = randomized_appearance(seed);
            assert!((HAIR_STYLE_MIN..=HAIR_STYLE_MAX).contains(&style));
            assert!((HAIR_COLOR_MIN..=HAIR_COLOR_MAX).contains(&color));
        }
    }

    #[test]
    fn reflect_form_values_has_disjoint_queries() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<CreationForm>();
        app.init_resource::<NameServerError>();
        app.add_systems(Update, reflect_form_values);
        app.update();
    }

    #[test]
    fn character_created_returns_to_selection() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        app.init_state::<GameState>();
        app.add_message::<CharacterCreatedEvent>();
        app.add_systems(
            Update,
            return_to_character_select.run_if(in_state(GameState::CharacterCreation)),
        );
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterCreation);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<CharacterCreatedEvent>>()
            .write(CharacterCreatedEvent);
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::CharacterSelection
        );
    }
}
