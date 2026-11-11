use super::*;
use net_contract::dto::CharacterInfo;

#[test]
fn slot_tiers_and_creation_limit_follow_server_metadata() {
    let info = net_contract::dto::CharacterSlotInfo {
        normal_slots: 2,
        premium_slots: 1,
        billing_slots: 1,
        producible_slots: 3,
        valid_slots: 5,
    };

    assert_eq!(slot_tier(&info, 0), SlotTier::Normal);
    assert_eq!(slot_tier(&info, 2), SlotTier::Premium);
    assert_eq!(slot_tier(&info, 3), SlotTier::Billing);
    assert_eq!(slot_tier(&info, 4), SlotTier::Locked);
    assert!(can_create_in_slot(&info, 2));
    assert!(!can_create_in_slot(&info, 3));
}

#[test]
fn unavailable_paid_tiers_keep_their_labels() {
    assert_eq!(
        vacant_slot_label(SlotKind::Vacant, SlotTier::Premium, false),
        "PREMIUM"
    );
    assert_eq!(
        vacant_slot_label(SlotKind::Vacant, SlotTier::Billing, false),
        "BILLING"
    );
    assert_eq!(
        vacant_slot_label(SlotKind::Vacant, SlotTier::Locked, false),
        "LOCKED"
    );
}

#[test]
fn slot_layout_marks_first_vacancy_as_new_hero() {
    let data = CharacterSelectionData {
        characters: vec![
            Some(with_job("Hero", 0)),
            None,
            Some(with_job("Mage", 2)),
            None,
        ],
        slot_info: CharacterSlotInfo {
            normal_slots: 4,
            producible_slots: 4,
            valid_slots: 4,
            ..default()
        },
    };
    assert_eq!(slot_kind(&data, 0), SlotKind::Occupied);
    assert_eq!(slot_kind(&data, 1), SlotKind::NewHero);
    assert_eq!(slot_kind(&data, 2), SlotKind::Occupied);
    assert_eq!(slot_kind(&data, 3), SlotKind::Vacant);
}

#[test]
fn deletion_failure_is_kept_for_the_matching_character() {
    use lifthrasir_client::domain::character::events::CharacterDeletionFailedEvent;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<DeletionFeedback>();
    app.add_message::<CharacterDeletionFailedEvent>();
    app.add_systems(Update, surface_deletion_failure);
    app.world_mut().write_message(CharacterDeletionFailedEvent {
        character_id: 7,
        error: "Cannot delete character".into(),
    });

    app.update();

    let feedback = app.world().resource::<DeletionFeedback>();
    assert_eq!(feedback.for_character(7), Some("Cannot delete character"));
    assert_eq!(feedback.for_character(8), None);
}

#[test]
fn refreshed_character_list_clears_deletion_failure() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<CharacterSelectionData>();
    app.init_resource::<CardsBuilt>();
    app.init_resource::<PendingDeletion>();
    app.init_resource::<DeletionFeedback>();
    app.add_message::<CharacterListReceivedEvent>();
    app.add_systems(Update, receive_character_list);
    app.world_mut().resource_mut::<DeletionFeedback>().0 = Some(DeletionFailure {
        character_id: 7,
        error: "Cannot delete character".into(),
    });
    app.world_mut().write_message(CharacterListReceivedEvent {
        characters: Vec::new(),
        slot_info: CharacterSlotInfo {
            normal_slots: 3,
            producible_slots: 3,
            valid_slots: 3,
            ..default()
        },
        display_pages: 1,
    });

    app.update();

    assert!(app.world().resource::<DeletionFeedback>().0.is_none());
}

