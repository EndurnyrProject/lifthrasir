//! Character creation screen.
//!
//! Reached from an empty slot's "Create" button on the selection screen (which
//! stashes the chosen [`CreationSlot`] and switches to [`GameState::CharacterCreation`]).
//! A raw `bevy_ui` form: a name field (`EditableText`), prev/next cyclers for
//! hair style and color, a sex toggle, and Create/Cancel buttons. The cyclers/toggle
//! drive a [`CreationForm`] resource through pointer observers; a single live SPR/ACT
//! preview rebuilds from that form through the in-world billboard path (mirroring
//! [`character_preview`], but for the character being created).
//!
//! Engine ownership: Create writes `CreateCharacterRequestEvent`; on
//! `CharacterCreatedEvent` the engine refreshes the list and the UI returns to
//! `CharacterSelection`; `CharacterCreationFailedEvent` surfaces as crimson text.

use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy::text::EditableText;
use game_engine::core::state::GameState;
use game_engine::domain::character::events::{
    CharacterCreatedEvent, CharacterCreationFailedEvent, CreateCharacterRequestEvent,
};
use game_engine::domain::character::forms::CharacterCreationForm;
use game_engine::domain::entities::character::components::visual::{
    CharacterDirection, CharacterSprite,
};
use game_engine::domain::entities::character::components::{
    CharacterAppearance, CharacterData, CharacterStats, Gender,
};
use game_engine::domain::entities::character::events::forward_character_sprite_events;
use game_engine::domain::entities::character::SpawnCharacterSpriteEvent;

use crate::screens::character_preview::{create_render_target, COLUMN_PX, ROW_PX};
use crate::theme::{self, label};
use crate::widgets::placeholder::Placeholder;

const NAME_MAX: usize = 16;

// NOTE: hardcoded client hair ranges — the old `GetHairstylesRequestedEvent`
// source no longer exists in the engine. Widen here if a data-driven range appears.
const HAIR_STYLE_MIN: u16 = 1;
const HAIR_STYLE_MAX: u16 = 25;
const HAIR_COLOR_MIN: u16 = 0;
const HAIR_COLOR_MAX: u16 = 8;

// Preview camera framing, mirrored from `character_preview` (single character at origin).
const PREVIEW_VIEWPORT_HEIGHT: f32 = 42.0;
const LOOK_AT_Y: f32 = -8.0;
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, -150.0, -150.0);

pub struct CharacterCreateScreenPlugin;

impl Plugin for CharacterCreateScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreationSlot>();
        app.init_resource::<CreationForm>();
        app.init_resource::<CreatePreview>();
        app.add_systems(
            OnEnter(GameState::CharacterCreation),
            show_character_create_screen,
        );
        app.add_systems(OnExit(GameState::CharacterCreation), teardown_preview);
        app.add_systems(
            Update,
            (
                reflect_form_values,
                surface_creation_failure,
                return_to_character_select,
            )
                .run_if(in_state(GameState::CharacterCreation)),
        );
        // `rebuild_preview_character` spawns the preview entity (deferred) and writes
        // its `SpawnCharacterSpriteEvent` in one frame. Ordering it AFTER the engine's
        // `forward_character_sprite_events` means that event is only read the NEXT
        // frame — after the entity's components have flushed — so the lookup succeeds
        // and the sprite actually builds (otherwise the event is consumed against a
        // not-yet-existing entity and the sprite never spawns).
        app.add_systems(
            Update,
            rebuild_preview_character
                .after(forward_character_sprite_events)
                .run_if(in_state(GameState::CharacterCreation)),
        );
    }
}

/// Slot the new character will occupy, stashed by the selection screen's Create button.
#[derive(Resource, Default)]
pub struct CreationSlot(pub u8);

/// In-progress appearance choices. `name`/`slot` are filled at submit time.
#[derive(Resource, Default)]
struct CreationForm(CharacterCreationForm);

