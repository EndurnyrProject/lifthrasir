//! Character selection screen ("Choose Your Hero").
//!
//! BSN scenes ([`scene`]) over the shaded menu background: the selected hero on a
//! pedestal, the Hero Codex window beside it and the slot strip below. The roster comes
//! from `CharacterListReceivedEvent`. The frame and the strip are rebuilt when the
//! roster changes; [`SelectedSlot`] only refreshes the stage and the codex, so a card
//! keeps its entity (and its double-click count) while the selection moves.
//! Entering writes `SelectCharacterEvent` and the engine drives the transition.
//! Deleting opens a modal that asks for the hero's name before it writes
//! `DeleteCharacterRequestEvent`; the server schedules the deletion and the refreshed
//! list reports its `delete_date`.

mod scene;
#[cfg(test)]
mod tests;

use bevy::input_focus::AutoFocus;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui_widgets::{Activate, TextInput};
use lifthrasir_client::{
    core::state::GameState,
    domain::character::events::{
        CharacterDeletionFailedEvent, CharacterInfoWithJobName, CharacterListReceivedEvent,
        DeleteCharacterRequestEvent, RequestCharacterListEvent, SelectCharacterEvent,
    },
};
use lifthrasir_ui::theme::ro_dark;
use net_contract::{
    dto::{CharacterInfo, CharacterSlotInfo},
    state::UserSession,
};

use crate::screens::{
    character_create::CreationSlot,
    character_preview::CharacterDiorama,
    shared::{DANGER_TINT, DISABLED_TINT, glow, thousands},
};
use crate::widgets::system_dialog::SystemDialogRoot;

pub struct CharacterSelectScreenPlugin;

impl Plugin for CharacterSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelectionData>();
        app.init_resource::<CardsBuilt>();
        app.init_resource::<PendingDeletion>();
        app.init_resource::<DeletionFeedback>();
        app.init_resource::<SelectedSlot>();
        app.add_observer(enter_game);
        app.add_observer(confirm_delete);
        app.add_systems(
            OnEnter(GameState::CharacterSelection),
            show_character_select_screen,
        );
        app.add_systems(
            Update,
            (
                receive_character_list,
                surface_deletion_failure,
                normalize_selection,
                handle_keys,
                rebuild_screen,
                fill_strip,
                refresh_showcase,
                sync_delete_modal,
            )
                .chain()
                .run_if(in_state(GameState::CharacterSelection)),
        );
        app.add_systems(
            Update,
            (style_cards, tick_doom_countdown, reflect_delete_confirm)
                .run_if(in_state(GameState::CharacterSelection)),
        );
    }
}

#[derive(Resource, Default)]
struct CharacterSelectionData {
    characters: Vec<Option<CharacterInfoWithJobName>>,
    slot_info: CharacterSlotInfo,
}

/// Whether the frame shows the current roster. Cleared by every character list.
#[derive(Resource, Default)]
struct CardsBuilt(bool);

/// The hero whose delete modal is open, by character id.
#[derive(Resource, Default)]
struct PendingDeletion(Option<u32>);

#[derive(Resource, Default)]
struct DeletionFeedback(Option<DeletionFailure>);

struct DeletionFailure {
    character_id: u32,
    error: String,
}

impl DeletionFeedback {
    fn for_character(&self, character_id: u32) -> Option<&str> {
        self.0
            .as_ref()
            .filter(|failure| failure.character_id == character_id)
            .map(|failure| failure.error.as_str())
    }

    fn clear(&mut self) {
        self.0 = None;
    }
}

#[derive(Resource, Default)]
struct SelectedSlot(usize);

#[derive(Component)]
struct ScreenRoot;

/// The frame built for the current roster.
#[derive(Component)]
struct SceneContent;

/// Holds the stage and the codex of the selected slot.
#[derive(Component, Default, Clone)]
struct Showcase;

/// Holds one [`SlotCard`] per character slot.
#[derive(Component, Default, Clone)]
struct SlotStrip;

/// A card of the slot strip.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct SlotCard {
    slot: usize,
    kind: SlotKind,
    /// The occupant is scheduled for deletion.
    doomed: bool,
}

/// The codex line counting down to the Unix time in `.0`.
#[derive(Component, Clone)]
struct DoomCountdown(u32);

/// The delete modal, holding the name that confirms it.
#[derive(Component, Clone)]
struct DeleteModal {
    name: String,
}

#[derive(Component, Default, Clone)]
#[require(AutoFocus, Pickable, TextCursorStyle)]
struct ConfirmField;