#[test]
fn pending_deletion_cannot_be_requested_again() {
    assert!(can_request_deletion(0));
    assert!(!can_request_deletion(1_800_000_000));
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
fn job_level_progress_is_clamped() {
    assert_eq!(job_level_fraction(25, 50), 0.5);
    assert_eq!(job_level_fraction(80, 50), 1.0);
    assert_eq!(job_level_fraction(1, 0), 0.0);
}

#[test]
fn cycle_slot_wraps_and_ignores_empty_or_single_rosters() {
    assert_eq!(cycle_slot(&[1, 4, 7], 1, 1), 4);
    assert_eq!(cycle_slot(&[1, 4, 7], 7, 1), 1);
    assert_eq!(cycle_slot(&[1, 4, 7], 1, -1), 7);
    assert_eq!(cycle_slot(&[], 3, 1), 3);
    assert_eq!(cycle_slot(&[4], 4, 1), 4);
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

#[test]
fn job_level_caps_cover_novice_and_job_tiers() {
    assert_eq!(job_level_cap(0), 10);
    assert_eq!(job_level_cap(1), 50);
    assert_eq!(job_level_cap(7), 50);
    assert_eq!(job_level_cap(4008), 50);
    assert_eq!(job_level_cap(u16::MAX), 50);
}

#[test]
fn doom_label_counts_down_on_the_server_clock() {
    assert_eq!(doom_label(100_000, Some(33_850)), "18 h 22 m 30 s");
    assert_eq!(doom_label(100, Some(500)), "0 h 00 m 00 s");
    assert_eq!(doom_label(100, None), "Pending");
}

#[test]
fn nudge_prefers_status_points() {
    let mut hero = CharacterInfo::default();
    assert_eq!(nudge(&hero), None);
    hero.skill_point = 2;
    assert_eq!(nudge(&hero).as_deref(), Some("2 skill points to spend"));
    hero.status_point = 12;
    assert_eq!(nudge(&hero).as_deref(), Some("12 status points to spend"));
}

/// An app showing a roster of Ygor (slot 0), Brann (slot 1, deletion pending) and
/// three free slots, one of them premium.
fn screen_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
    ));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.add_message::<SelectCharacterEvent>();
    app.add_message::<DeleteCharacterRequestEvent>();
    app.init_resource::<CardsBuilt>();
    app.init_resource::<PendingDeletion>();
    app.init_resource::<DeletionFeedback>();
    app.init_resource::<SelectedSlot>();
    app.init_resource::<NextState<GameState>>();
    app.init_resource::<CharacterDiorama>();
    app.world_mut().resource_mut::<CharacterDiorama>().target = Some(Handle::default());
    let mut ygor = with_job("Ygor", 0);
    ygor.base.char_id = 1;
    ygor.base.base_level = 99;
    ygor.base.zeny = 12_480_300;
    ygor.base.last_map = "prontera.gat".into();
    let mut brann = with_job("Brann", 1);
    brann.base.char_id = 2;
    brann.base.delete_date = 1_800_000_000;
    app.insert_resource(CharacterSelectionData {
        characters: vec![Some(ygor), Some(brann), None, None, None],
        slot_info: CharacterSlotInfo {
            normal_slots: 4,
            premium_slots: 1,
            producible_slots: 4,
            valid_slots: 5,
            ..default()
        },
    });
    app.add_observer(enter_game);
    app.add_observer(confirm_delete);
    app.world_mut().spawn(ScreenRoot);
    app.add_systems(
        Update,
        (
            rebuild_screen,
            fill_strip,
            refresh_showcase,
            sync_delete_modal,
            tick_doom_countdown,
        )
            .chain(),
    );
    app.update();
    app.update();
    app
}

fn texts(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect()
}

fn card_kinds(app: &mut App) -> Vec<SlotKind> {
    let mut cards: Vec<SlotCard> = app
        .world_mut()
        .query::<&SlotCard>()
        .iter(app.world())
        .copied()
        .collect();
    cards.sort_by_key(|card| card.slot);
    cards.into_iter().map(|card| card.kind).collect()
}

#[test]
fn screen_shows_the_strip_and_the_selected_heros_codex() {
    let mut app = screen_app();

    assert_eq!(
        card_kinds(&mut app),
        [
            SlotKind::Occupied,
            SlotKind::Occupied,
            SlotKind::NewHero,
            SlotKind::Vacant,
            SlotKind::Vacant,
        ]
    );
    let shown = texts(&mut app);
    for expected in [
        "2 / 4",
        "Novice · Base 99 · Job 0",
        "Base 99",
        "12,480,300 z",
        "prontera",
        "Enter Game",
        "NEW HERO",
        "EMPTY",
        "PREMIUM",
        "Deleting",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }

    app.world_mut().resource_mut::<SelectedSlot>().0 = 1;
    app.update();
    app.update();

    let shown = texts(&mut app);
    for expected in ["Deletion Pending", "DELETING IN", "Pending"] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == "12,480,300 z"));
    assert_eq!(card_kinds(&mut app).len(), 5);
}

#[test]
fn entering_selects_the_slot_unless_its_hero_is_being_deleted() {
    let mut app = screen_app();

    app.world_mut().resource_mut::<SelectedSlot>().0 = 1;
    app.world_mut().trigger(EnterGame);
    assert!(
        app.world()
            .resource::<Messages<SelectCharacterEvent>>()
            .is_empty()
    );

    app.world_mut().resource_mut::<SelectedSlot>().0 = 0;
    app.world_mut().trigger(EnterGame);
    assert_eq!(
        app.world()
            .resource::<Messages<SelectCharacterEvent>>()
            .len(),
        1
    );
}

#[test]
fn delete_modal_opens_for_the_armed_hero_and_needs_the_typed_name() {
    let mut app = screen_app();

    app.world_mut().resource_mut::<PendingDeletion>().0 = Some(1);
    app.update();
    app.update();

    let mut modals = app.world_mut().query::<&DeleteModal>();
    assert_eq!(modals.single(app.world()).unwrap().name, "Ygor");

    app.world_mut().trigger(ConfirmDelete);
    assert!(
        app.world()
            .resource::<Messages<DeleteCharacterRequestEvent>>()
            .is_empty()
    );
    assert_eq!(app.world().resource::<PendingDeletion>().0, Some(1));

    app.world_mut().resource_mut::<PendingDeletion>().0 = None;
    app.update();
    app.update();
    assert_eq!(modals.iter(app.world()).count(), 0);
}
