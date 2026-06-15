//! Character creation screen.
//!
//! Reached from an empty slot's "Create" button on the selection screen (which
//! stashes the chosen [`CreationSlot`] and switches to [`GameState::CharacterCreation`]).
//! The static shell (`assets/ui/character_create.html`) is an extended_ui screen:
//! a name `<input>`, prev/next cyclers for hair style and color, a sex toggle, and
//! Create/Cancel buttons. The cyclers/toggle drive a [`CreationForm`] resource via
//! `#[html_fn]` handlers; a single live SPR/ACT preview rebuilds from that form
//! through the in-world billboard path (mirroring [`character_preview`], but for the
//! character being created rather than the existing roster).
//!
//! Engine ownership: Create writes `CreateCharacterRequestEvent`; on
//! `CharacterCreatedEvent` the engine refreshes the list and the UI returns to
//! `CharacterSelection`; `CharacterCreationFailedEvent` surfaces as crimson text.

use bevy::camera::{
    ClearColorConfig, OrthographicProjection, Projection, RenderTarget, ScalingMode,
};
use bevy::prelude::*;
use bevy_extended_ui::html::{HtmlClick, HtmlSource, HtmlSubmit};
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use bevy_extended_ui::styles::CssID;
use bevy_extended_ui::widgets::Paragraph;
use bevy_extended_ui_macros::html_fn;
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

const CHARACTER_CREATE_UI: &str = "character_create";
const CHARACTER_CREATE_HTML: &str = "ui/character_create.html";
const PREVIEW_CONTAINER_ID: &str = "char-preview";

