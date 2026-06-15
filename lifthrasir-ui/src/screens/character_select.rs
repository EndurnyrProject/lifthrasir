//! Character selection screen.
//!
//! The static shell (`assets/ui/character_select.html`) is an extended_ui screen
//! with `#hero-panel` and `#character-grid` containers. The hero panel features
//! the selected character with a live diorama crop, name, job/level, and action
//! buttons. The roster grid shows compact slot cards; clicking a card updates
//! the selected slot and the hero panel rebuilds.

use bevy::prelude::*;
use bevy_extended_ui::html::HtmlSource;
use bevy_extended_ui::io::HtmlAsset;
use bevy_extended_ui::old::registry::UiRegistry;
use bevy_extended_ui::styles::CssID;
use game_engine::core::state::GameState;
use game_engine::domain::character::events::{
    CharacterInfoWithJobName, CharacterListReceivedEvent, DeleteCharacterRequestEvent,
    RequestCharacterListEvent, SelectCharacterEvent,
};

use crate::screens::character_create::CreationSlot;
use crate::screens::character_preview::{CharacterDiorama, COLUMN_PX, ROW_PX};
use crate::theme;

const CHARACTER_SELECT_UI: &str = "character_select";
const CHARACTER_SELECT_HTML: &str = "ui/character_select.html";
const GRID_CONTAINER_ID: &str = "character-grid";
const HERO_PANEL_ID: &str = "hero-panel";

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
            OnExit(GameState::CharacterSelection),
            hide_character_select_screen,
        );
        app.add_systems(
            Update,
            (receive_character_list, build_cards, rebuild_hero_panel, update_delete_labels)
                .chain()
                .run_if(in_state(GameState::CharacterSelection)),
        );
    }
}

/// Latest character list received from the engine, keyed by slot.
#[derive(Resource, Default)]
struct CharacterSelectionData {
    characters: Vec<Option<CharacterInfoWithJobName>>,
    max_slots: u8,
}

/// Guards the one-shot card spawn; reset whenever a fresh list arrives so the grid rebuilds.
#[derive(Resource, Default)]
struct CardsBuilt(bool);

/// `char_id` currently armed for deletion (first Delete click arms, second confirms).
#[derive(Resource, Default)]
struct PendingDeletion(Option<u32>);

/// Roster slot currently featured in the hero panel.
#[derive(Resource, Default)]
struct SelectedSlot(usize);

/// Marks a runtime-spawned slot card so the grid can be cleared on rebuild.
#[derive(Component)]
struct CharacterCard;

/// A card's Delete button, carrying the character it would delete.
#[derive(Component)]
struct DeleteButton {
    character_id: u32,
}

/// Marks the spawned hero-panel content for clean rebuild.
#[derive(Component)]
struct HeroContent;

#[allow(deprecated)]
fn show_character_select_screen(
    mut registry: ResMut<UiRegistry>,
    asset_server: Res<AssetServer>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut selected: ResMut<SelectedSlot>,
    mut requests: MessageWriter<RequestCharacterListEvent>,
) {
    built.0 = false;
    pending.0 = None;
    selected.0 = 0;
    let handle: Handle<HtmlAsset> = asset_server.load(CHARACTER_SELECT_HTML);
    registry.add_and_use(CHARACTER_SELECT_UI.into(), HtmlSource::from_handle(handle));
    requests.write(RequestCharacterListEvent);
}

#[allow(deprecated)]
fn hide_character_select_screen(mut registry: ResMut<UiRegistry>) {
    registry.remove(CHARACTER_SELECT_UI);
}

/// Stores the latest character list and arms a card rebuild.
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

/// Builds (or rebuilds) the compact slot cards under the grid container.
/// Waits for the diorama target when occupied slots exist (hero panel needs it).
fn build_cards(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    mut built: ResMut<CardsBuilt>,
    containers: Query<(Entity, &CssID)>,
    existing_cards: Query<Entity, With<CharacterCard>>,
) {
    if built.0 || data.characters.is_empty() {
        return;
    }

    let slots = (data.max_slots as usize).min(data.characters.len());
    let has_occupied = data.characters[..slots].iter().any(Option::is_some);
    if has_occupied && diorama.target.is_none() {
        return;
    }

    let Some((container, _)) = containers.iter().find(|(_, id)| id.0 == GRID_CONTAINER_ID) else {
        return;
    };

    for card in &existing_cards {
        commands.entity(card).despawn();
    }

    let font_bold = asset_server.load(theme::FONT_BODY_BOLD);
    let font_body = asset_server.load(theme::FONT_BODY);

    for (slot, entry) in data.characters[..slots].iter().enumerate() {
        let slot = slot as u8;
        match entry {
            Some(info) => spawn_occupied_card(
                &mut commands,
                container,
                slot,
                info,
                font_bold.clone(),
                font_body.clone(),
            ),
            None => spawn_empty_card(&mut commands, container, slot, font_body.clone()),
        }
    }

    built.0 = true;
}

