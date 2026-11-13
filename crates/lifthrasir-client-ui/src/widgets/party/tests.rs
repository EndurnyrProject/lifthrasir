use bevy::ecs::system::RunSystemOnce;
use bevy::scene::ScenePlugin;
use net_contract::dto::PartyMemberInfo;

use super::*;

const ME: u32 = 1;

fn member(char_id: u32, name: &str) -> PartyMemberInfo {
    PartyMemberInfo {
        char_id,
        name: name.into(),
        base_level: 50,
        online: true,
        map: "prontera".into(),
        job_id: 0,
        hp: 80,
        max_hp: 100,
        sp: 30,
        max_sp: 60,
        ap: 0,
        max_ap: 0,
    }
}

fn wolfpack() -> PartyState {
    PartyState {
        party_id: 7,
        name: "Wolfpack".into(),
        leader_char_id: ME,
        members: vec![member(ME, "Ygor"), member(2, "Freyja")],
        ..default()
    }
}

/// An app holding the party UI under a bare HUD root, as the local player [`ME`].
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<PartyState>()
        .init_resource::<EntityRegistry>()
        .init_resource::<PartyUi>()
        .init_resource::<PendingPartyInvite>()
        .init_resource::<TradeSession>()
        .init_resource::<VendingState>()
        .init_resource::<TargetingMode>()
        .insert_resource(ZoneSession {
            char_id: ME,
            ..default()
        })
        .add_message::<PartyCreateRequested>()
        .add_message::<PartyInviteRequested>()
        .add_message::<PartyInviteResponded>()
        .add_message::<PartyLeaveRequested>()
        .add_message::<PartyKickRequested>()
        .add_message::<PartyLeaderRequested>()
        .add_message::<PartyOptionsRequested>()
        .add_message::<RequestTrade>()
        .add_message::<SkillCastResolved>()
        .add_systems(Update, (rebuild_party_ui, sync_readouts).chain());
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_party_window(&mut commands, root);
    app.world_mut().flush();
    app
}

fn set_window(app: &mut App, visibility: Visibility) {
    *app.world_mut()
        .query_filtered::<&mut Visibility, With<PartyWindowRoot>>()
        .single_mut(app.world_mut())
        .unwrap() = visibility;
}

fn filled(app: &mut App, host: PartyHost) -> bool {
    let world = app.world_mut();
    world
        .query::<(&PartyHost, Option<&Children>)>()
        .iter(world)
        .find(|(kind, _)| **kind == host)
        .is_some_and(|(_, children)| children.is_some_and(|children| !children.is_empty()))
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

fn has_text(app: &mut App, expected: &str) -> bool {
    texts(app).iter().any(|text| text == expected)
}

fn button(app: &mut App, action: PartyAction) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &PartyAction)>()
        .iter(world)
        .find(|(_, found)| **found == action)
        .map(|(entity, _)| entity)
}

fn press(app: &mut App, action: PartyAction) {
    let button = app.world_mut().spawn(action).observe(on_party_button).id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
}

fn sent<M: Message + Clone>(app: &App) -> Vec<M> {
    let messages = app.world().resource::<Messages<M>>();
    messages.get_cursor().read(messages).cloned().collect()
}

#[test]
fn hosts_follow_the_window_the_party_and_the_invitation() {
    let mut app = app();
    app.update();
    for host in [
        PartyHost::Head,
        PartyHost::Form,
        PartyHost::Body,
        PartyHost::Frames,
        PartyHost::Invite,
    ] {
        assert!(
            !filled(&mut app, host),
            "{host:?} while hidden and partyless"
        );
    }

    set_window(&mut app, Visibility::Visible);
    app.update();
    assert!(filled(&mut app, PartyHost::Head) && filled(&mut app, PartyHost::Form));
    assert!(!filled(&mut app, PartyHost::Body) && !filled(&mut app, PartyHost::Frames));
    assert!(has_text(&mut app, "No Party") && has_text(&mut app, "Create Party"));

    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    app.update();
    assert!(filled(&mut app, PartyHost::Body) && filled(&mut app, PartyHost::Frames));
    assert!(has_text(&mut app, "Invite") && !has_text(&mut app, "Create Party"));
    assert!(has_text(&mut app, "2 / 2 online"));

    set_window(&mut app, Visibility::Hidden);
    app.update();
    assert!(!filled(&mut app, PartyHost::Head) && !filled(&mut app, PartyHost::Body));
    assert!(
        filled(&mut app, PartyHost::Frames),
        "frames outlive the window"
    );
}