#[derive(Component, Default, Clone)]
struct ConfirmHint;

#[derive(Component, Default, Clone)]
struct ConfirmButton;

/// Requests entering the game with the selected hero (Enter, double-click or the
/// primary button).
#[derive(Event)]
struct EnterGame;

/// Requests the deletion armed in the modal (Enter or its Delete button).
#[derive(Event)]
struct ConfirmDelete;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotKind {
    Occupied,
    NewHero,
    Vacant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotTier {
    Normal,
    Premium,
    Billing,
    Locked,
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
    mut feedback: ResMut<DeletionFeedback>,
    mut selected: ResMut<SelectedSlot>,
    mut requests: MessageWriter<RequestCharacterListEvent>,
) {
    built.0 = false;
    pending.0 = None;
    feedback.clear();
    selected.0 = 0;

    commands.spawn((
        ScreenRoot,
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        DespawnOnExit(GameState::CharacterSelection),
    ));
    requests.write(RequestCharacterListEvent);
}

fn receive_character_list(
    mut events: MessageReader<CharacterListReceivedEvent>,
    mut data: ResMut<CharacterSelectionData>,
    mut built: ResMut<CardsBuilt>,
    mut pending: ResMut<PendingDeletion>,
    mut feedback: ResMut<DeletionFeedback>,
) {
    let Some(event) = events.read().last() else {
        return;
    };
    data.characters = event.characters.clone();
    data.slot_info.clone_from(&event.slot_info);
    built.0 = false;
    pending.0 = None;
    feedback.clear();
}

fn surface_deletion_failure(
    mut failures: MessageReader<CharacterDeletionFailedEvent>,
    mut feedback: ResMut<DeletionFeedback>,
) {
    let Some(failure) = failures.read().last() else {
        return;
    };
    feedback.0 = Some(DeletionFailure {
        character_id: failure.character_id,
        error: failure.error.clone(),
    });
}

fn normalize_selection(
    data: Res<CharacterSelectionData>,
    mut selected: ResMut<SelectedSlot>,
    mut pending: ResMut<PendingDeletion>,
    mut feedback: ResMut<DeletionFeedback>,
) {
    if !data.is_changed() {
        return;
    }
    let selection_still_occupied = data.characters.get(selected.0).is_some_and(Option::is_some);
    if !selection_still_occupied {
        selected.0 = data
            .characters
            .iter()
            .take(data.slot_info.valid_slots as usize)
            .position(Option::is_some)
            .unwrap_or(0);
    }
    pending.0 = None;
    feedback.clear();
}

/// Left/Right cycle the occupied slots and Enter plays. While the delete modal is open
/// the keys belong to it: Enter confirms and Escape closes.
fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    data: Res<CharacterSelectionData>,
    mut selected: ResMut<SelectedSlot>,
    mut pending: ResMut<PendingDeletion>,
    mut feedback: ResMut<DeletionFeedback>,
    dialogs: Query<(), With<SystemDialogRoot>>,
    mut commands: Commands,
) {
    if !dialogs.is_empty() {
        return;
    }
    let enter = keys.just_pressed(KeyCode::Enter);
    if pending.0.is_some() {
        if enter {
            commands.trigger(ConfirmDelete);
        }
        if keys.just_pressed(KeyCode::Escape) {
            pending.0 = None;
        }
        return;
    }
    if enter {
        commands.trigger(EnterGame);
    }
    let direction = keys.just_pressed(KeyCode::ArrowRight) as i32
        - keys.just_pressed(KeyCode::ArrowLeft) as i32;
    let next = cycle_slot(&occupied_slots(&data), selected.0, direction);
    if next != selected.0 {
        selected.0 = next;
        feedback.clear();
    }
}

/// Enters the game with the selected hero, unless it is scheduled for deletion.
fn enter_game(
    _: On<EnterGame>,
    selected: Res<SelectedSlot>,
    data: Res<CharacterSelectionData>,
    mut writer: MessageWriter<SelectCharacterEvent>,
) {
    let Some(info) = featured(&data.characters, selected.0) else {
        return;
    };
    if info.base.delete_date != 0 {
        return;
    }
    writer.write(SelectCharacterEvent {
        slot: selected.0 as u8,
    });
}

