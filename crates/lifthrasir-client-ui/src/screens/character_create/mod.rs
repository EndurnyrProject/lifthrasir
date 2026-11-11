//! Character creation screen ("Forge a Hero").
//!
//! One BSN scene ([`scene`]) over the shaded menu background: the live preview on a
//! pedestal and the New Hero window with the name, body, hair style and hair colour.
//! [`CreationForm`] holds the choices; every option is a [`Choice`] that writes into it
//! on click, and the preview character is respawned from it. The hair styles and
//! colours on offer come from the content itself ([`hair`]). Forging writes
//! `CreateCharacterRequestEvent`; the server's refusal lands under the name field and
//! its acceptance returns to the selection screen.

mod hair;
mod scene;
#[cfg(test)]
mod tests;

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::ecs::system::SystemParam;
use bevy::input_focus::{AutoFocus, InputFocus};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui_widgets::{Activate, TextInput};
use lifthrasir_assets::Gender;
use lifthrasir_client::{
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
            components::{CharacterAppearance, CharacterData, CharacterStats},
            events::forward_character_sprite_events,
        },
    },
};
use lifthrasir_runtime::actors::sprites::legacy::{CharacterDirection, CharacterSprite, Direction};
use lifthrasir_ui::theme::{self, ro_dark};
use net_contract::state::UserSession;

use crate::screens::{
    character_preview::{PreviewCamera, spawn_preview_diorama},
    shared::{DISABLED_TINT, glow},
};
use hair::*;

const NAME_MIN: usize = 4;
const NAME_MAX: usize = 16;

pub struct CharacterCreateScreenPlugin;

