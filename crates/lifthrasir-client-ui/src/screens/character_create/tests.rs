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
fn server_name_error_is_displayed_in_red() {
    let feedback = name_feedback(
        NameState::ServerError,
        "ForbiddenName",
        Some("Invalid character name"),
    );
    assert_eq!(feedback.0, "!");
    assert_eq!(feedback.1, "Invalid character name");
    assert_eq!(feedback.2, lifthrasir_ui::theme::BAD);
}

#[test]
fn server_name_error_clears_only_when_the_name_changes() {
    let mut last_name = LastNameValue("ForbiddenName".into());
    let mut server_error = NameServerError(Some("Invalid character name".into()));

    let unchanged = "ForbiddenName";
    if unchanged != last_name.0 {
        server_error.0 = None;
        last_name.0.clone_from(&unchanged.to_string());
    }
    assert_eq!(server_error.0.as_deref(), Some("Invalid character name"));

    let changed = "AllowedName";
    if changed != last_name.0 {
        server_error.0 = None;
        last_name.0.clone_from(&changed.to_string());
    }
    assert!(server_error.0.is_none());
    assert_eq!(last_name.0, "AllowedName");
}

#[test]
fn focused_name_field_accepts_keyboard_input() {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::window::PrimaryWindow;

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::input_focus::InputFocusPlugin,
        bevy::input_focus::InputDispatchPlugin,
        bevy::text::TextPlugin,
        bevy::ui_widgets::TextInputPlugin,
        bevy::picking::DefaultPickingPlugins,
    ));
    app.init_resource::<bevy::ui::UiScale>();
    app.add_message::<bevy::window::Ime>();
    app.add_message::<bevy::window::WindowEvent>();
    app.add_message::<bevy::picking::events::PointerRelease>();
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let field = app
        .world_mut()
        .spawn((NameField, EditableText::default(), TextInput))
        .id();

    assert_eq!(
        app.world()
            .resource::<bevy::input_focus::InputFocus>()
            .get(),
        Some(field)
    );
    assert!(app.world().get::<Pickable>(field).is_some());
    assert!(app.world().get::<TextCursorStyle>(field).is_some());

    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyV,
        logical_key: Key::Character("V".into()),
        state: ButtonState::Pressed,
        text: Some("V".into()),
        repeat: false,
        window,
    });
    app.update();

    assert_eq!(app.world().get::<EditableText>(field).unwrap().value(), "V");
}

#[test]
fn cycle_wraps_both_directions() {
    assert_eq!(cycle(1, -1, 1, 25), 25);
    assert_eq!(cycle(25, 1, 1, 25), 1);
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
fn random_look_stays_within_the_catalog() {
    let catalog = HairCatalog::scan(CONTENT);
    for seed in [0, 1, 25, 512, 1027, u64::MAX] {
        let (sex, style, color) = random_look(seed, &catalog).unwrap();
        assert!(catalog.styles(sex).contains(&style));
        assert!(catalog.colors(style, sex).contains(&color));
    }

    assert_eq!(random_look(0, &HairCatalog::default()), None);
}

#[test]
fn reflect_form_values_has_disjoint_queries() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<CreationForm>();
    app.init_resource::<NameServerError>();
    app.init_resource::<LastNameValue>();
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

use lifthrasir_assets::{
    loaders::{RoPaletteAsset, RoSpriteAsset},
    paths,
};

/// Content paths with male styles 1, 2 and 7, female styles 7 and 40, and palettes for
/// style 7 only. The rest must be ignored: a Doram head, a head filed under the wrong
/// sex, an action file and a body palette.
const CONTENT: [&str; 13] = [
    "data/sprite/인간족/머리통/남/7_남.spr",
    "data/sprite/인간족/머리통/남/1_남.spr",
    "data/sprite/인간족/머리통/남/1_남.act",
    "data/sprite/인간족/머리통/남/2_남.spr",
    "data/sprite/인간족/머리통/여/40_여.spr",
    "data/sprite/인간족/머리통/여/7_여.spr",
    "data/sprite/인간족/머리통/남/3_여.spr",
    "data/sprite/도람족/머리통/남/9_남.spr",
    "data/palette/머리/머리7_남_3.pal",
    "data/palette/머리/머리7_남_0.pal",
    "data/palette/머리/머리7_남_1.pal",
    "data/palette/머리/머리7_여_3.pal",
    "data/palette/몸/초보자_남_1.pal",
];

#[test]
fn catalog_scan_finds_the_styles_and_colours_in_the_content() {
    let catalog = HairCatalog::scan(CONTENT);

    assert_eq!(catalog.styles(Gender::Male), [1, 2, 7]);
    assert_eq!(catalog.styles(Gender::Female), [7, 40]);
    assert_eq!(catalog.colors(7, Gender::Male), [0, 1, 3]);
    assert_eq!(catalog.colors(7, Gender::Female), [0, 3]);
    assert_eq!(catalog.colors(1, Gender::Male), [0]);
}

#[test]
fn stepping_and_picking_stay_within_the_catalog() {
    let catalog = HairCatalog::scan(CONTENT);

    assert_eq!(catalog.step_style(Gender::Male, 7, 1), 1);
    assert_eq!(catalog.step_style(Gender::Male, 1, -1), 7);
    assert_eq!(catalog.step_style(Gender::Male, 9, 1), 9);
    assert_eq!(pick(4, catalog.styles(Gender::Male)), Some(2));
    assert_eq!(pick(4, &[]), None);
}

/// An app showing the creation scene over [`CONTENT`], with the form systems running.
fn screen_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
    ));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.add_message::<CreateCharacterRequestEvent>();
    app.init_resource::<CreationSlot>();
    app.init_resource::<CreationForm>();
    app.init_resource::<NameServerError>();
    app.init_resource::<LastNameValue>();
    app.init_asset::<RoSpriteAsset>();
    app.init_asset::<RoPaletteAsset>();
    app.insert_resource(HairCatalog::scan(CONTENT));
    app.add_observer(forge_hero);
    app.world_mut().commands().spawn_scene(scene::create_screen(
        Handle::default(),
        "Valhalla · Slot 3".into(),
    ));
    app.add_systems(
        Update,
        (
            reflect_form_values,
            sync_hair_grids,
            style_choices,
            draw_hair_thumbs,
            tint_hair_swatches,
        )
            .chain(),
    );
    settle(&mut app);
    app
}