fn spawn_occupied_card(
    commands: &mut Commands,
    container: Entity,
    slot: u8,
    info: &CharacterInfoWithJobName,
    font_bold: Handle<Font>,
    font_body: Handle<Font>,
) {
    let detail = format!("Lv {}", info.base.base_level);
    let glyph = info.base.name.chars().next().unwrap_or('?').to_string();

    let card = commands
        .spawn((
            CharacterCard,
            Pickable::default(),
            Node {
                width: Val::Px(214.0),
                height: Val::Px(64.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(13.0),
                padding: UiRect::all(Val::Px(12.0)),
                margin: UiRect::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::STROKE),
            ChildOf(container),
        ))
        .id();

    commands.spawn((
        label(glyph, font_bold.clone(), 19.0, theme::GOLD),
        Node {
            width: Val::Px(46.0),
            height: Val::Px(46.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(9.0)),
            ..default()
        },
        BackgroundColor(theme::GLASS),
        BorderColor::all(theme::GOLD_FAINT),
        ChildOf(card),
    ));

    let col = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() },
            ChildOf(card),
        ))
        .id();
    commands.spawn((label(info.base.name.clone(), font_bold, 15.0, theme::TEXT), ChildOf(col)));
    commands.spawn((label(detail, font_body, 11.5, theme::TEXT_FAINT), ChildOf(col)));

    let selected_slot = slot as usize;
    commands.entity(card).observe(
        move |_: On<Pointer<Click>>, mut sel: ResMut<SelectedSlot>| {
            sel.0 = selected_slot;
        },
    );
}