/// A click on a hero card selects it and a double-click plays. The New Hero card opens
/// the creation screen for its slot.
fn pick_card(
    click: On<PointerClick>,
    cards: Query<&SlotCard>,
    mut selected: ResMut<SelectedSlot>,
    mut feedback: ResMut<DeletionFeedback>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    let Ok(card) = cards.get(click.entity) else {
        return;
    };
    match card.kind {
        SlotKind::Occupied => {
            if selected.0 != card.slot {
                selected.0 = card.slot;
                feedback.clear();
            }
            if click.count == 2 {
                commands.trigger(EnterGame);
            }
        }
        SlotKind::NewHero => {
            commands.insert_resource(CreationSlot(card.slot as u8));
            next.set(GameState::CharacterCreation);
        }
        SlotKind::Vacant => {}
    }
}

/// Rebuilds the frame once the roster (and, for a roster with heroes, its preview
/// target) is known, and again whenever either changes.
fn rebuild_screen(
    mut commands: Commands,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    session: Option<Res<UserSession>>,
    mut built: ResMut<CardsBuilt>,
    root: Query<Entity, With<ScreenRoot>>,
    old_content: Query<Entity, With<SceneContent>>,
) {
    if data.slot_info.valid_slots == 0 {
        return;
    }
    if occupied_count(&data) > 0 && diorama.target.is_none() {
        return;
    }
    if built.0 && !diorama.is_changed() {
        return;
    }
    let Ok(root) = root.single() else {
        return;
    };
    for entity in &old_content {
        commands.entity(entity).despawn();
    }
    let world = session
        .as_ref()
        .and_then(|session| session.selected_server.as_ref())
        .map_or("Endurnir", |server| server.name.as_str())
        .to_string();
    let username = session
        .as_ref()
        .map(|session| session.username.clone())
        .unwrap_or_default();
    commands
        .spawn_scene(scene::frame(
            world,
            username,
            occupied_count(&data),
            data.slot_info.producible_slots,
        ))
        .insert((SceneContent, ChildOf(root)));
    built.0 = true;
}

/// Spawns one card per slot into a freshly built strip.
fn fill_strip(
    mut commands: Commands,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    strip: Single<(Entity, Has<Children>), With<SlotStrip>>,
) {
    let (strip, filled) = *strip;
    if filled {
        return;
    }
    for slot in 0..usize::from(data.slot_info.valid_slots) {
        let mut card = match featured(&data.characters, slot) {
            Some(info) => commands.spawn_scene(scene::hero_card(slot, info, &diorama)),
            None if slot_kind(&data, slot) == SlotKind::NewHero => {
                commands.spawn_scene(scene::new_hero_card(slot))
            }
            None => commands.spawn_scene(scene::vacant_card(
                slot,
                slot_tier(&data.slot_info, slot),
                can_create_in_slot(&data.slot_info, slot),
            )),
        };
        card.insert(ChildOf(strip));
    }
}

/// Fills the stage and the codex for the selected slot: once for a fresh frame, then
/// whenever the selection or the deletion feedback changes.
fn refresh_showcase(
    mut commands: Commands,
    data: Res<CharacterSelectionData>,
    diorama: Res<CharacterDiorama>,
    selected: Res<SelectedSlot>,
    feedback: Res<DeletionFeedback>,
    showcase: Single<(Entity, Has<Children>), With<Showcase>>,
) {
    let (showcase, filled) = *showcase;
    if filled && !selected.is_changed() && !feedback.is_changed() {
        return;
    }
    let error = featured(&data.characters, selected.0)
        .and_then(|info| feedback.for_character(info.base.char_id));
    commands.entity(showcase).despawn_children();
    commands
        .spawn_scene(scene::showcase(&data, &diorama, selected.0, error))
        .insert(ChildOf(showcase));
}

/// Opens the delete modal for the armed hero and closes it once nothing is armed.
fn sync_delete_modal(
    mut commands: Commands,
    pending: Res<PendingDeletion>,
    data: Res<CharacterSelectionData>,
    root: Query<Entity, With<ScreenRoot>>,
    modals: Query<Entity, With<DeleteModal>>,
) {
    if !pending.is_changed() {
        return;
    }
    for entity in &modals {
        commands.entity(entity).despawn();
    }
    let Some(name) = pending.0.and_then(|id| character_name(&data, id)) else {
        return;
    };
    let Ok(root) = root.single() else {
        return;
    };
    commands
        .spawn_scene(scene::delete_modal(name.to_string()))
        .insert(ChildOf(root));
}