/// Runs enough frames for a form change to rebuild a grid and style its options.
fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

fn shown(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect()
}

fn choices(app: &mut App) -> Vec<Choice> {
    let mut choices = app.world_mut().query::<&Choice>();
    choices.iter(app.world()).copied().collect()
}

fn selected_choices(app: &mut App) -> Vec<Choice> {
    let form = app.world().resource::<CreationForm>().0.clone();
    let mut choices = app.world_mut().query::<(&Choice, &BorderColor)>();
    choices
        .iter(app.world())
        .filter(|(_, border)| **border == BorderColor::all(ro_dark::SELECT_RING))
        .map(|(choice, _)| *choice)
        .inspect(|choice| assert!(choice.is_selected(&form)))
        .collect()
}

#[test]
fn screen_offers_the_catalogs_options_and_marks_the_forms_choices() {
    let mut app = screen_app();

    // Two sexes, the three male styles and style 1's only colour.
    assert_eq!(choices(&mut app).len(), 2 + 3 + 1);
    assert_eq!(selected_choices(&mut app).len(), 3);
    let shown = shown(&mut app);
    for expected in [
        "Valhalla · Slot 3",
        "Your name",
        "0 / 16",
        "1 / 3",
        "Name your hero",
        "Forge Hero",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }

    Choice::HairStyle(7).apply(&mut app.world_mut().resource_mut::<CreationForm>().0);
    settle(&mut app);

    assert!(selected_choices(&mut app).contains(&Choice::HairStyle(7)));
    assert!(choices(&mut app).contains(&Choice::HairColor(3)));
    assert!(self::shown(&mut app).iter().any(|t| t == "3 / 3"));
}

#[test]
fn form_hair_falls_back_to_what_the_new_sex_or_style_offers() {
    let mut app = screen_app();
    let mut form = app.world_mut().resource_mut::<CreationForm>();
    form.0.hair_style = 2;
    form.0.hair_color = 1;
    form.0.sex = Gender::Female;
    settle(&mut app);

    let form = &app.world().resource::<CreationForm>().0;
    assert_eq!((form.hair_style, form.hair_color), (7, 0));
    let offered = choices(&mut app);
    assert!(offered.contains(&Choice::HairStyle(40)));
    assert!(!offered.contains(&Choice::HairStyle(2)));
}

#[test]
fn forging_needs_a_valid_name() {
    let mut app = screen_app();

    app.world_mut().trigger(ForgeHero);

    assert!(
        app.world()
            .resource::<Messages<CreateCharacterRequestEvent>>()
            .is_empty()
    );
}

fn hair_thumb(app: &mut App, style: u16) -> HeadArt {
    let mut thumbs = app.world_mut().query::<(&HairThumb, &HeadArt)>();
    thumbs
        .iter(app.world())
        .find(|(thumb, _)| thumb.0 == style)
        .map(|(_, art)| art.clone())
        .unwrap()
}

fn hair_swatch(app: &mut App, color: u16) -> HeadArt {
    let mut swatches = app.world_mut().query::<(&HairSwatch, &HeadArt)>();
    swatches
        .iter(app.world())
        .find(|(swatch, _)| swatch.0 == color)
        .map(|(_, art)| art.clone())
        .unwrap()
}

#[test]
fn hair_tiles_and_swatches_request_the_head_they_preview() {
    let mut app = screen_app();

    let thumb = hair_thumb(&mut app, 7);
    assert_eq!(thumb.head, Some((7, Gender::Male, 0)));
    assert_eq!(
        thumb.sprite.path().unwrap().to_string(),
        paths::head_sprite_path(Gender::Male, 7)
    );
    assert!(thumb.palette.is_none());

    let mut form = app.world_mut().resource_mut::<CreationForm>();
    form.0.hair_style = 7;
    form.0.hair_color = 3;
    settle(&mut app);

    let thumb = hair_thumb(&mut app, 2);
    assert_eq!(
        thumb.palette.unwrap().path().unwrap().to_string(),
        paths::hair_palette_path(2, Gender::Male, 3)
    );
    assert!(!thumb.drawn);
    let swatch = hair_swatch(&mut app, 1);
    assert_eq!(swatch.head, Some((7, Gender::Male, 1)));
    assert_eq!(
        swatch.palette.unwrap().path().unwrap().to_string(),
        paths::hair_palette_path(7, Gender::Male, 1)
    );
}