fn spawn_empty_card(commands: &mut Commands, container: Entity, slot: u8, font: Handle<Font>) {
    let card = commands
        .spawn((
            CharacterCard,
            Pickable::default(),
            Node {
                width: Val::Px(214.0),
                height: Val::Px(64.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                margin: UiRect::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(theme::STROKE),
            ChildOf(container),
        ))
        .id();
    commands.spawn((label("+ Create", font, 12.0, theme::TEXT_FAINT), ChildOf(card)));

    let selected_slot = slot as usize;
    commands.entity(card).observe(
        move |_: On<Pointer<Click>>, mut sel: ResMut<SelectedSlot>| {
            sel.0 = selected_slot;
        },
    );
}

/// Despawns and rebuilds the hero panel content when selection or roster changes.
fn rebuild_hero_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    selected: Res<SelectedSlot>,
    built: Res<CardsBuilt>,
    containers: Query<(Entity, &CssID)>,
    existing: Query<Entity, With<HeroContent>>,
) {
    if !built.0 {
        return;
    }
    if !selected.is_changed() && !built.is_changed() {
        return;
    }
    let Some((panel, _)) = containers.iter().find(|(_, id)| id.0 == HERO_PANEL_ID) else {
        return;
    };
    for e in &existing {
        commands.entity(e).despawn();
    }

    let font_title = asset_server.load(theme::FONT_TITLE);
    let font_body = asset_server.load(theme::FONT_BODY);

    let frame = commands
        .spawn((
            HeroContent,
            Node {
                width: Val::Px(392.0),
                height: Val::Px(440.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(26.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(panel),
        ))
        .id();

    match featured(&data.characters, selected.0) {
        Some(info) => {
            let slot = selected.0 as u8;
            let mut preview = ImageNode::default();
            if let Some(target) = &diorama.target {
                preview.image = target.clone();
                preview.rect = diorama.columns.get(&slot).copied();
            }
            commands.spawn((
                preview,
                Node {
                    width: Val::Px(COLUMN_PX as f32),
                    height: Val::Px(ROW_PX as f32),
                    ..default()
                },
                ChildOf(frame),
            ));
            commands.spawn((
                label(info.base.name.clone(), font_title, 25.0, theme::DISPLAY_GOLD),
                ChildOf(frame),
            ));
            commands.spawn((
                label(
                    format!("{}   Lv. {}", info.job_name, info.base.base_level),
                    font_body.clone(),
                    13.0,
                    theme::TEXT_DIM,
                ),
                ChildOf(frame),
            ));
            let actions = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                    ChildOf(frame),
                ))
                .id();
            spawn_enter_button(&mut commands, actions, slot, font_body.clone());
            spawn_delete_button(&mut commands, actions, info.base.char_id, font_body);
        }
        None => {
            commands.spawn((
                label("Empty Slot", font_title, 20.0, theme::DISPLAY_GOLD),
                ChildOf(frame),
            ));
            commands.spawn((
                label("Forge a new hero.", font_body.clone(), 13.0, theme::TEXT_FAINT),
                ChildOf(frame),
            ));
            spawn_create_button(&mut commands, frame, selected.0 as u8, font_body);
        }
    }
}

fn spawn_enter_button(commands: &mut Commands, parent: Entity, slot: u8, font: Handle<Font>) {
    let btn = commands
        .spawn((
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((label("Enter Game", font, 15.0, theme::EMERALD_INK), ChildOf(btn)));
    commands.entity(btn).observe(
        move |mut click: On<Pointer<Click>>, mut writer: MessageWriter<SelectCharacterEvent>| {
            click.propagate(false);
            writer.write(SelectCharacterEvent { slot });
        },
    );
}

fn spawn_create_button(commands: &mut Commands, parent: Entity, slot: u8, font: Handle<Font>) {
    let btn = commands
        .spawn((
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((label("Create Character", font, 15.0, theme::EMERALD_INK), ChildOf(btn)));
    commands.entity(btn).observe(
        move |mut click: On<Pointer<Click>>,
              mut commands: Commands,
              mut next: ResMut<NextState<GameState>>| {
            click.propagate(false);
            commands.insert_resource(CreationSlot(slot));
            next.set(GameState::CharacterCreation);
        },
    );
}

/// Delete button: first click arms, second click within the armed state confirms.
/// Label flips Delete<->Confirm? via `update_delete_labels`.
fn spawn_delete_button(commands: &mut Commands, parent: Entity, character_id: u32, font: Handle<Font>) {
    let btn = commands
        .spawn((
            DeleteButton { character_id },
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.878, 0.384, 0.369, 0.12)),
            BorderColor::all(theme::BAD),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((label("Delete", font, 14.0, theme::BAD), ChildOf(btn)));
    commands.entity(btn).observe(
        move |mut click: On<Pointer<Click>>,
              mut pending: ResMut<PendingDeletion>,
              mut writer: MessageWriter<DeleteCharacterRequestEvent>| {
            click.propagate(false);
            if pending.0 == Some(character_id) {
                writer.write(DeleteCharacterRequestEvent { character_id });
                pending.0 = None;
            } else {
                pending.0 = Some(character_id);
            }
        },
    );
}

/// Reflects the armed-for-deletion state in the Delete button labels.
fn update_delete_labels(
    pending: Res<PendingDeletion>,
    buttons: Query<(&DeleteButton, &Children)>,
    mut texts: Query<&mut Text>,
) {
    if !pending.is_changed() {
        return;
    }
    for (button, children) in &buttons {
        let text = if pending.0 == Some(button.character_id) {
            "Confirm?"
        } else {
            "Delete"
        };
        for child in children.iter() {
            if let Ok(mut t) = texts.get_mut(child) {
                *t = Text::new(text);
            }
        }
    }
}

fn label(text: impl Into<String>, font: Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font,
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

/// The character to feature in the hero panel for the selected slot, or `None`
/// if the slot is empty or out of range.
fn featured(
    characters: &[Option<CharacterInfoWithJobName>],
    selected: usize,
) -> Option<&CharacterInfoWithJobName> {
    characters.get(selected).and_then(Option::as_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::infrastructure::networking::protocol::character::CharacterInfo as ProtocolCharacterInfo;

    #[test]
    fn featured_returns_occupied_slot() {
        let chars = vec![Some(with_job("Hero", 1, 0, 50, "Swordman")), None];
        assert!(featured(&chars, 0).is_some());
        assert_eq!(featured(&chars, 0).unwrap().base.name, "Hero");
    }

    #[test]
    fn featured_empty_slot_is_none() {
        let chars = vec![Some(with_job("Hero", 1, 0, 50, "Swordman")), None];
        assert!(featured(&chars, 1).is_none());
    }

    #[test]
    fn featured_out_of_range_is_none() {
        let chars = vec![Some(with_job("Hero", 1, 0, 50, "Swordman"))];
        assert!(featured(&chars, 9).is_none());
    }

    fn protocol_char(name: &str, char_id: u32, slot: u8, base_level: u16) -> ProtocolCharacterInfo {
        ProtocolCharacterInfo {
            char_id,
            base_exp: 0,
            zeny: 0,
            job_exp: 0,
            job_level: 1,
            body_state: 0,
            health_state: 0,
            option: 0,
            karma: 0,
            manner: 0,
            status_point: 0,
            hp: 40,
            max_hp: 40,
            sp: 11,
            max_sp: 11,
            walk_speed: 150,
            class: 0,
            hair: 1,
            body: 0,
            weapon: 0,
            base_level,
            skill_point: 0,
            head_bottom: 0,
            shield: 0,
            head_top: 0,
            head_mid: 0,
            hair_color: 0,
            clothes_color: 0,
            name: name.to_string(),
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            char_num: slot,
            rename: 0,
            last_map: "prontera".to_string(),
            delete_date: 0,
            robe: 0,
            char_slot_change: 0,
            char_rename: 0,
            sex: 1,
        }
    }

    fn with_job(
        name: &str,
        char_id: u32,
        slot: u8,
        level: u16,
        job: &str,
    ) -> CharacterInfoWithJobName {
        CharacterInfoWithJobName {
            base: protocol_char(name, char_id, slot, level),
            job_name: job.to_string(),
            body_sprite_path: "body.spr".to_string(),
            hair_sprite_path: "hair.spr".to_string(),
            hair_palette_path: None,
        }
    }

    /// Builds an app with just enough plugins to spawn cards headlessly.
    fn card_app(data: CharacterSelectionData, diorama: CharacterDiorama) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<SelectCharacterEvent>();
        app.add_message::<DeleteCharacterRequestEvent>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
        app.init_resource::<SelectedSlot>();
        app.insert_resource(data);
        app.insert_resource(diorama);
        app.world_mut().spawn(CssID(GRID_CONTAINER_ID.to_string()));
        app.add_systems(Update, build_cards);
        app
    }

    fn occupied_diorama() -> CharacterDiorama {
        let mut diorama = CharacterDiorama {
            target: Some(Handle::default()),
            ..default()
        };
        diorama.columns.insert(0, Rect::new(0.0, 0.0, 144.0, 224.0));
        diorama.columns.insert(2, Rect::new(144.0, 0.0, 288.0, 224.0));
        diorama
    }

    fn card_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<CharacterCard>>()
            .iter(app.world())
            .count()
    }

    fn all_texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect()
    }

    #[test]
    fn builds_one_card_per_slot_with_names() {
        let data = CharacterSelectionData {
            characters: vec![
                Some(with_job("Hero", 1, 0, 50, "Swordman")),
                None,
                Some(with_job("Mage", 2, 2, 33, "Magician")),
            ],
            max_slots: 3,
        };
        let mut app = card_app(data, occupied_diorama());

        app.update();

        assert_eq!(card_count(&mut app), 3, "one card per slot up to max_slots");
        let texts = all_texts(&mut app);
        assert!(texts.iter().any(|t| t == "Hero"));
        assert!(texts.iter().any(|t| t == "Mage"));
        assert!(texts.iter().any(|t| t == "Lv 50"));
        assert!(texts.iter().any(|t| t == "+ Create"), "empty slot shows create");
        assert!(app.world().resource::<CardsBuilt>().0);
    }

    #[test]
    fn is_idempotent_across_frames() {
        let data = CharacterSelectionData {
            characters: vec![Some(with_job("Hero", 1, 0, 50, "Swordman")), None],
            max_slots: 2,
        };
        let mut app = card_app(data, occupied_diorama());

        app.update();
        app.update();
        app.update();

        assert_eq!(card_count(&mut app), 2);
    }

    #[test]
    fn waits_for_diorama_before_building_occupied_cards() {
        let data = CharacterSelectionData {
            characters: vec![Some(with_job("Hero", 1, 0, 50, "Swordman"))],
            max_slots: 1,
        };
        let mut app = card_app(data, CharacterDiorama::default());

        app.update();

        assert_eq!(card_count(&mut app), 0);
        assert!(!app.world().resource::<CardsBuilt>().0);
    }
}