impl Plugin for CharacterCreateScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreationSlot>();
        app.init_resource::<CreationForm>();
        app.init_resource::<CreatePreview>();
        app.init_resource::<NameServerError>();
        app.init_resource::<LastNameValue>();
        app.init_resource::<HairCatalog>();
        app.add_observer(forge_hero);
        app.add_systems(
            OnEnter(GameState::CharacterCreation),
            (scan_hair_catalog, show_character_create_screen),
        );
        app.add_systems(OnExit(GameState::CharacterCreation), teardown_preview);
        app.add_systems(
            Update,
            (
                surface_creation_failure,
                reflect_form_values,
                sync_hair_grids,
                style_choices,
                draw_hair_thumbs,
                tint_hair_swatches,
                submit_on_enter,
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

#[derive(Resource, Default)]
struct LastNameValue(String);

#[derive(SystemParam)]
struct CreationScreenState<'w> {
    form: ResMut<'w, CreationForm>,
    preview: ResMut<'w, CreatePreview>,
    server_error: ResMut<'w, NameServerError>,
    last_name: ResMut<'w, LastNameValue>,
}

/// The name field with its focus, and the server's verdict on the last name sent.
#[derive(SystemParam)]
struct NameInput<'w, 's> {
    server_error: ResMut<'w, NameServerError>,
    last_name: ResMut<'w, LastNameValue>,
    focus: Option<Res<'w, InputFocus>>,
    fields: Query<'w, 's, (Entity, &'static EditableText), With<NameField>>,
}

/// The nodes that mirror the form: its labels, the name frame and the Forge Hero button.
#[derive(SystemParam)]
struct FormView<'w, 's> {
    texts: Query<'w, 's, (&'static NameText, &'static mut Text, &'static mut TextColor)>,
    frames: Query<'w, 's, &'static mut Outline, With<NameFrame>>,
    create_buttons: Query<'w, 's, &'static mut ImageNode, With<CreateButton>>,
}

#[derive(Component)]
struct CreatePreviewCharacter;

#[derive(Component, Default, Clone)]
#[require(AutoFocus, Pickable, TextCursorStyle)]
struct NameField;

/// The input frame around [`NameField`]; its outline shows focus and errors.
#[derive(Component, Default, Clone)]
struct NameFrame;

#[derive(Component, Default, Clone)]
struct CreateButton;

/// A label [`reflect_form_values`] keeps in step with the name field.
#[derive(Component, Default, Clone, Copy)]
enum NameText {
    /// The name being typed, above the preview.
    #[default]
    Tag,
    /// The character count inside the name field.
    Count,
    /// The hint or error under the name field.
    Status,
}

/// One selectable option of the form. Clicking it writes its value into
/// [`CreationForm`].
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Choice {
    Sex(Gender),
    HairStyle(u16),
    HairColor(u16),
}

impl Choice {
    fn is_selected(self, form: &CharacterCreationForm) -> bool {
        match self {
            Choice::Sex(sex) => form.sex == sex,
            Choice::HairStyle(style) => form.hair_style == style,
            Choice::HairColor(color) => form.hair_color == color,
        }
    }

    fn apply(self, form: &mut CharacterCreationForm) {
        match self {
            Choice::Sex(sex) => form.sex = sex,
            Choice::HairStyle(style) => form.hair_style = style,
            Choice::HairColor(color) => form.hair_color = color,
        }
    }
}

/// Requests creating the hero as configured (Enter or the Forge Hero button).
#[derive(Event)]
struct ForgeHero;

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
    mut images: ResMut<Assets<Image>>,
    mut creation: CreationScreenState,
    slot: Res<CreationSlot>,
    session: Option<Res<UserSession>>,
) {
    *creation.form = CreationForm::default();
    creation.server_error.0 = None;
    creation.last_name.0.clear();
    let target = spawn_preview_diorama(&mut commands, &mut images, 1, 2.0);
    creation.preview.target = Some(target.clone());
    let world = session
        .as_ref()
        .and_then(|session| session.selected_server.as_ref())
        .map_or("Endurnir", |server| server.name.as_str());
    let place = format!("{world} · Slot {}", u16::from(slot.0) + 1);

    commands
        .spawn_scene(scene::create_screen(target, place))
        .insert(DespawnOnExit(GameState::CharacterCreation));
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

/// Respawns the preview character from the form, keeping the way it was facing.
fn rebuild_preview_character(
    mut commands: Commands,
    form: Res<CreationForm>,
    preview: Res<CreatePreview>,
    mut sprite_events: MessageWriter<SpawnCharacterSpriteEvent>,
    existing: Query<(Entity, &CharacterDirection), With<CreatePreviewCharacter>>,
) {
    if preview.target.is_none() || !form.is_changed() {
        return;
    }
    let facing = existing
        .iter()
        .next()
        .map_or(Direction::South, |(_, direction)| direction.facing);
    for (entity, _) in &existing {
        commands.entity(entity).despawn();
    }
    let (data, appearance) = preview_components(&form.0);
    let entity = commands
        .spawn((
            data,
            appearance,
            CharacterSprite::default(),
            CharacterDirection { facing },
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

/// Mirrors the name field into its labels, the name frame and the Forge Hero button.
/// Editing the name clears the server's verdict on the previous one.
fn reflect_form_values(mut name: NameInput, mut view: FormView) {
    let Ok((field, text)) = name.fields.single() else {
        return;
    };
    let value = text.value().to_string();
    if value != name.last_name.0 {
        name.server_error.0 = None;
        name.last_name.0.clone_from(&value);
    }
    let server_error = name.server_error.0.as_deref();
    let state = name_state(&value, server_error);
    let (count, status, color) = name_feedback(state, &value, server_error);

    for (kind, mut text, mut text_color) in &mut view.texts {
        let (shown, tint) = match kind {
            NameText::Tag if value.is_empty() => {
                ("Your name".to_string(), ro_dark::STRONG.with_alpha(0.4))
            }
            NameText::Tag => (value.clone(), ro_dark::STRONG),
            NameText::Count => (count.clone(), color),
            NameText::Status => (status.clone(), color),
        };
        if text.0 != shown {
            text.0 = shown;
        }
        if text_color.0 != tint {
            text_color.0 = tint;
        }
    }

    let focused = name
        .focus
        .as_ref()
        .is_some_and(|focus| focus.get() == Some(field));
    let ring = match state {
        NameState::TooShort | NameState::Invalid | NameState::ServerError => color,
        NameState::Idle | NameState::Ok if focused => ro_dark::FROST_GLOW,
        NameState::Idle | NameState::Ok => Color::NONE,
    };
    for mut outline in &mut view.frames {
        if outline.color != ring {
            outline.color = ring;
        }
    }
    let art = if state == NameState::Ok {
        Color::WHITE
    } else {
        DISABLED_TINT
    };
    for mut button in &mut view.create_buttons {
        if button.color != art {
            button.color = art;
        }
    }
}

/// The character count, the line under the name field and their colour.
fn name_feedback(
    state: NameState,
    name: &str,
    server_error: Option<&str>,
) -> (String, String, Color) {
    let count = format!("{} / {NAME_MAX}", name.chars().count());
    match state {
        NameState::Idle => (
            count,
            format!("Letters, numbers and _ · {NAME_MIN} to {NAME_MAX}"),
            ro_dark::MUTED,
        ),
        NameState::TooShort => (
            "!".into(),
            format!("At least {NAME_MIN} letters"),
            theme::BAD,
        ),
        NameState::Invalid => ("!".into(), "Only letters, numbers and _".into(), theme::BAD),
        NameState::Ok => (
            count,
            "Looks good · We check it's free when you forge".into(),
            ro_dark::STATUS_ONLINE,
        ),
        NameState::ServerError => (
            "!".into(),
            server_error.unwrap_or_default().into(),
            theme::BAD,
        ),
    }
}

/// Fill and ring of an option for its selection/hover state.
fn choice_style(selected: bool, hovered: bool) -> (Color, Color) {
    match (selected, hovered) {
        (true, _) => (ro_dark::TAB_ACTIVE, ro_dark::SELECT_RING),
        (false, true) => (ro_dark::TAB_HOVER, ro_dark::MUTED_BRONZE),
        (false, false) => (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE),
    }
}

/// Recolours the options whose hover state changed, or all of them when the form did.
/// A hair colour swatch keeps its own fill.
fn style_choices(
    form: Res<CreationForm>,
    mut choices: Query<(
        &Choice,
        Ref<Hovered>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BoxShadow,
    )>,
) {
    for (choice, hovered, mut background, mut border, mut halo) in &mut choices {
        if !form.is_changed() && !hovered.is_changed() {
            continue;
        }
        let selected = choice.is_selected(&form.0);
        let (fill, ring) = choice_style(selected, hovered.get());
        if !matches!(choice, Choice::HairColor(_)) {
            *background = BackgroundColor(fill);
        }
        *border = BorderColor::all(ring);
        *halo = glow(if selected {
            ro_dark::FROST_GLOW.with_alpha(0.3)
        } else {
            Color::NONE
        });
    }
}

fn pick_choice(click: On<PointerClick>, choices: Query<&Choice>, mut form: ResMut<CreationForm>) {
    if let Ok(choice) = choices.get(click.entity) {
        choice.apply(&mut form.0);
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

fn submit_on_enter(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::Enter) {
        commands.trigger(ForgeHero);
    }
}

/// Sends the creation request once the name passes the local checks and the server has
/// not already refused it.
fn forge_hero(
    _: On<ForgeHero>,
    form: Res<CreationForm>,
    slot: Res<CreationSlot>,
    server_error: Res<NameServerError>,
    names: Query<&EditableText, With<NameField>>,
    mut writer: MessageWriter<CreateCharacterRequestEvent>,
) {
    let Ok(name) = names.single() else { return };
    let name = name.value().to_string();
    if name_state(&name, server_error.0.as_deref()) != NameState::Ok {
        return;
    }
    let submitted = submitted_form(&form.0, name, slot.0);
    if submitted.validate().is_ok() {
        writer.write(CreateCharacterRequestEvent { form: submitted });
    }
}

/// Shows the preview from behind, or from the front again.
fn turn_around(
    _: On<Activate>,
    mut previews: Query<&mut CharacterDirection, With<CreatePreviewCharacter>>,
) {
    for mut direction in &mut previews {
        direction.facing = match direction.facing {
            Direction::South => Direction::North,
            _ => Direction::South,
        };
    }
}

fn randomize_form(_: On<Activate>, mut form: ResMut<CreationForm>, catalog: Res<HairCatalog>) {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let Some((sex, hair_style, hair_color)) = random_look(seed, &catalog) else {
        return;
    };
    form.0.sex = sex;
    form.0.hair_style = hair_style;
    form.0.hair_color = hair_color;
}

/// A sex, then a hair style and a hair colour the catalog offers it, all drawn from
/// `seed`. `None` when the catalog has no style for that sex.
fn random_look(seed: u64, catalog: &HairCatalog) -> Option<(Gender, u16, u16)> {
    let sex = if seed & 1 == 0 {
        Gender::Male
    } else {
        Gender::Female
    };
    let style = pick(seed >> 1, catalog.styles(sex))?;
    let color = pick(seed >> 9, &catalog.colors(style, sex))?;
    Some((sex, style, color))
}

/// The option `seed` lands on, or `None` when there are none.
fn pick(seed: u64, options: &[u16]) -> Option<u16> {
    let index = seed.checked_rem(options.len() as u64)?;
    options.get(index as usize).copied()
}

/// Steps the hair style by `delta` through the styles on offer.
fn step_hair_style(form: &mut CharacterCreationForm, catalog: &HairCatalog, delta: i32) {
    form.hair_style = catalog.step_style(form.sex, form.hair_style, delta);
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