/// Shows whether the typed name matches and lights the modal's Delete button once it does.
fn reflect_delete_confirm(
    modals: Query<&DeleteModal>,
    fields: Query<&EditableText, With<ConfirmField>>,
    mut hints: Query<&mut Text, With<ConfirmHint>>,
    mut buttons: Query<&mut ImageNode, With<ConfirmButton>>,
) {
    let (Ok(modal), Ok(field)) = (modals.single(), fields.single()) else {
        return;
    };
    let typed = field.value().to_string();
    let matches = typed == modal.name;
    let hint = if typed.is_empty() || matches {
        ""
    } else {
        "That doesn't match"
    };
    for mut text in &mut hints {
        if text.0 != hint {
            text.0 = hint.to_string();
        }
    }
    let tint = if matches { DANGER_TINT } else { DISABLED_TINT };
    for mut art in &mut buttons {
        if art.color != tint {
            art.color = tint;
        }
    }
}

/// Requests the armed deletion once the typed name matches the hero's.
fn confirm_delete(
    _: On<ConfirmDelete>,
    modals: Query<&DeleteModal>,
    fields: Query<&EditableText, With<ConfirmField>>,
    mut pending: ResMut<PendingDeletion>,
    mut writer: MessageWriter<DeleteCharacterRequestEvent>,
) {
    let (Ok(modal), Ok(field)) = (modals.single(), fields.single()) else {
        return;
    };
    let Some(character_id) = pending.0 else {
        return;
    };
    if field.value().to_string() != modal.name {
        return;
    }
    if arm_delete(&mut pending.0, character_id) == DeleteAction::Confirmed {
        writer.write(DeleteCharacterRequestEvent { character_id });
    }
}

/// Recolours cards whose hover state changed, or all of them when the selection moved.
fn style_cards(
    selected: Res<SelectedSlot>,
    mut cards: Query<(
        &SlotCard,
        Ref<Hovered>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BoxShadow,
    )>,
) {
    for (card, hovered, mut background, mut border, mut halo) in &mut cards {
        if !selected.is_changed() && !hovered.is_changed() {
            continue;
        }
        let (fill, ring, light) = card_style(card, card.slot == selected.0, hovered.get());
        *background = BackgroundColor(fill);
        *border = BorderColor::all(ring);
        *halo = glow(light);
    }
}

/// Keeps the codex's deletion countdown on the current second of the server clock.
fn tick_doom_countdown(
    session: Option<Res<UserSession>>,
    mut labels: Query<(&DoomCountdown, &mut Text)>,
) {
    let now = session.as_ref().and_then(|session| session.server_now());
    for (doom, mut text) in &mut labels {
        let label = doom_label(doom.0, now);
        if text.0 != label {
            text.0 = label;
        }
    }
}

/// Fill, ring and halo of a slot card. A halo of [`Color::NONE`] means no glow.
fn card_style(card: &SlotCard, selected: bool, hovered: bool) -> (Color, Color, Color) {
    let rest = ro_dark::INSET.with_alpha(0.88);
    match (card.kind, selected, hovered) {
        (SlotKind::Occupied, true, _) => {
            let accent = if card.doomed {
                ro_dark::STATUS_FULL
            } else {
                ro_dark::SELECT_RING
            };
            (
                ro_dark::SLOT_HOVER.with_alpha(0.95),
                accent,
                accent.with_alpha(0.3),
            )
        }
        (SlotKind::Occupied, false, _) if card.doomed => {
            (rest, ro_dark::STATUS_FULL.with_alpha(0.7), Color::NONE)
        }
        (SlotKind::Occupied, false, true) => (
            ro_dark::SLOT_HOVER.with_alpha(0.92),
            ro_dark::BUTTON_STROKE,
            Color::NONE,
        ),
        (SlotKind::Occupied, false, false) => (rest, ro_dark::SLOT_STROKE, Color::NONE),
        (SlotKind::NewHero, _, true) => (
            ro_dark::PRIMARY.with_alpha(0.45),
            ro_dark::FROST_GLOW,
            ro_dark::FROST_GLOW.with_alpha(0.3),
        ),
        (SlotKind::NewHero, _, false) => (
            ro_dark::PRIMARY.with_alpha(0.28),
            ro_dark::FROST_GLOW.with_alpha(0.55),
            Color::NONE,
        ),
        (SlotKind::Vacant, ..) => (
            ro_dark::SCROLL_TRACK.with_alpha(0.7),
            ro_dark::SLOT_STROKE.with_alpha(0.5),
            Color::NONE,
        ),
    }
}

/// "18 h 22 m 10 s" until the Unix time `delete_date`, or "Pending" while the server
/// clock `now` is unknown.
fn doom_label(delete_date: u32, now: Option<u64>) -> String {
    let Some(now) = now else {
        return "Pending".to_string();
    };
    let left = u64::from(delete_date).saturating_sub(now);
    format!(
        "{} h {:02} m {:02} s",
        left / 3_600,
        left / 60 % 60,
        left % 60
    )
}