#[test]
fn vitals_change_in_place_and_a_roster_change_rebuilds() {
    let mut app = app();
    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    set_window(&mut app, Visibility::Visible);
    app.update();
    let card = button(&mut app, PartyAction::Select(2)).unwrap();
    assert!(has_text(&mut app, "80 / 100"));

    app.world_mut().resource_mut::<PartyState>().members[1].hp = 50;
    app.update();
    assert_eq!(button(&mut app, PartyAction::Select(2)), Some(card));
    assert!(has_text(&mut app, "50 / 100"));
    let world = app.world_mut();
    let fills: Vec<_> = world
        .query::<(&MemberReadout, &Node)>()
        .iter(world)
        .filter(|(readout, _)| readout.char_id == 2 && readout.part == ReadoutPart::HpFill)
        .map(|(_, node)| node.width)
        .collect();
    assert_eq!(fills, vec![Val::Percent(50.0); 2], "the card and the frame");

    app.world_mut()
        .resource_mut::<PartyState>()
        .members
        .push(member(3, "Bjorn"));
    app.update();
    assert_ne!(button(&mut app, PartyAction::Select(2)), Some(card));
    assert!(button(&mut app, PartyAction::Select(3)).is_some());
}

#[test]
fn selecting_a_member_offers_what_the_leader_may_do() {
    let mut app = app();
    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    set_window(&mut app, Visibility::Visible);
    app.update();
    assert!(has_text(&mut app, "Select a member"));

    press(&mut app, PartyAction::Select(2));
    app.update();
    assert!(button(&mut app, PartyAction::MakeLeader(2)).is_some());
    assert!(button(&mut app, PartyAction::AskExpel).is_some());

    press(&mut app, PartyAction::AskExpel);
    app.update();
    assert!(has_text(&mut app, "Expel Freyja?"));
    assert!(button(&mut app, PartyAction::MakeLeader(2)).is_none());

    press(&mut app, PartyAction::Expel(2));
    app.update();
    assert_eq!(sent::<PartyKickRequested>(&app)[0].target_char_id, 2);
    assert!(has_text(&mut app, "Select a member"));
}

#[test]
fn submit_founds_a_party_when_partyless_and_invites_when_in_one() {
    let mut app = app();
    let field = app
        .world_mut()
        .spawn((PartyNameField, EditableText::new("  Wolfpack ")))
        .id();
    let typed = |app: &mut App, text: &str| {
        app.world_mut()
            .entity_mut(field)
            .insert(EditableText::new(text));
    };

    press(&mut app, PartyAction::Submit);
    assert_eq!(sent::<PartyCreateRequested>(&app)[0].name, "Wolfpack");
    assert!(sent::<PartyInviteRequested>(&app).is_empty());

    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    typed(&mut app, "Odin");
    press(&mut app, PartyAction::Submit);
    let invites = sent::<PartyInviteRequested>(&app);
    assert_eq!(
        (invites[0].target_char_id, invites[0].target_name.as_str()),
        (0, "Odin")
    );

    typed(&mut app, "   ");
    press(&mut app, PartyAction::Submit);
    assert_eq!(sent::<PartyInviteRequested>(&app).len(), 1, "nothing typed");
    assert_eq!(sent::<PartyCreateRequested>(&app).len(), 1);
}

#[test]
fn submit_name_trims_and_caps() {
    assert_eq!(submit_name("  Wolfpack  "), Some("Wolfpack".to_string()));
    assert_eq!(submit_name("   "), None);
    assert_eq!(submit_name(&"x".repeat(40)).unwrap().len(), NAME_MAX_CHARS);
}

#[test]
fn a_sharing_choice_sends_both_options() {
    let mut app = app();
    app.insert_resource(PartyState {
        item_pickup_share: true,
        ..wolfpack()
    });

    press(&mut app, PartyAction::ExpShare(true));
    press(&mut app, PartyAction::PickupShare(false));

    let options = sent::<PartyOptionsRequested>(&app);
    assert!(options[0].exp_share && options[0].item_pickup_share);
    assert!(!options[1].exp_share && !options[1].item_pickup_share);
}

#[test]
fn only_the_leader_can_pick_a_sharing_choice() {
    let mut app = app();
    app.insert_resource(PartyState {
        leader_char_id: 2,
        ..wolfpack()
    });
    set_window(&mut app, Visibility::Visible);
    app.update();
    assert!(button(&mut app, PartyAction::ExpShare(true)).is_none());

    app.world_mut().resource_mut::<PartyState>().leader_char_id = ME;
    app.update();
    assert!(button(&mut app, PartyAction::ExpShare(true)).is_some());
    assert!(button(&mut app, PartyAction::PickupShare(true)).is_some());
}

#[test]
fn leaving_asks_first() {
    let mut app = app();
    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    set_window(&mut app, Visibility::Visible);

    press(&mut app, PartyAction::AskLeave);
    app.update();
    assert!(has_text(&mut app, "Leaving disbands Wolfpack."));
    assert!(sent::<PartyLeaveRequested>(&app).is_empty());

    press(&mut app, PartyAction::Stay);
    app.update();
    assert!(button(&mut app, PartyAction::AskLeave).is_some());

    press(&mut app, PartyAction::Leave);
    assert_eq!(sent::<PartyLeaveRequested>(&app).len(), 1);
}