// ponytail: hardcoded client hair ranges — the old `GetHairstylesRequestedEvent`
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
        app.add_systems(
            OnExit(GameState::CharacterCreation),
            hide_character_create_screen,
        );
        app.add_systems(
            Update,
            (
                mount_preview_image,
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

/// The live preview's render target plus one-shot mount/teardown bookkeeping.
#[derive(Resource, Default)]
struct CreatePreview {
    target: Option<Handle<Image>>,
    image_mounted: bool,
}

/// The single preview character entity (despawned/respawned on every form change).
#[derive(Component)]
struct CreatePreviewCharacter;

/// The off-screen preview camera (lives only while the screen is shown).
#[derive(Component)]
struct CreatePreviewCamera;

#[allow(deprecated)]
fn show_character_create_screen(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut form: ResMut<CreationForm>,
    mut preview: ResMut<CreatePreview>,
) {
    *form = CreationForm::default();

    let handle: Handle<HtmlAsset> = asset_server.load(CHARACTER_CREATE_HTML);
    registry.add_and_use(CHARACTER_CREATE_UI.into(), HtmlSource::from_handle(handle));

    let target = images.add(create_render_target(COLUMN_PX, ROW_PX));
    spawn_preview_camera(&mut commands, target.clone());
    *preview = CreatePreview {
        target: Some(target),
        image_mounted: false,
    };
}

#[allow(deprecated)]
fn hide_character_create_screen(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    mut preview: ResMut<CreatePreview>,
    characters: Query<Entity, With<CreatePreviewCharacter>>,
    cameras: Query<Entity, With<CreatePreviewCamera>>,
) {
    registry.remove(CHARACTER_CREATE_UI);
    for entity in characters.iter().chain(&cameras) {
        commands.entity(entity).despawn();
    }
    *preview = CreatePreview::default();
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

/// Spawns the preview image node under the static container once both the render
/// target and the (async-built) container entity exist.
fn mount_preview_image(
    mut commands: Commands,
    mut preview: ResMut<CreatePreview>,
    containers: Query<(Entity, &CssID)>,
) {
    if preview.image_mounted {
        return;
    }
    let Some(target) = preview.target.clone() else {
        return;
    };
    let Some((container, _)) = containers
        .iter()
        .find(|(_, id)| id.0 == PREVIEW_CONTAINER_ID)
    else {
        return;
    };
    commands.spawn((
        ImageNode::new(target),
        Node {
            width: Val::Px(COLUMN_PX as f32),
            height: Val::Px(ROW_PX as f32),
            ..default()
        },
        ChildOf(container),
    ));
    preview.image_mounted = true;
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

/// Mirrors the form's current values into the cycler value paragraphs.
fn reflect_form_values(form: Res<CreationForm>, mut values: Query<(&mut Paragraph, &CssID)>) {
    if !form.is_changed() {
        return;
    }
    for (mut paragraph, id) in &mut values {
        let text = match id.0.as_str() {
            "hair-style-value" => form.0.hair_style.to_string(),
            "hair-color-value" => form.0.hair_color.to_string(),
            "sex-value" => sex_label(form.0.sex).to_string(),
            _ => continue,
        };
        paragraph.text = text;
    }
}

fn surface_creation_failure(
    mut failures: MessageReader<CharacterCreationFailedEvent>,
    mut errors: Query<(&mut Paragraph, &CssID)>,
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

fn sex_label(sex: Gender) -> &'static str {
    match sex {
        Gender::Male => "Male",
        Gender::Female => "Female",
    }
}

/// Wraps `value` by `delta` within the inclusive `[min, max]` range.
fn cycle(value: u16, delta: i32, min: u16, max: u16) -> u16 {
    let span = (max - min + 1) as i32;
    let pos = (value as i32 - min as i32 + delta).rem_euclid(span);
    (min as i32 + pos) as u16
}

/// Builds the submitted form from the in-progress appearance, the name field, and slot.
fn submitted_form(base: &CharacterCreationForm, name: String, slot: u8) -> CharacterCreationForm {
    CharacterCreationForm {
        name,
        slot,
        ..base.clone()
    }
}

fn set_error(errors: &mut Query<(&mut Paragraph, &CssID)>, text: &str) {
    for (mut paragraph, id) in errors.iter_mut() {
        if id.0 == "create-error" {
            paragraph.text = text.to_string();
        }
    }
}

#[html_fn("create_character")]
fn create_character(
    In(event): In<HtmlSubmit>,
    form: Res<CreationForm>,
    slot: Res<CreationSlot>,
    mut writer: MessageWriter<CreateCharacterRequestEvent>,
    mut errors: Query<(&mut Paragraph, &CssID)>,
) {
    let name = event.data.get("name").cloned().unwrap_or_default();
    let form = submitted_form(&form.0, name, slot.0);
    match form.validate() {
        Ok(()) => {
            set_error(&mut errors, "");
            writer.write(CreateCharacterRequestEvent { form });
        }
        Err(error) => set_error(&mut errors, &error.to_string()),
    }
}

#[html_fn("cancel_creation")]
fn cancel_creation(In(_event): In<HtmlClick>, mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::CharacterSelection);
}

#[html_fn("hair_style_prev")]
fn hair_style_prev(In(_event): In<HtmlClick>, mut form: ResMut<CreationForm>) {
    form.0.hair_style = cycle(form.0.hair_style, -1, HAIR_STYLE_MIN, HAIR_STYLE_MAX);
}

#[html_fn("hair_style_next")]
fn hair_style_next(In(_event): In<HtmlClick>, mut form: ResMut<CreationForm>) {
    form.0.hair_style = cycle(form.0.hair_style, 1, HAIR_STYLE_MIN, HAIR_STYLE_MAX);
}

#[html_fn("hair_color_prev")]
fn hair_color_prev(In(_event): In<HtmlClick>, mut form: ResMut<CreationForm>) {
    form.0.hair_color = cycle(form.0.hair_color, -1, HAIR_COLOR_MIN, HAIR_COLOR_MAX);
}

#[html_fn("hair_color_next")]
fn hair_color_next(In(_event): In<HtmlClick>, mut form: ResMut<CreationForm>) {
    form.0.hair_color = cycle(form.0.hair_color, 1, HAIR_COLOR_MIN, HAIR_COLOR_MAX);
}

#[html_fn("toggle_sex")]
fn toggle_sex(In(_event): In<HtmlClick>, mut form: ResMut<CreationForm>) {
    form.0.sex = match form.0.sex {
        Gender::Male => Gender::Female,
        Gender::Female => Gender::Male,
    };
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