/// The live preview's render target.
#[derive(Resource, Default)]
struct CreatePreview {
    target: Option<Handle<Image>>,
}

/// The single preview character entity (despawned/respawned on every form change).
#[derive(Component)]
struct CreatePreviewCharacter;

/// The off-screen preview camera (lives only while the screen is shown).
#[derive(Component)]
struct CreatePreviewCamera;

/// The name text input.
#[derive(Component)]
struct NameField;

/// The `<p>` that surfaces creation failures / validation errors.
#[derive(Component)]
struct CreateError;

/// Tags a value text so [`reflect_form_values`] can mirror the matching form field.
#[derive(Component, Clone, Copy)]
enum FormValue {
    HairStyle,
    HairColor,
    Sex,
}

/// The male/female glyph on the sex toggle; its image swaps with the current sex.
#[derive(Component)]
struct SexIcon;

fn show_character_create_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut form: ResMut<CreationForm>,
    mut preview: ResMut<CreatePreview>,
) {
    *form = CreationForm::default();

    let target = images.add(create_render_target(COLUMN_PX, ROW_PX));
    spawn_preview_camera(&mut commands, target.clone());

    let font_body = asset_server.load(theme::FONT_BODY);
    let font_title = asset_server.load(theme::FONT_TITLE);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            DespawnOnExit(GameState::CharacterCreation),
        ))
        .id();

    let head = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        label(
            "Endurnir",
            font_body.clone(),
            11.0,
            theme::GOLD.with_alpha(0.55),
        ),
        ChildOf(head),
    ));
    commands.spawn((
        Text::new("Create Character"),
        TextFont {
            font: font_title.into(),
            font_size: 27.0.into(),
            ..default()
        },
        TextColor(theme::DISPLAY_GOLD),
        Node {
            margin: UiRect::top(Val::Px(3.0)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(head),
    ));

    let stage = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::new(Val::Px(64.0), Val::Px(64.0), Val::Px(128.0), Val::Px(40.0)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    let preview_panel = commands
        .spawn((
            Node {
                width: Val::Px(384.0),
                padding: UiRect::all(Val::Px(22.0)),
                margin: UiRect::right(Val::Px(28.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(stage),
        ))
        .id();
    commands.spawn((
        ImageNode::new(target.clone()),
        Node {
            width: Val::Px(COLUMN_PX as f32),
            height: Val::Px(ROW_PX as f32),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(preview_panel),
    ));

    let form_panel = commands
        .spawn((
            Node {
                width: Val::Px(468.0),
                padding: UiRect::axes(Val::Px(28.0), Val::Px(26.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::STROKE),
            ChildOf(stage),
        ))
        .id();

    commands.spawn((cc_label("NAME", font_body.clone()), ChildOf(form_panel)));
    let name_box = commands
        .spawn((
            Node {
                height: Val::Px(50.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(15.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(form_panel),
        ))
        .id();
    let name_field = commands
        .spawn((
            EditableText {
                max_characters: Some(NAME_MAX),
                ..default()
            },
            TextFont {
                font: font_body.clone().into(),
                font_size: 15.0.into(),
                ..default()
            },
            TextColor(theme::TEXT),
            NameField,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                ..default()
            },
            ChildOf(name_box),
        ))
        .id();
    commands.spawn((
        Text::new("Name your hero"),
        TextFont {
            font: font_body.clone().into(),
            font_size: 15.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_FAINT),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Pickable::IGNORE,
        Placeholder(name_field),
        ChildOf(name_field),
    ));

    spawn_segmented_sex(
        &mut commands,
        &asset_server,
        form_panel,
        form.0.sex,
        font_body.clone(),
    );
    spawn_stepper(
        &mut commands,
        &asset_server,
        form_panel,
        "HAIR STYLE",
        FormValue::HairStyle,
        form.0.hair_style,
        font_body.clone(),
    );
    spawn_stepper(
        &mut commands,
        &asset_server,
        form_panel,
        "HAIR COLOR",
        FormValue::HairColor,
        form.0.hair_color,
        font_body.clone(),
    );

    commands.spawn((
        Text::new(""),
        TextFont {
            font: font_body.clone().into(),
            font_size: 13.0.into(),
            ..default()
        },
        TextColor(theme::BAD),
        Node {
            min_height: Val::Px(18.0),
            margin: UiRect::top(Val::Px(16.0)),
            ..default()
        },
        CreateError,
        Pickable::IGNORE,
        ChildOf(form_panel),
    ));

    let actions = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                margin: UiRect::top(Val::Px(22.0)),
                ..default()
            },
            ChildOf(form_panel),
        ))
        .id();
    let cancel = commands
        .spawn((
            Pickable::default(),
            Node {
                height: Val::Px(46.0),
                flex_basis: Val::Percent(38.0),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                column_gap: Val::Px(8.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
            BorderColor::all(theme::STROKE),
            ChildOf(actions),
        ))
        .id();
    commands.spawn((
        theme::icon(&asset_server, "back", 16.0, theme::TEXT_DIM),
        ChildOf(cancel),
    ));
    commands.spawn((
        label("Cancel", font_body.clone(), 14.0, theme::TEXT_DIM),
        ChildOf(cancel),
    ));
    commands.entity(cancel).observe(
        |_: On<Pointer<Click>>, mut next: ResMut<NextState<GameState>>| {
            next.set(GameState::CharacterSelection);
        },
    );

    let create = commands
        .spawn((
            Pickable::default(),
            Node {
                height: Val::Px(50.0),
                flex_grow: 1.0,
                column_gap: Val::Px(8.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            ChildOf(actions),
        ))
        .id();
    commands.spawn((
        theme::icon(&asset_server, "check", 18.0, theme::EMERALD_INK),
        ChildOf(create),
    ));
    commands.spawn((
        label("Create Hero", font_body, 15.0, theme::EMERALD_INK),
        ChildOf(create),
    ));
    commands.entity(create).observe(create_character);

    *preview = CreatePreview {
        target: Some(target),
    };
}

fn cc_label(text: &str, font: Handle<Font>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.into(),
            font_size: 11.0.into(),
            ..default()
        },
        TextColor(theme::TEXT_DIM),
        Node {
            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(18.0), Val::Px(8.0)),
            ..default()
        },
        Pickable::IGNORE,
    )
}

/// The emerald "Sex" toggle: one button whose label cycles Male/Female on click.
fn spawn_segmented_sex(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    sex: Gender,
    font: Handle<Font>,
) {
    commands.spawn((cc_label("SEX", font.clone()), ChildOf(parent)));
    let button = commands
        .spawn((
            Pickable::default(),
            Node {
                height: Val::Px(50.0),
                column_gap: Val::Px(9.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, sex_icon(sex), 18.0, theme::EMERALD_INK),
        SexIcon,
        ChildOf(button),
    ));
    commands.spawn((
        Text::new(sex_label(sex)),
        TextFont {
            font: font.into(),
            font_size: 15.0.into(),
            ..default()
        },
        TextColor(theme::EMERALD_INK),
        FormValue::Sex,
        Pickable::IGNORE,
        ChildOf(button),
    ));
    commands
        .entity(button)
        .observe(|_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
            form.0.sex = match form.0.sex {
                Gender::Male => Gender::Female,
                Gender::Female => Gender::Male,
            };
        });
}

/// A `‹ value ›` stepper bound to one [`FormValue`] field; prev/next cycle it.
fn spawn_stepper(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    label_text: &str,
    kind: FormValue,
    initial: u16,
    font: Handle<Font>,
) {
    commands.spawn((cc_label(label_text, font.clone()), ChildOf(parent)));
    let stepper = commands
        .spawn((
            Node {
                height: Val::Px(50.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(parent),
        ))
        .id();

    let prev = spawn_step_button(commands, asset_server, stepper, "chevron-left");
    let cell = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(stepper),
        ))
        .id();
    commands.spawn((
        Text::new(initial.to_string()),
        TextFont {
            font: font.clone().into(),
            font_size: 16.0.into(),
            ..default()
        },
        TextColor(theme::TEXT),
        kind,
        Pickable::IGNORE,
        ChildOf(cell),
    ));
    let next = spawn_step_button(commands, asset_server, stepper, "chevron-right");

    commands.entity(prev).observe(
        move |_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
            apply_cycle(&mut form.0, kind, -1);
        },
    );
    commands.entity(next).observe(
        move |_: On<Pointer<Click>>, mut form: ResMut<CreationForm>| {
            apply_cycle(&mut form.0, kind, 1);
        },
    );
}

fn spawn_step_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    icon: &str,
) -> Entity {
    let button = commands
        .spawn((
            Pickable::default(),
            Node {
                width: Val::Px(52.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.025)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, icon, 15.0, theme::TEXT_DIM),
        ChildOf(button),
    ));
    button
}

fn spawn_preview_camera(commands: &mut Commands, target: Handle<Image>) {
    let look_at = Vec3::new(0.0, LOOK_AT_Y, 0.0);
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            order: -1,
            ..default()
        },
        RenderTarget::Image(target.into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: PREVIEW_VIEWPORT_HEIGHT,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(look_at + CAMERA_OFFSET).looking_at(look_at, Vec3::NEG_Y),
        CreatePreviewCamera,
        Name::new("CharacterCreatePreviewCamera"),
    ));
}

/// Despawns the off-screen preview entity/camera on exit. The UI tree is cleaned up
/// by its `DespawnOnExit`.
fn teardown_preview(
    mut commands: Commands,
    mut preview: ResMut<CreatePreview>,
    characters: Query<Entity, With<CreatePreviewCharacter>>,
    cameras: Query<Entity, With<CreatePreviewCamera>>,
) {
    for entity in characters.iter().chain(&cameras) {
        commands.entity(entity).despawn();
    }
    *preview = CreatePreview::default();
}

/// Rebuilds the preview character from the current form whenever it changes (and on
/// the initial frame, since the form is reset on enter).
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

/// The two ECS components the sprite pipeline reads to build a character billboard.
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

/// Mirrors the form's current values into the stepper/segment value texts and swaps
/// the sex toggle's glyph to match the chosen gender.
fn reflect_form_values(
    form: Res<CreationForm>,
    asset_server: Res<AssetServer>,
    mut values: Query<(&mut Text, &FormValue)>,
    mut sex_icons: Query<&mut ImageNode, With<SexIcon>>,
) {
    if !form.is_changed() {
        return;
    }
    for (mut text, kind) in &mut values {
        let value = match kind {
            FormValue::HairStyle => form.0.hair_style.to_string(),
            FormValue::HairColor => form.0.hair_color.to_string(),
            FormValue::Sex => sex_label(form.0.sex).to_string(),
        };
        *text = Text::new(value);
    }
    let icon = format!("{}{}.svg", theme::ICON_DIR, sex_icon(form.0.sex));
    for mut node in &mut sex_icons {
        node.image = asset_server.load(&icon);
    }
}

fn surface_creation_failure(
    mut failures: MessageReader<CharacterCreationFailedEvent>,
    mut errors: Query<&mut Text, With<CreateError>>,
) {
    let Some(failure) = failures.read().last() else {
        return;
    };
    set_error(&mut errors, &failure.error);
}

fn return_to_character_select(
    mut events: MessageReader<CharacterCreatedEvent>,
    mut next: ResMut<NextState<GameState>>,
) {
    if events.read().next().is_some() {
        next.set(GameState::CharacterSelection);
    }
}

fn create_character(
    _click: On<Pointer<Click>>,
    form: Res<CreationForm>,
    slot: Res<CreationSlot>,
    names: Query<&EditableText, With<NameField>>,
    mut writer: MessageWriter<CreateCharacterRequestEvent>,
    mut errors: Query<&mut Text, With<CreateError>>,
) {
    let Ok(name) = names.single() else {
        return;
    };
    let submitted = submitted_form(&form.0, name.value().to_string(), slot.0);
    match submitted.validate() {
        Ok(()) => {
            set_error(&mut errors, "");
            writer.write(CreateCharacterRequestEvent { form: submitted });
        }
        Err(error) => set_error(&mut errors, &error.to_string()),
    }
}

fn set_error(errors: &mut Query<&mut Text, With<CreateError>>, text: &str) {
    for mut error in errors.iter_mut() {
        *error = Text::new(text);
    }
}

fn sex_label(sex: Gender) -> &'static str {
    match sex {
        Gender::Male => "Male",
        Gender::Female => "Female",
    }
}

fn sex_icon(sex: Gender) -> &'static str {
    match sex {
        Gender::Male => "male",
        Gender::Female => "female",
    }
}

/// Wraps `value` by `delta` within the inclusive `[min, max]` range.
fn cycle(value: u16, delta: i32, min: u16, max: u16) -> u16 {
    let span = (max - min + 1) as i32;
    let pos = (value as i32 - min as i32 + delta).rem_euclid(span);
    (min as i32 + pos) as u16
}

fn apply_cycle(form: &mut CharacterCreationForm, kind: FormValue, delta: i32) {
    match kind {
        FormValue::HairStyle => {
            form.hair_style = cycle(form.hair_style, delta, HAIR_STYLE_MIN, HAIR_STYLE_MAX)
        }
        FormValue::HairColor => {
            form.hair_color = cycle(form.hair_color, delta, HAIR_COLOR_MIN, HAIR_COLOR_MAX)
        }
        FormValue::Sex => {}
    }
}

/// Builds the submitted form from the in-progress appearance, the name field, and slot.
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
    use game_engine::domain::entities::character::components::CharacterInfo as DomainCharacterInfo;

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
        assert_eq!(cycle(3, 1, 1, 25), 4);
        assert_eq!(
            cycle(HAIR_COLOR_MIN, -1, HAIR_COLOR_MIN, HAIR_COLOR_MAX),
            HAIR_COLOR_MAX
        );
    }

    #[test]
    fn apply_cycle_only_moves_the_named_field() {
        let mut form = CharacterCreationForm {
            hair_style: 3,
            hair_color: 2,
            ..default()
        };
        apply_cycle(&mut form, FormValue::HairStyle, 1);
        assert_eq!(form.hair_style, 4);
        assert_eq!(form.hair_color, 2);
        apply_cycle(&mut form, FormValue::HairColor, -1);
        assert_eq!(form.hair_color, 1);
        assert_eq!(form.hair_style, 4);
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
        assert_eq!(form.name, "Valkyrie");
        assert_eq!(form.slot, 2);
        assert_eq!(form.hair_style, 7);
        assert_eq!(form.hair_color, 3);
        assert_eq!(form.sex, Gender::Female);
        assert!(form.validate().is_ok());
    }

    fn created_character() -> DomainCharacterInfo {
        DomainCharacterInfo {
            name: "Hero".into(),
            job_id: 0,
            level: 1,
            experience: 0,
            stats: CharacterStats::default(),
            slot: 0,
            sex: Gender::Male,
            hair_style: 1,
            hair_color: 0,
            clothes_color: 0,
            char_id: 1,
            last_map: "prontera".into(),
            delete_date: None,
        }
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
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::CharacterCreation
        );

        app.world_mut()
            .resource_mut::<Messages<CharacterCreatedEvent>>()
            .write(CharacterCreatedEvent {
                character: created_character(),
                slot: 0,
            });
        app.update(); // reader sets NextState(CharacterSelection)
        app.update(); // StateTransition applies it

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::CharacterSelection
        );
    }
}