#[test]
fn a_frame_click_casts_only_an_armed_entity_skill() {
    let mut app = app();

    press(&mut app, PartyAction::CastOn(2));
    assert!(sent::<SkillCastResolved>(&app).is_empty(), "nothing armed");

    app.insert_resource(TargetingMode::AwaitingGround {
        skill_id: 12,
        level: 1,
    });
    press(&mut app, PartyAction::CastOn(2));
    assert!(sent::<SkillCastResolved>(&app).is_empty(), "a ground skill");

    app.insert_resource(TargetingMode::AwaitingEntity {
        skill_id: 28,
        level: 3,
    });
    press(&mut app, PartyAction::CastOn(2));
    let casts = sent::<SkillCastResolved>(&app);
    assert_eq!((casts[0].skill_id, casts[0].level), (28, 3));
    assert!(matches!(casts[0].target, CastTarget::Entity(2)));
    assert_eq!(
        *app.world().resource::<TargetingMode>(),
        TargetingMode::Idle
    );
}

#[test]
fn collapsing_the_frames_keeps_the_plate_and_drops_the_members() {
    let mut app = app();
    *app.world_mut().resource_mut::<PartyState>() = wolfpack();
    app.update();
    assert!(button(&mut app, PartyAction::CastOn(2)).is_some());

    press(&mut app, PartyAction::ToggleFrames);
    app.update();
    assert!(button(&mut app, PartyAction::CastOn(2)).is_none());
    assert!(button(&mut app, PartyAction::ToggleFrames).is_some());
}

#[test]
fn answering_an_invite_responds_once_and_clears_it() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<PendingPartyInvite>()
        .party_id = 42;

    press(&mut app, PartyAction::AnswerInvite(true));
    press(&mut app, PartyAction::AnswerInvite(false));

    let responses = sent::<PartyInviteResponded>(&app);
    assert_eq!(responses.len(), 1);
    assert_eq!((responses[0].party_id, responses[0].accept), (42, true));
    assert!(!app.world().resource::<PendingPartyInvite>().is_pending());
}

#[test]
fn distance_is_counted_in_cells_while_both_characters_are_in_view() {
    let mut app = app();
    let at = |x: f32, z: f32| {
        GlobalTransform::from_xyz(x * RO_UNITS_PER_CELL, 0.0, z * RO_UNITS_PER_CELL)
    };
    let own = app.world_mut().spawn(at(10.0, 10.0)).id();
    let other = app.world_mut().spawn(at(13.0, 3.0)).id();
    let mut registry = app.world_mut().resource_mut::<EntityRegistry>();
    registry.set_local_player(own, ME);
    registry.register_entity(2, other);

    let cells = app
        .world_mut()
        .run_system_once(|data: PartyData| (data.cells(2), data.cells(3)))
        .unwrap();

    assert_eq!(cells, (Some(7), None), "the longer axis, RO style");
}

fn visibility_app() -> App {
    let mut app = App::new();
    app.init_resource::<PartyState>();
    app.world_mut().spawn((PartyWindowRoot, Visibility::Hidden));
    app.add_systems(Update, party_visibility);
    app
}

fn root_visibility(app: &mut App) -> Visibility {
    *app.world_mut()
        .query_filtered::<&Visibility, With<PartyWindowRoot>>()
        .single(app.world())
        .unwrap()
}

fn set_party(app: &mut App, party_id: u32) {
    app.world_mut().resource_mut::<PartyState>().party_id = party_id;
}

#[test]
fn stays_hidden_on_join_edge_and_closes_on_leave_edge() {
    let mut app = visibility_app();

    app.update();
    assert_eq!(root_visibility(&mut app), Visibility::Hidden);

    set_party(&mut app, 7);
    app.update();
    assert_eq!(root_visibility(&mut app), Visibility::Hidden);

    set_window(&mut app, Visibility::Visible);
    set_party(&mut app, 0);
    app.update();
    assert_eq!(root_visibility(&mut app), Visibility::Hidden);
}

#[test]
fn manual_close_is_not_reopened_by_same_party_update() {
    let mut app = visibility_app();

    set_party(&mut app, 7);
    app.update();
    set_window(&mut app, Visibility::Visible);
    assert_eq!(root_visibility(&mut app), Visibility::Visible);

    set_window(&mut app, Visibility::Hidden);
    app.world_mut().resource_mut::<PartyState>().set_changed();
    app.update();
    assert_eq!(root_visibility(&mut app), Visibility::Hidden);
}
