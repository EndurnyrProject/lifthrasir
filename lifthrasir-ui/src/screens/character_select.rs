//! Character selection screen.
//!
//! The static shell (`assets/ui/character_select.html`) is an extended_ui screen
//! with an empty `#character-grid` container. The slot cards are too rich and
//! data-driven for extended_ui's templating, so they are spawned at runtime as raw
//! `bevy_ui` nodes parented under that container (one bevy_ui tree — they nest and
//! render fine), styled from [`theme`]. Each occupied card shows a live animated
//! sprite preview by cropping the shared diorama render target
//! ([`CharacterDiorama`]) to the character's column via `ImageNode.rect`.
//!
//! Engine ownership of transitions is preserved: clicking a card writes
//! `SelectCharacterEvent` and the engine drives the connect + `InGame` transition;
//! deleting writes `DeleteCharacterRequestEvent` and the engine refreshes the list.

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

use crate::screens::character_preview::{CharacterDiorama, COLUMN_PX, ROW_PX};
use crate::theme;

const CHARACTER_SELECT_UI: &str = "character_select";
/// `AssetServer` path relative to `assets/`. The `<link>` CSS hrefs inside resolve
/// relative to this file, so `theme.css` -> `ui/theme.css`.
const CHARACTER_SELECT_HTML: &str = "ui/character_select.html";
/// `id` of the `<div>` that holds the runtime-spawned slot cards.
const GRID_CONTAINER_ID: &str = "character-grid";

pub struct CharacterSelectScreenPlugin;

impl Plugin for CharacterSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelectionData>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
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
            (receive_character_list, build_cards, update_delete_labels)
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

/// Guards the one-shot card spawn; reset whenever a fresh list arrives (initial
/// load, or a refresh after deletion) so the grid rebuilds.
#[derive(Resource, Default)]
struct CardsBuilt(bool);

/// `char_id` currently armed for deletion (first Delete click arms, second
/// confirms). `None` = nothing armed.
#[derive(Resource, Default)]
struct PendingDeletion(Option<u32>);

/// Marks a runtime-spawned slot card so the grid can be cleared on rebuild.
#[derive(Component)]
struct CharacterCard;

/// A card's Delete button, carrying the character it would delete.
#[derive(Component)]
struct DeleteButton {
    character_id: u32,
}

#[allow(deprecated)]
fn show_character_select_screen(
    mut registry: ResMut<UiRegistry>,
    asset_server: Res<AssetServer>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut requests: MessageWriter<RequestCharacterListEvent>,
) {
    built.0 = false;
    pending.0 = None;
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

/// Builds (or rebuilds) the slot cards under the grid container once the list and
/// — for occupied slots — the diorama render target are both ready.
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
                &diorama,
                font_bold.clone(),
                font_body.clone(),
            ),
            None => spawn_empty_card(&mut commands, container, slot, font_body.clone()),
        }
    }

    built.0 = true;
}

/// Shared card frame: dark slate panel with a steel border, laid out as a column.
fn card_frame() -> impl Bundle {
    (
        CharacterCard,
        Pickable::default(),
        Node {
            width: Val::Px(COLUMN_PX as f32 + 24.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(8.0)),
            margin: UiRect::all(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(theme::SLATE_GRAY),
        BorderColor::all(theme::POLISHED_STEEL),
    )
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

/// Occupied slot: animated preview (cropped from the shared target), name,
/// job + level, and a two-step Delete button. The whole card selects on click.
fn spawn_occupied_card(
    commands: &mut Commands,
    container: Entity,
    slot: u8,
    info: &CharacterInfoWithJobName,
    diorama: &CharacterDiorama,
    font_bold: Handle<Font>,
    font_body: Handle<Font>,
) {
    let character_id = info.base.char_id;
    let detail = format!("{}  Lv.{}", info.job_name, info.base.base_level);

    let mut preview = ImageNode::default();
    if let Some(target) = &diorama.target {
        preview.image = target.clone();
        preview.rect = diorama.columns.get(&slot).copied();
    }

    let card = commands.spawn((card_frame(), ChildOf(container))).id();

    commands.spawn((
        preview,
        Node {
            width: Val::Px(COLUMN_PX as f32),
            height: Val::Px(ROW_PX as f32),
            ..default()
        },
        ChildOf(card),
    ));
    commands.spawn((
        label(info.base.name.clone(), font_bold, 16.0, theme::ASHEN_WHITE),
        ChildOf(card),
    ));
    commands.spawn((
        label(detail, font_body.clone(), 13.0, theme::POLISHED_STEEL),
        ChildOf(card),
    ));

    let delete = commands
        .spawn((
            DeleteButton { character_id },
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme::WORN_CRIMSON),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        label("Delete", font_body, 12.0, theme::ASHEN_WHITE),
        ChildOf(delete),
    ));

    commands.entity(delete).observe(
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

    commands.entity(card).observe(
        move |_: On<Pointer<Click>>, mut writer: MessageWriter<SelectCharacterEvent>| {
            writer.write(SelectCharacterEvent { slot });
        },
    );
}

/// Empty slot: a single Create button. Its transition lands in Task 10 (the
/// `CharacterCreation` state); for now it is a visible placeholder.
fn spawn_empty_card(commands: &mut Commands, container: Entity, slot: u8, font: Handle<Font>) {
    let card = commands.spawn((card_frame(), ChildOf(container))).id();

    commands.spawn((
        Node {
            width: Val::Px(COLUMN_PX as f32),
            height: Val::Px(ROW_PX as f32),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        ChildOf(card),
        children![label("Empty", font.clone(), 14.0, theme::POLISHED_STEEL)],
    ));

    let create = commands
        .spawn((
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme::ENERGETIC_GREEN),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        label("Create", font, 12.0, theme::FORGE_SOOT),
        ChildOf(create),
    ));

    commands
        .entity(create)
        .observe(move |mut click: On<Pointer<Click>>| {
            click.propagate(false);
            info!("Create character requested for slot {slot} (creation screen pending)");
        });
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
        let label = if pending.0 == Some(button.character_id) {
            "Confirm?"
        } else {
            "Delete"
        };
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                *text = Text::new(label);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::infrastructure::networking::protocol::character::CharacterInfo as ProtocolCharacterInfo;

    #[test]
    fn detail_line_shows_job_and_level() {
        let job = "Swordman";
        let level: u16 = 42;
        assert_eq!(format!("{job}  Lv.{level}"), "Swordman  Lv.42");
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
        diorama
            .columns
            .insert(2, Rect::new(144.0, 0.0, 288.0, 224.0));
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
        assert!(texts.iter().any(|t| t == "Swordman  Lv.50"));
        assert!(
            texts.iter().any(|t| t == "Create"),
            "empty slot shows Create"
        );
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
        // No render target yet -> occupied cards must not build.
        let mut app = card_app(data, CharacterDiorama::default());

        app.update();

        assert_eq!(card_count(&mut app), 0);
        assert!(!app.world().resource::<CardsBuilt>().0);
    }
}
