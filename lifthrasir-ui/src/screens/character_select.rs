//! Character selection screen.
//!
//! A raw `bevy_ui` stage with a hero-panel and a character-grid container. The hero
//! panel features the selected character with a live diorama crop, name, job/level,
//! and action buttons. The roster grid shows compact slot cards; clicking a card
//! updates the selected slot and the hero panel rebuilds.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::character::events::{
    CharacterInfoWithJobName, CharacterListReceivedEvent, DeleteCharacterRequestEvent,
    RequestCharacterListEvent, SelectCharacterEvent,
};

use crate::screens::character_create::CreationSlot;
use crate::screens::character_preview::{CharacterDiorama, COLUMN_PX, ROW_PX};
use crate::theme::{self, label};

pub struct CharacterSelectScreenPlugin;

impl Plugin for CharacterSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelectionData>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
        app.init_resource::<SelectedSlot>();
        app.init_resource::<RosterPage>();
        app.add_systems(
            OnEnter(GameState::CharacterSelection),
            show_character_select_screen,
        );
        app.add_systems(
            Update,
            (
                receive_character_list,
                build_cards,
                rebuild_hero_panel,
                update_delete_labels,
                highlight_selected_cards,
            )
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
    /// Display pages (3 slots each), from HC_CHARLIST_NOTIFY.
    display_pages: u8,
}

/// Character-select roster page currently shown (0-based). The roster is laid
/// out in pages of 3 slots, matching the RO client when more than one page
/// exists.
#[derive(Resource, Default)]
struct RosterPage(usize);

/// Slots per roster page (RO shows 3 character slots per page).
const ROSTER_PAGE_SIZE: usize = 3;

/// Marks the page-navigation bar so it is rebuilt alongside the slot cards.
#[derive(Component)]
struct RosterNav;

/// Direction a page-nav button moves the roster page (-1 = prev, +1 = next).
#[derive(Component, Clone, Copy)]
struct PageNavStep(isize);

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

/// The roster slot a card represents, so the selected card can be highlighted.
#[derive(Component)]
struct CardSlot(u8);

/// A card's Delete button, carrying the character it would delete.
#[derive(Component)]
struct DeleteButton {
    character_id: u32,
}

/// Marks the spawned hero-panel content for clean rebuild.
#[derive(Component)]
struct HeroContent;

/// Marks the hero panel container (left column of the stage).
#[derive(Component)]
struct HeroPanel;

/// Marks the roster grid container that holds the slot cards.
#[derive(Component)]
struct CharacterGrid;