/// A map resource name without its `.gat` suffix.
fn map_label(last_map: &str) -> String {
    match last_map.trim_end_matches(".gat") {
        "" => "Unknown".to_string(),
        map => map.to_string(),
    }
}

/// The reminder for unspent status points, else unspent skill points.
fn nudge(hero: &CharacterInfo) -> Option<String> {
    match (hero.status_point, hero.skill_point) {
        (0, 0) => None,
        (0, points) => Some(format!("{points} skill points to spend")),
        (points, _) => Some(format!("{points} status points to spend")),
    }
}

fn featured(
    characters: &[Option<CharacterInfoWithJobName>],
    selected: usize,
) -> Option<&CharacterInfoWithJobName> {
    characters.get(selected).and_then(Option::as_ref)
}

fn character_name(data: &CharacterSelectionData, character_id: u32) -> Option<&str> {
    data.characters
        .iter()
        .flatten()
        .find(|info| info.base.char_id == character_id)
        .map(|info| info.base.name.as_str())
}

fn occupied_slots(data: &CharacterSelectionData) -> Vec<usize> {
    data.characters
        .iter()
        .take(data.slot_info.valid_slots as usize)
        .enumerate()
        .filter_map(|(slot, character)| character.as_ref().map(|_| slot))
        .collect()
}

fn occupied_count(data: &CharacterSelectionData) -> usize {
    occupied_slots(data).len()
}

fn slot_kind(data: &CharacterSelectionData, slot: usize) -> SlotKind {
    if featured(&data.characters, slot).is_some() {
        SlotKind::Occupied
    } else if can_create_in_slot(&data.slot_info, slot)
        && (0..slot).all(|prior| featured(&data.characters, prior).is_some())
    {
        SlotKind::NewHero
    } else {
        SlotKind::Vacant
    }
}

fn slot_tier(info: &CharacterSlotInfo, slot: usize) -> SlotTier {
    let normal_end = usize::from(info.normal_slots);
    let premium_end = normal_end + usize::from(info.premium_slots);
    let billing_end = premium_end + usize::from(info.billing_slots);

    if slot < normal_end {
        SlotTier::Normal
    } else if slot < premium_end {
        SlotTier::Premium
    } else if slot < billing_end {
        SlotTier::Billing
    } else {
        SlotTier::Locked
    }
}

fn can_create_in_slot(info: &CharacterSlotInfo, slot: usize) -> bool {
    slot < usize::from(info.producible_slots) && slot < usize::from(info.valid_slots)
}

fn vacant_slot_label(kind: SlotKind, tier: SlotTier, can_create: bool) -> &'static str {
    if kind == SlotKind::NewHero {
        return "NEW HERO";
    }
    match tier {
        SlotTier::Premium => "PREMIUM",
        SlotTier::Billing => "BILLING",
        SlotTier::Normal if can_create => "EMPTY",
        SlotTier::Normal | SlotTier::Locked => "LOCKED",
    }
}

fn can_request_deletion(delete_date: u32) -> bool {
    delete_date == 0
}

/// The two steps of a deletion: the first call arms `character_id` (the modal opens),
/// a second call for the same hero confirms it.
fn arm_delete(pending: &mut Option<u32>, character_id: u32) -> DeleteAction {
    if *pending == Some(character_id) {
        *pending = None;
        DeleteAction::Confirmed
    } else {
        *pending = Some(character_id);
        DeleteAction::Armed
    }
}

fn cycle_slot(occupied: &[usize], current: usize, direction: i32) -> usize {
    if occupied.is_empty() || direction == 0 {
        return current;
    }
    let current_index = occupied
        .iter()
        .position(|&slot| slot == current)
        .unwrap_or(0);
    let next_index = if direction > 0 {
        (current_index + 1) % occupied.len()
    } else {
        (current_index + occupied.len() - 1) % occupied.len()
    };
    occupied[next_index]
}

/// `value / max` clamped to `0.0..=1.0`, and 0 for an unknown `max`.
fn fraction(value: u64, max: u64) -> f32 {
    if max == 0 {
        0.0
    } else {
        (value as f32 / max as f32).clamp(0.0, 1.0)
    }
}

/// The job level cap of a class: 10 for a Novice, 50 for every other job.
fn job_level_cap(class_id: u16) -> u32 {
    if matches!(class_id, 0 | 4001) { 10 } else { 50 }
}

fn job_level_fraction(level: u32, cap: u32) -> f32 {
    fraction(level.into(), cap.into())
}