fn show_character_select_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut selected: ResMut<SelectedSlot>,
    mut roster_page: ResMut<RosterPage>,
    mut requests: MessageWriter<RequestCharacterListEvent>,
) {
    built.0 = false;
    pending.0 = None;
    selected.0 = 0;
    roster_page.0 = 0;

    let font_body = asset_server.load(theme::FONT_BODY);
    let font_title = asset_server.load(theme::FONT_TITLE);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            DespawnOnExit(GameState::CharacterSelection),
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
        label("Endurnir", font_body, 11.0, theme::GOLD.with_alpha(0.55)),
        ChildOf(head),
    ));
    commands.spawn((
        Text::new("Select Character"),
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
                padding: UiRect::new(Val::Px(64.0), Val::Px(64.0), Val::Px(130.0), Val::Px(38.0)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    commands.spawn((
        HeroPanel,
        Node {
            width: Val::Px(392.0),
            margin: UiRect::right(Val::Px(26.0)),
            ..default()
        },
        ChildOf(stage),
    ));
    commands.spawn((
        CharacterGrid,
        Node {
            width: Val::Px(700.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            ..default()
        },
        ChildOf(stage),
    ));

    requests.write(RequestCharacterListEvent);
}

/// Stores the latest character list and arms a card rebuild.
fn receive_character_list(
    mut events: MessageReader<CharacterListReceivedEvent>,
    mut data: ResMut<CharacterSelectionData>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut roster_page: ResMut<RosterPage>,
) {
    let Some(event) = events.read().last() else {
        return;
    };
    data.characters = event.characters.clone();
    data.max_slots = event.max_slots;
    data.display_pages = event.display_pages.max(1);
    built.0 = false;
    pending.0 = None;
    roster_page.0 = 0;
}

/// Builds (or rebuilds) the compact slot cards under the grid container.
/// Waits for the diorama target when occupied slots exist (hero panel needs it).
#[allow(clippy::too_many_arguments)]
fn build_cards(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    page: Res<RosterPage>,
    mut built: ResMut<CardsBuilt>,
    container: Query<Entity, With<CharacterGrid>>,
    existing_cards: Query<Entity, With<CharacterCard>>,
    existing_nav: Query<Entity, With<RosterNav>>,
) {
    if built.0 || data.characters.is_empty() {
        return;
    }

    let slots = (data.max_slots as usize).min(data.characters.len());
    let has_occupied = data.characters[..slots].iter().any(Option::is_some);
    if has_occupied && diorama.target.is_none() {
        return;
    }

    let Ok(container) = container.single() else {
        return;
    };

    for card in &existing_cards {
        commands.entity(card).despawn();
    }
    for nav in &existing_nav {
        commands.entity(nav).despawn();
    }

    // Lay the roster out in pages of 3 slots (RO style) when the server
    // advertised more than one page; otherwise show every slot at once.
    let total_pages = (data.display_pages as usize).max(1);
    let page_size = if total_pages > 1 {
        ROSTER_PAGE_SIZE
    } else {
        slots
    };
    let page = page.0.min(total_pages.saturating_sub(1));
    let start = (page * page_size).min(slots);
    let end = (start + page_size).min(slots);

    let font_bold = asset_server.load(theme::FONT_BODY_BOLD);
    let font_body = asset_server.load(theme::FONT_BODY);

    for (offset, entry) in data.characters[start..end].iter().enumerate() {
        let slot = (start + offset) as u8;
        match entry {
            Some(info) => spawn_occupied_card(
                &mut commands,
                container,
                slot,
                info,
                font_bold.clone(),
                font_body.clone(),
            ),
            None => spawn_empty_card(
                &mut commands,
                &asset_server,
                container,
                slot,
                font_body.clone(),
            ),
        }
    }

    if total_pages > 1 {
        spawn_page_nav(
            &mut commands,
            &asset_server,
            container,
            page,
            total_pages,
            font_body,
        );
    }

    built.0 = true;
}

/// Spawns the prev/next page bar under the slot cards. Marked `RosterNav` so it
/// is cleared and rebuilt on every grid rebuild (including page changes).
fn spawn_page_nav(
    commands: &mut Commands,
    asset_server: &AssetServer,
    container: Entity,
    page: usize,
    total_pages: usize,
    font: Handle<Font>,
) {
    let bar = commands
        .spawn((
            RosterNav,
            Node {
                width: Val::Px(700.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                margin: UiRect::top(Val::Px(14.0)),
                ..default()
            },
            ChildOf(container),
        ))
        .id();

    if page > 0 {
        spawn_nav_button(
            commands,
            asset_server,
            bar,
            "Prev",
            "chevron-left",
            PageNavStep(-1),
            font.clone(),
        );
    }

    commands.spawn((
        label(
            format!("Page {} / {}", page + 1, total_pages),
            font.clone(),
            13.0,
            theme::TEXT_FAINT,
        ),
        ChildOf(bar),
    ));

    if page + 1 < total_pages {
        spawn_nav_button(
            commands,
            asset_server,
            bar,
            "Next",
            "chevron-right",
            PageNavStep(1),
            font,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_nav_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    text: &str,
    icon: &str,
    step: PageNavStep,
    font: Handle<Font>,
) {
    let btn = commands
        .spawn((
            step,
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, icon, 14.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
    commands.spawn((
        label(text.to_string(), font, 13.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
    commands.entity(btn).observe(
        move |mut click: On<Pointer<Click>>,
              data: Res<CharacterSelectionData>,
              mut page: ResMut<RosterPage>,
              mut built: ResMut<CardsBuilt>| {
            click.propagate(false);
            let total_pages = (data.display_pages as usize).max(1);
            let next = (page.0 as isize + step.0).clamp(0, total_pages as isize - 1) as usize;
            if next != page.0 {
                page.0 = next;
                built.0 = false;
            }
        },
    );
}

fn spawn_occupied_card(
    commands: &mut Commands,
    container: Entity,
    slot: u8,
    info: &CharacterInfoWithJobName,
    font_bold: Handle<Font>,
    font_body: Handle<Font>,
) {
    let level = format!("Lv {}", info.base.base_level);
    let glyph = info.base.name.chars().next().unwrap_or('?').to_string();

    let card = commands
        .spawn((
            CharacterCard,
            CardSlot(slot),
            Pickable::default(),
            Node {
                width: Val::Px(214.0),
                height: Val::Px(66.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(13.0),
                padding: UiRect::all(Val::Px(13.0)),
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

    // Level badge, pinned to the top-right corner of the card.
    let badge = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(9.0),
                right: Val::Px(9.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            BorderColor::all(theme::STROKE),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        label(level, font_body.clone(), 10.5, theme::TEXT_DIM),
        ChildOf(badge),
    ));

    // Glyph lives as a child so the avatar's flex centering actually centers it
    // (a node's own Text isn't affected by align/justify).
    let avatar = commands
        .spawn((
            Node {
                width: Val::Px(46.0),
                height: Val::Px(46.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        label(glyph, font_bold.clone(), 19.0, theme::GOLD),
        ChildOf(avatar),
    ));

    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        label(info.base.name.clone(), font_bold, 15.0, theme::TEXT),
        ChildOf(col),
    ));
    commands.spawn((
        label(info.job_name.clone(), font_body, 11.5, theme::TEXT_FAINT),
        ChildOf(col),
    ));

    let selected_slot = slot as usize;
    commands.entity(card).observe(
        move |_: On<Pointer<Click>>, mut sel: ResMut<SelectedSlot>| {
            sel.0 = selected_slot;
        },
    );
}

fn spawn_empty_card(
    commands: &mut Commands,
    asset_server: &AssetServer,
    container: Entity,
    slot: u8,
    font: Handle<Font>,
) {
    let card = commands
        .spawn((
            CharacterCard,
            CardSlot(slot),
            Pickable::default(),
            Node {
                width: Val::Px(214.0),
                height: Val::Px(66.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(7.0),
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
    // NOTE: bevy_ui borders can't be dashed; the plus-ring + dimmer fill carry
    // the "empty" read instead of the mockup's dashed outline.
    let ring = commands
        .spawn((
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BorderColor::all(theme::STROKE_STRONG),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "plus", 18.0, theme::TEXT_FAINT),
        ChildOf(ring),
    ));
    commands.spawn((
        label("Create", font, 12.0, theme::TEXT_FAINT),
        ChildOf(card),
    ));

    let selected_slot = slot as usize;
    commands.entity(card).observe(
        move |_: On<Pointer<Click>>, mut sel: ResMut<SelectedSlot>| {
            sel.0 = selected_slot;
        },
    );
}

/// Highlights the selected slot card with an emerald border (mirrors the mockup's
/// selected state). Runs after a rebuild and whenever the selection changes.
fn highlight_selected_cards(
    selected: Res<SelectedSlot>,
    built: Res<CardsBuilt>,
    mut cards: Query<(&CardSlot, &mut BorderColor)>,
) {
    if !selected.is_changed() && !built.is_changed() {
        return;
    }
    for (slot, mut border) in &mut cards {
        let color = if slot.0 as usize == selected.0 {
            theme::EMERALD
        } else {
            theme::STROKE
        };
        *border = BorderColor::all(color);
    }
}

/// Despawns and rebuilds the hero panel content when selection or roster changes.
#[allow(clippy::too_many_arguments)]
fn rebuild_hero_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    selected: Res<SelectedSlot>,
    built: Res<CardsBuilt>,
    panel: Query<Entity, With<HeroPanel>>,
    existing: Query<Entity, With<HeroContent>>,
) {
    if !built.0 {
        return;
    }
    if !selected.is_changed() && !built.is_changed() {
        return;
    }
    let Ok(panel) = panel.single() else {
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
                label(
                    info.base.name.clone(),
                    font_title,
                    25.0,
                    theme::DISPLAY_GOLD,
                ),
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
            spawn_enter_button(
                &mut commands,
                &asset_server,
                actions,
                slot,
                font_body.clone(),
            );
            spawn_delete_button(
                &mut commands,
                &asset_server,
                actions,
                info.base.char_id,
                font_body,
            );
        }
        None => {
            commands.spawn((
                label("Empty Slot", font_title, 20.0, theme::DISPLAY_GOLD),
                ChildOf(frame),
            ));
            commands.spawn((
                label(
                    "Forge a new hero.",
                    font_body.clone(),
                    13.0,
                    theme::TEXT_FAINT,
                ),
                ChildOf(frame),
            ));
            spawn_create_button(
                &mut commands,
                &asset_server,
                frame,
                selected.0 as u8,
                font_body,
            );
        }
    }
}

fn spawn_enter_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    slot: u8,
    font: Handle<Font>,
) {
    let btn = commands
        .spawn((
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "play", 15.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
    commands.spawn((
        label("Enter Game", font, 15.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
    commands.entity(btn).observe(
        move |mut click: On<Pointer<Click>>, mut writer: MessageWriter<SelectCharacterEvent>| {
            click.propagate(false);
            writer.write(SelectCharacterEvent { slot });
        },
    );
}

fn spawn_create_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    slot: u8,
    font: Handle<Font>,
) {
    let btn = commands
        .spawn((
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(theme::EMERALD),
            BorderColor::all(theme::GOLD_FAINT),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "plus", 16.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
    commands.spawn((
        label("Create Character", font, 15.0, theme::EMERALD_INK),
        ChildOf(btn),
    ));
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
fn spawn_delete_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    character_id: u32,
    font: Handle<Font>,
) {
    let btn = commands
        .spawn((
            DeleteButton { character_id },
            Pickable::default(),
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.878, 0.384, 0.369, 0.12)),
            BorderColor::all(theme::BAD),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon(asset_server, "trash", 15.0, theme::BAD),
        ChildOf(btn),
    ));
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
    use net_contract::dto::CharacterInfo as ProtocolCharacterInfo;

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
        // The cards spawn `theme::icon` handles; register Image so the load doesn't panic
        // on an unregistered asset type (no SvgLoader here — the handle just stays unloaded).
        app.init_asset::<Image>();
        app.add_message::<SelectCharacterEvent>();
        app.add_message::<DeleteCharacterRequestEvent>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
        app.init_resource::<SelectedSlot>();
        app.init_resource::<RosterPage>();
        app.insert_resource(data);
        app.insert_resource(diorama);
        app.world_mut().spawn(CharacterGrid);
        app.add_systems(Update, build_cards);
        app
    }

    fn occupied_diorama() -> CharacterDiorama {
        let mut diorama = CharacterDiorama::default();
        diorama.target = Some(Handle::default());
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
            display_pages: 1,
        };
        let mut app = card_app(data, occupied_diorama());

        app.update();

        assert_eq!(card_count(&mut app), 3, "one card per slot up to max_slots");
        let texts = all_texts(&mut app);
        assert!(texts.iter().any(|t| t == "Hero"));
        assert!(texts.iter().any(|t| t == "Mage"));
        assert!(
            texts.iter().any(|t| t == "Swordman"),
            "occupied card shows the class name"
        );
        assert!(
            texts.iter().any(|t| t == "Lv 50"),
            "occupied card shows the level badge"
        );
        assert!(
            texts.iter().any(|t| t == "Create"),
            "empty slot shows create"
        );
        assert!(app.world().resource::<CardsBuilt>().0);
    }

    #[test]
    fn is_idempotent_across_frames() {
        let data = CharacterSelectionData {
            characters: vec![Some(with_job("Hero", 1, 0, 50, "Swordman")), None],
            max_slots: 2,
            display_pages: 1,
        };
        let mut app = card_app(data, occupied_diorama());

        app.update();
        app.update();
        app.update();

        assert_eq!(card_count(&mut app), 2);
    }

    #[test]
    fn pages_roster_into_threes_with_nav() {
        let characters = (0..9)
            .map(|i| Some(with_job("Hero", i as u32 + 1, i as u8, 50, "Swordman")))
            .collect();
        let data = CharacterSelectionData {
            characters,
            max_slots: 9,
            display_pages: 3,
        };
        let mut app = card_app(data, occupied_diorama());

        app.update();

        assert_eq!(card_count(&mut app), 3, "only one page of 3 slots is shown");
        let texts = all_texts(&mut app);
        assert!(
            texts.iter().any(|t| t == "Page 1 / 3"),
            "shows the page indicator"
        );
        assert!(
            texts.iter().any(|t| t == "Next"),
            "shows Next on the first page"
        );
        assert!(
            !texts.iter().any(|t| t == "Prev"),
            "no Prev on the first page"
        );
    }

    #[test]
    fn waits_for_diorama_before_building_occupied_cards() {
        let data = CharacterSelectionData {
            characters: vec![Some(with_job("Hero", 1, 0, 50, "Swordman"))],
            max_slots: 1,
            display_pages: 1,
        };
        let mut app = card_app(data, CharacterDiorama::default());

        app.update();

        assert_eq!(card_count(&mut app), 0);
        assert!(!app.world().resource::<CardsBuilt>().0);
    }
}
