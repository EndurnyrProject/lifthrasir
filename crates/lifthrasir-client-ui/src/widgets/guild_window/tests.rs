use bevy::input_focus::FocusCause;
use bevy::scene::ScenePlugin;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_client::domain::guild::GuildPlugin;
use net_contract::dto::{
    GuildActionResult, GuildActivityInfo, GuildActivityKind, GuildErrorKind, GuildInfo,
    GuildInviteInfo, GuildMemberInfo, GuildRelationInfo, GuildSkillInfo,
};
use net_contract::events::{GuildIngress, GuildIngressPayload};

use super::*;

const ASTRID: u32 = 1;
const ME: u32 = 2;
const FREYJA: u32 = 3;
const ORM: u32 = 4;
const GENERATION: ZoneSessionGeneration = ZoneSessionGeneration(9);
/// The moment the view tests look at the guild from.
const NOW: u64 = 100_000_000;

fn position(index: u32, name: &str, perms: [bool; 5], tax: u32) -> GuildPositionInfo {
    let [
        can_invite,
        can_expel,
        can_storage,
        can_notice,
        can_relations,
    ] = perms;
    GuildPositionInfo {
        index,
        name: name.into(),
        can_invite,
        can_expel,
        can_storage,
        tax,
        can_notice,
        can_relations,
    }
}

fn member(char_id: u32, name: &str, position_index: u32, online: bool) -> GuildMemberInfo {
    GuildMemberInfo {
        char_id,
        name: name.into(),
        base_level: 200 + char_id,
        online,
        map: "prontera".into(),
        position_index,
        ..default()
    }
}

/// Astrid leads; the local player is a Vanguard who may invite; Freyja is an Officer
/// who may also expel and post the notice; Orm is an offline Recruit.
fn wardens() -> GuildInfo {
    let mut me = member(ME, "Ygor", 2, true);
    me.exp = 1_284_300;
    let mut orm = member(ORM, "Orm", 19, false);
    orm.last_seen = NOW - 2 * 3600;
    GuildInfo {
        guild_id: 7,
        name: "Wardens".into(),
        master_char_id: ASTRID,
        notice_subject: "Siege prep".into(),
        notice_body: "Bring potions.".into(),
        notice_author: "Astrid".into(),
        notice_posted_at: NOW - 2 * 86_400,
        positions: vec![
            position(0, "Guild Master", [true, true, false, false, false], 0),
            position(1, "Officer", [true, true, false, true, false], 5),
            position(2, "Vanguard", [true, false, false, false, false], 5),
            position(3, "", [false; 5], 0),
            position(19, "Recruit", [false; 5], 10),
        ],
        members: vec![
            member(ASTRID, "Astrid", 0, true),
            me,
            member(FREYJA, "Freyja", 1, true),
            orm,
        ],
        level: 12,
        exp: 64,
        next_exp: 100,
        skill_points: 2,
        skills: vec![
            GuildSkillInfo {
                skill_id: 10_000,
                level: 1,
                max_level: 1,
            },
            GuildSkillInfo {
                skill_id: 10_004,
                level: 5,
                max_level: 10,
            },
        ],
        relations: vec![
            GuildRelationInfo {
                guild_id: 8,
                name: "Ravens".into(),
                kind: GuildRelationKind::Ally,
                level: 14,
                member_count: 47,
                master_name: "Hrafn".into(),
                castles: vec!["Roost".into()],
                ..default()
            },
            GuildRelationInfo {
                guild_id: 9,
                name: "Ash".into(),
                kind: GuildRelationKind::Antagonist,
                ..default()
            },
        ],
        max_members: 22,
        castles: vec!["Bastion".into()],
        activity: vec![GuildActivityInfo {
            kind: GuildActivityKind::Joined,
            subject: "Orm".into(),
            actor: "Freyja".into(),
            at: NOW - 2 * 3600,
            ..default()
        }],
        ..default()
    }
}

/// An app holding the guild UI under a bare HUD root, as the character `me`.
fn app_as(me: u32) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.add_message::<GuildIngress>()
        .add_message::<ZoneDisconnected>()
        .insert_resource(GENERATION)
        .insert_resource(ZoneSession {
            char_id: me,
            ..default()
        })
        .add_plugins(GuildPlugin)
        .init_resource::<GuildUi>()
        .init_resource::<GuildUiSession>()
        .init_resource::<PendingGuildInvite>()
        .init_resource::<emblem::GuildEmblemPreview>()
        .init_resource::<relation_dialogs::PendingRelationConfirmation>()
        .add_message::<GuildInviteRequested>()
        .add_message::<GuildInviteResponded>()
        .add_message::<GuildLeaveRequested>()
        .add_message::<GuildExpelRequested>()
        .add_message::<GuildPositionEditRequested>()
        .add_message::<GuildMemberPositionRequested>()
        .add_message::<GuildNoticeEditRequested>()
        .add_message::<GuildSkillUpRequested>()
        .add_message::<GuildAllianceRequested>()
        .add_message::<GuildAntagonistRemoveRequested>()
        .add_message::<PartyInviteRequested>()
        .add_systems(
            Update,
            reset_guild_ui_session.in_set(GuildSystems::SessionReset),
        )
        .add_systems(
            Update,
            (
                reset_guild_ui_guild,
                feedback::apply_guild_results,
                invite_dialog::receive_invite,
                invite_dialog::expire_pending_invite,
                sync_membership_mode,
                expire_feedback,
                rebuild_guild_ui,
            )
                .chain()
                .in_set(GuildSystems::UiSync),
        );
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_guild_window(&mut commands, root);
    app.world_mut().flush();
    app
}

fn ingress(app: &mut App, payload: GuildIngressPayload) {
    app.world_mut().write_message(GuildIngress {
        generation: GENERATION,
        payload,
    });
    app.update();
}

/// An app whose character `me` is in [`wardens`], with the window open.
fn guilded_as(me: u32) -> App {
    let mut app = app_as(me);
    ingress(&mut app, GuildIngressPayload::Info(wardens()));
    set_window(&mut app, Visibility::Visible);
    // The frame's hosts fill first, the page's the frame after.
    app.update();
    app.update();
    app
}

fn set_window(app: &mut App, visibility: Visibility) {
    *app.world_mut()
        .query_filtered::<&mut Visibility, With<GuildWindowRoot>>()
        .single_mut(app.world_mut())
        .unwrap() = visibility;
}

fn window(app: &mut App) -> Visibility {
    *app.world_mut()
        .query_filtered::<&Visibility, With<GuildWindowRoot>>()
        .single(app.world())
        .unwrap()
}

fn filled(app: &mut App, host: GuildHost) -> bool {
    let world = app.world_mut();
    world
        .query::<(&GuildHost, Option<&Children>)>()
        .iter(world)
        .find(|(kind, _)| **kind == host)
        .is_some_and(|(_, children)| children.is_some_and(|children| !children.is_empty()))
}

fn has_text(app: &mut App, expected: &str) -> bool {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0 == expected)
}

fn button(app: &mut App, action: GuildAction) -> bool {
    let world = app.world_mut();
    world
        .query::<&GuildAction>()
        .iter(world)
        .any(|found| *found == action)
}

fn press(app: &mut App, action: GuildAction) {
    let button = app.world_mut().spawn(action).observe(on_guild_button).id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().despawn(button);
    app.world_mut().flush();
}

/// Lets `duration` pass in one frame.
fn pass(app: &mut App, duration: Duration) {
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(duration);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(duration));
    app.update();
    app.insert_resource(TimeUpdateStrategy::Automatic);
}

/// Presses a button, then lets the page and its hosts catch up.
fn press_and_settle(app: &mut App, action: GuildAction) {
    press(app, action);
    app.update();
    app.update();
}

fn typed<F: Component + Default>(app: &mut App, text: &str) {
    app.world_mut()
        .spawn((F::default(), EditableText::new(text)));
}

fn sent<M: Message + Clone>(app: &App) -> Vec<M> {
    let messages = app.world().resource::<Messages<M>>();
    messages.get_cursor().read(messages).cloned().collect()
}

fn ui(app: &App) -> &GuildUi {
    app.world().resource::<GuildUi>()
}

fn answer(app: &mut App, action: &str, error: GuildErrorKind) {
    ingress(
        app,
        GuildIngressPayload::ActionResult(GuildActionResult {
            action: action.into(),
            success: error == GuildErrorKind::None,
            error,
        }),
    );
}

/// Projects a view of [`wardens`] as `me` sees it at [`NOW`].
fn seen_by<R>(app: &App, me: u32, project: impl FnOnce(Guild) -> R) -> R {
    let state = app.world().resource::<GuildState>();
    project(Guild {
        state,
        info: state.info().unwrap(),
        me,
        jobs: None,
        skills: None,
        now: NOW,
    })
}

#[test]
fn hosts_follow_the_window_the_guild_and_the_tab() {
    let mut app = app_as(ME);
    app.update();
    set_window(&mut app, Visibility::Visible);
    app.update();
    assert_eq!(window(&mut app), Visibility::Hidden, "guildless");
    assert!(!filled(&mut app, GuildHost::Tabs) && !filled(&mut app, GuildHost::Page));

    let mut app = guilded_as(ME);
    for host in [
        GuildHost::Title,
        GuildHost::Tabs,
        GuildHost::Page,
        GuildHost::Summary,
        GuildHost::Notice,
        GuildHost::Activity,
        GuildHost::Footer,
    ] {
        assert!(filled(&mut app, host), "{host:?} on the Overview");
    }
    assert!(has_text(&mut app, "Wardens \u{b7} Vanguard"));
    assert!(has_text(&mut app, "3 online") && has_text(&mut app, "4 / 22"));
    assert!(has_text(&mut app, "Siege prep") && has_text(&mut app, "Bastion"));
    assert!(has_text(
        &mut app,
        "Orm joined the guild \u{b7} Invited by Freyja"
    ));
    assert!(!filled(&mut app, GuildHost::Toast));

    press_and_settle(&mut app, GuildAction::Tab(GuildTab::Members));
    assert!(filled(&mut app, GuildHost::MemberRows) && filled(&mut app, GuildHost::Selection));
    assert!(has_text(&mut app, "3 online of 4") && has_text(&mut app, "Select a member"));
    assert!(!has_text(&mut app, "Siege prep"), "the Overview is gone");
    let world = app.world_mut();
    let summaries = world
        .query::<&GuildHost>()
        .iter(world)
        .filter(|host| **host == GuildHost::Summary)
        .count();
    assert_eq!(summaries, 0, "only the page on show exists");

    set_window(&mut app, Visibility::Hidden);
    app.update();
    assert!(!filled(&mut app, GuildHost::Tabs) && !filled(&mut app, GuildHost::Page));
}

#[test]
fn the_roster_filters_by_text_and_presence_and_sorts() {
    let app = guilded_as(ME);
    let names = |ui: &GuildUi, query: &str| -> Vec<String> {
        seen_by(&app, ME, |guild| view::member_rows(guild, ui, query))
            .into_iter()
            .map(|row| row.name)
            .collect()
    };

    let mut ui = GuildUi::default();
    assert_eq!(
        names(&ui, ""),
        ["Astrid", "Freyja", "Ygor", "Orm"],
        "online by rank, then the offline"
    );
    assert_eq!(names(&ui, " ORM "), ["Orm"]);
    assert!(names(&ui, "nobody").is_empty());

    ui.online_only = true;
    assert_eq!(names(&ui, ""), ["Astrid", "Freyja", "Ygor"]);

    ui.online_only = false;
    ui.sort = MemberSort::Level;
    assert_eq!(names(&ui, ""), ["Orm", "Freyja", "Ygor", "Astrid"]);
    ui.sort_reversed = true;
    assert_eq!(names(&ui, ""), ["Astrid", "Ygor", "Freyja", "Orm"]);

    let rows = seen_by(&app, ME, |guild| {
        view::member_rows(guild, &GuildUi::default(), "")
    });
    let orm = rows.iter().find(|row| row.name == "Orm").unwrap();
    assert_eq!(
        (orm.seen.as_str(), orm.position.as_str()),
        ("2 h ago", "Recruit")
    );
    let me = rows.iter().find(|row| row.me).unwrap();
    assert_eq!(
        (me.exp.as_str(), me.seen.as_str()),
        ("1,284,300", "prontera")
    );
}

#[test]
fn a_click_on_a_column_sorts_by_it_and_a_second_one_flips_it() {
    let mut app = guilded_as(ME);
    press(&mut app, GuildAction::Sort(MemberSort::Name));
    assert_eq!(
        (ui(&app).sort, ui(&app).sort_reversed),
        (MemberSort::Name, false)
    );
    press(&mut app, GuildAction::Sort(MemberSort::Name));
    assert!(ui(&app).sort_reversed);
    press(&mut app, GuildAction::Sort(MemberSort::Exp));
    assert_eq!(
        (ui(&app).sort, ui(&app).sort_reversed),
        (MemberSort::Exp, false)
    );
}

#[test]
fn the_selection_bar_offers_what_the_position_allows() {
    let app = guilded_as(ME);
    let select = |me: u32, target: u32, menu: bool| {
        let ui = GuildUi {
            selected_member: Some(target),
            position_menu: menu,
            ..default()
        };
        seen_by(&app, me, |guild| view::selection(guild, &ui)).unwrap()
    };

    let own = select(ME, ME, false);
    assert!(!own.can_whisper && !own.can_party && !own.can_expel && !own.can_assign);
    assert_eq!(own.title, "Ygor (you)");

    let vanguard = select(ME, FREYJA, false);
    assert!(vanguard.can_whisper && vanguard.can_party);
    assert!(!vanguard.can_expel, "a Vanguard may not expel");

    let officer = select(FREYJA, ORM, false);
    assert!(officer.can_expel && !officer.can_assign && !officer.can_party);
    assert!(officer.meta.ends_with("Recruit \u{b7} Last seen 2 h ago"));
    assert!(!select(FREYJA, ASTRID, false).can_expel, "never the master");

    let master = select(ASTRID, ORM, true);
    assert!(master.can_assign);
    let slots: Vec<_> = master.menu.iter().map(|(index, ..)| *index).collect();
    assert_eq!(slots, [1, 2, 19], "named slots but the master's own");
    assert!(
        master
            .menu
            .iter()
            .any(|(index, _, held)| *index == 19 && *held)
    );
    assert!(select(ASTRID, ASTRID, true).menu.is_empty());
}

#[test]
fn an_invite_is_trimmed_and_only_one_request_is_in_flight() {
    let mut app = guilded_as(ME);
    typed::<GuildInviteNameField>(&mut app, "  Odin ");

    press(&mut app, GuildAction::Invite);
    let invites = sent::<GuildInviteRequested>(&app);
    assert_eq!(
        (invites[0].target_char_id, invites[0].target_name.as_str()),
        (0, "Odin")
    );
    assert_eq!(ui(&app).pending.as_ref().unwrap().action, "invite");

    press(&mut app, GuildAction::Leave);
    assert!(sent::<GuildLeaveRequested>(&app).is_empty());
    assert_eq!(
        ui(&app).feedback.as_deref(),
        Some("A guild action is already pending.")
    );

    answer(&mut app, "notice_edit", GuildErrorKind::None);
    assert!(ui(&app).pending.is_some(), "another action's result");

    answer(&mut app, "invite", GuildErrorKind::TargetOffline);
    assert!(ui(&app).pending.is_none());
    assert_eq!(ui(&app).feedback.as_deref(), Some("Target is offline"));
    assert!(filled(&mut app, GuildHost::Toast));
}

#[test]
fn an_empty_invite_sends_nothing() {
    let mut app = guilded_as(ME);
    typed::<GuildInviteNameField>(&mut app, "   ");
    press(&mut app, GuildAction::Invite);
    assert!(sent::<GuildInviteRequested>(&app).is_empty());
    assert!(ui(&app).pending.is_none());
    assert_eq!(
        ui(&app).feedback.as_deref(),
        Some("Enter a character name.")
    );
}

#[test]
fn the_toast_clears_itself() {
    let mut app = guilded_as(ME);
    press(&mut app, GuildAction::Storage);
    app.update();
    assert!(filled(&mut app, GuildHost::Toast));
    assert!(has_text(&mut app, "Guild storage opens at a Kafra"));

    pass(&mut app, FEEDBACK_DISMISS + Duration::from_secs(1));
    assert!(ui(&app).feedback.is_none());
    assert!(!filled(&mut app, GuildHost::Toast));
}

#[test]
fn expelling_asks_first_and_needs_a_reason_and_the_position() {
    let mut app = guilded_as(FREYJA);
    press_and_settle(&mut app, GuildAction::Tab(GuildTab::Members));
    press_and_settle(&mut app, GuildAction::Select(ORM));
    assert!(button(&mut app, GuildAction::AskExpel));

    press_and_settle(&mut app, GuildAction::AskExpel);
    assert!(has_text(&mut app, "Expel Orm?"));
    assert!(!button(&mut app, GuildAction::Whisper(ORM)));

    press(&mut app, GuildAction::Expel(ORM));
    assert!(sent::<GuildExpelRequested>(&app).is_empty());
    assert_eq!(
        ui(&app).feedback.as_deref(),
        Some("Enter a reason before expelling a member.")
    );

    let world = app.world_mut();
    let mut field = world
        .query_filtered::<&mut EditableText, With<ExpelReasonField>>()
        .single_mut(world)
        .unwrap();
    *field = EditableText::new(" Inactive ");
    press(&mut app, GuildAction::Expel(ASTRID));
    assert!(
        sent::<GuildExpelRequested>(&app).is_empty(),
        "never the master"
    );
    press_and_settle(&mut app, GuildAction::Expel(ORM));
    let expelled = sent::<GuildExpelRequested>(&app);
    assert_eq!(
        (expelled[0].target_char_id, expelled[0].reason.as_str()),
        (ORM, "Inactive")
    );
    assert!(has_text(&mut app, "Select a member"));

    let mut app = guilded_as(ME);
    typed::<ExpelReasonField>(&mut app, "Inactive");
    press(&mut app, GuildAction::Expel(ORM));
    assert!(
        sent::<GuildExpelRequested>(&app).is_empty(),
        "a Vanguard may not"
    );
}

#[test]
fn leaving_asks_first() {
    let mut app = guilded_as(ME);
    press_and_settle(&mut app, GuildAction::AskLeave);
    assert!(has_text(
        &mut app,
        "Leave Wardens? You\u{2019}ll need a new invite to come back."
    ));
    assert!(button(&mut app, GuildAction::Leave) && !button(&mut app, GuildAction::AskLeave));

    press_and_settle(&mut app, GuildAction::Stay);
    assert!(button(&mut app, GuildAction::AskLeave));
    assert!(sent::<GuildLeaveRequested>(&app).is_empty());

    press(&mut app, GuildAction::Leave);
    assert_eq!(sent::<GuildLeaveRequested>(&app).len(), 1);
    assert_eq!(ui(&app).pending.as_ref().unwrap().action, "leave");

    let prompt = seen_by(&app, ASTRID, |guild| {
        view::footer(guild, &GuildUi::default())
    });
    assert_eq!(prompt.prompt, "Leaving disbands Wardens.");
    let own = seen_by(&app, ME, |guild| view::footer(guild, &GuildUi::default()));
    assert_eq!((own.tax, own.contributed.as_str()), (5, "1,284,300"));
}

#[test]
fn the_positions_table_shows_what_each_viewer_may_see_and_edit() {
    let app = guilded_as(ME);
    let table = |me: u32| {
        seen_by(&app, me, |guild| {
            view::positions(guild, &GuildUi::default())
        })
    };

    let member = table(ME);
    assert!(!member.can_edit);
    let slots: Vec<_> = member.rows.iter().map(|row| row.index).collect();
    assert_eq!(slots, [0, 1, 2, 19], "the unnamed, unheld slot is left out");
    let master_row = &member.rows[0];
    assert!(master_row.protected && master_row.perms == [true; 5]);
    let mine = &member.rows[2];
    assert!(mine.mine && mine.count == 1 && mine.tax == 5);
    assert_eq!(mine.perms, [true, false, false, false, false]);

    let master = table(ASTRID);
    assert!(master.can_edit);
    assert_eq!(master.rows.len(), 5, "the master names the blank slots");
    assert_eq!(master.rows[3].name, "Position 4");
}

#[test]
fn the_master_flips_one_permission_at_a_time() {
    let mut app = guilded_as(ASTRID);
    press(&mut app, GuildAction::TogglePerm(2, Perm::Notice));
    let edits = sent::<GuildPositionEditRequested>(&app);
    assert_eq!(edits.len(), 1);
    let edit = &edits[0];
    assert_eq!((edit.index, edit.name.as_str()), (2, "Vanguard"));
    assert!(
        edit.can_invite && !edit.can_expel,
        "kept as the server has them"
    );
    assert_eq!(edit.can_notice, Some(true));
    assert_eq!(
        (edit.can_storage, edit.can_relations, edit.tax),
        (None, None, None)
    );

    answer(&mut app, "position_edit", GuildErrorKind::None);
    press(&mut app, GuildAction::TogglePerm(2, Perm::Invite));
    assert!(!sent::<GuildPositionEditRequested>(&app)[1].can_invite);

    answer(&mut app, "position_edit", GuildErrorKind::None);
    press(&mut app, GuildAction::TogglePerm(3, Perm::Invite));
    press(&mut app, GuildAction::TogglePerm(0, Perm::Storage));
    assert_eq!(
        sent::<GuildPositionEditRequested>(&app).len(),
        2,
        "not an unnamed slot, never the master's own"
    );

    let mut app = guilded_as(FREYJA);
    press(&mut app, GuildAction::TogglePerm(2, Perm::Notice));
    assert!(sent::<GuildPositionEditRequested>(&app).is_empty());
}

#[test]
fn saving_a_position_renames_it_and_sends_a_changed_tax() {
    let mut app = guilded_as(ASTRID);
    press_and_settle(&mut app, GuildAction::Tab(GuildTab::Positions));
    assert!(!filled(&mut app, GuildHost::PositionEdit));
    press_and_settle(&mut app, GuildAction::SelectPosition(2));
    assert!(filled(&mut app, GuildHost::PositionEdit));
    let set = |app: &mut App, name: &str, tax: &str| {
        let world = app.world_mut();
        *world
            .query_filtered::<&mut EditableText, With<PositionNameField>>()
            .single_mut(world)
            .unwrap() = EditableText::new(name);
        *world
            .query_filtered::<&mut EditableText, With<PositionTaxField>>()
            .single_mut(world)
            .unwrap() = EditableText::new(tax);
    };

    set(&mut app, "Shieldbearer", "101");
    press(&mut app, GuildAction::SavePosition(2));
    assert!(sent::<GuildPositionEditRequested>(&app).is_empty());
    assert_eq!(
        ui(&app).feedback.as_deref(),
        Some("EXP tax must be a whole percentage from 0 to 100.")
    );

    set(&mut app, "  ", "5");
    press(&mut app, GuildAction::SavePosition(2));
    assert_eq!(ui(&app).feedback.as_deref(), Some("Enter a position name."));

    set(&mut app, " Shieldbearer ", "5");
    press(&mut app, GuildAction::SavePosition(2));
    let edit = &sent::<GuildPositionEditRequested>(&app)[0];
    assert_eq!((edit.name.as_str(), edit.tax), ("Shieldbearer", None));
    assert!(edit.can_invite && edit.can_notice.is_none());

    answer(&mut app, "position_edit", GuildErrorKind::None);
    set(&mut app, "Shieldbearer", "15");
    press(&mut app, GuildAction::SavePosition(2));
    assert_eq!(sent::<GuildPositionEditRequested>(&app)[1].tax, Some(15));

    press_and_settle(&mut app, GuildAction::SelectPosition(2));
    assert!(
        !filled(&mut app, GuildHost::PositionEdit),
        "a second click closes it"
    );
}

#[test]
fn only_the_master_moves_members_between_positions() {
    let mut app = guilded_as(ASTRID);
    press(&mut app, GuildAction::Assign(ORM, 0));
    press(&mut app, GuildAction::Assign(ASTRID, 2));
    assert!(sent::<GuildMemberPositionRequested>(&app).is_empty());

    press(&mut app, GuildAction::Assign(ORM, 2));
    let moved = sent::<GuildMemberPositionRequested>(&app);
    assert_eq!((moved[0].target_char_id, moved[0].index), (ORM, 2));

    let mut app = guilded_as(FREYJA);
    press(&mut app, GuildAction::Assign(ORM, 2));
    assert!(sent::<GuildMemberPositionRequested>(&app).is_empty());
}

#[test]
fn the_notice_is_edited_by_those_whose_position_may() {
    let mut app = guilded_as(FREYJA);
    assert!(button(&mut app, GuildAction::EditNotice));
    press_and_settle(&mut app, GuildAction::EditNotice);
    assert!(button(&mut app, GuildAction::SaveNotice));
    let world = app.world_mut();
    *world
        .query_filtered::<&mut EditableText, With<NoticeSubjectField>>()
        .single_mut(world)
        .unwrap() = EditableText::new("Muster");
    press_and_settle(&mut app, GuildAction::SaveNotice);
    let notice = &sent::<GuildNoticeEditRequested>(&app)[0];
    assert_eq!(
        (notice.subject.as_str(), notice.body.as_str()),
        ("Muster", "Bring potions.")
    );
    assert!(
        button(&mut app, GuildAction::EditNotice),
        "back to the notice"
    );

    let view = |me: u32| seen_by(&app, me, |guild| view::notice(guild, &GuildUi::default()));
    assert!(view(FREYJA).can_edit && view(ASTRID).can_edit && !view(ME).can_edit);
    assert_eq!(view(ME).by, "Posted by Astrid \u{b7} 2 d ago");

    let mut app = guilded_as(ME);
    typed::<NoticeSubjectField>(&mut app, "Mine");
    press(&mut app, GuildAction::SaveNotice);
    assert!(sent::<GuildNoticeEditRequested>(&app).is_empty());
}

#[test]
fn guild_skills_are_raised_by_the_master_while_points_remain() {
    let mut app = guilded_as(ASTRID);
    let cards = seen_by(&app, ASTRID, view::skills).cards;
    assert_eq!(cards[0].level, "1 / 1");
    assert!(!cards[0].can_upgrade && cards[1].can_upgrade);
    assert_eq!(cards[1].name, "Skill 10004", "no catalog loaded");
    assert!(
        seen_by(&app, ME, view::skills)
            .cards
            .iter()
            .all(|card| !card.can_upgrade)
    );

    press(&mut app, GuildAction::SkillUp(10_000));
    assert!(sent::<GuildSkillUpRequested>(&app).is_empty(), "maxed");
    press(&mut app, GuildAction::SkillUp(10_004));
    assert_eq!(sent::<GuildSkillUpRequested>(&app)[0].skill_id, 10_004);

    let mut app = guilded_as(ME);
    press(&mut app, GuildAction::SkillUp(10_004));
    assert!(sent::<GuildSkillUpRequested>(&app).is_empty());
}

#[test]
fn relations_are_managed_by_those_whose_position_may() {
    let mut app = guilded_as(ASTRID);
    let relations = seen_by(&app, ASTRID, view::relations);
    assert!(relations.can_manage);
    let ally = &relations.allies[0];
    assert_eq!(ally.meta, "Lv 14 \u{b7} 47 members \u{b7} Master Hrafn");
    assert_eq!(ally.castle.as_deref(), Some("Roost"));
    assert_eq!(relations.antagonists[0].castle, None);
    assert!(!seen_by(&app, FREYJA, view::relations).can_manage);

    typed::<GuildAllianceNameField>(&mut app, " Hrafn ");
    press(&mut app, GuildAction::ProposeAlliance);
    assert_eq!(sent::<GuildAllianceRequested>(&app)[0].target_name, "Hrafn");

    answer(&mut app, "alliance_request", GuildErrorKind::None);
    press(&mut app, GuildAction::RemoveAntagonist(8));
    assert!(
        sent::<GuildAntagonistRemoveRequested>(&app).is_empty(),
        "Ravens are allies"
    );
    press(&mut app, GuildAction::RemoveAntagonist(9));
    assert_eq!(sent::<GuildAntagonistRemoveRequested>(&app)[0].guild_id, 9);

    answer(&mut app, "antagonist_remove", GuildErrorKind::None);
    press(&mut app, GuildAction::BreakAlliance(8));
    assert!(
        app.world()
            .resource::<relation_dialogs::PendingRelationConfirmation>()
            .is_pending(),
        "ending an alliance is confirmed first"
    );

    let mut app = guilded_as(FREYJA);
    typed::<GuildAllianceNameField>(&mut app, "Hrafn");
    press(&mut app, GuildAction::ProposeAlliance);
    press(&mut app, GuildAction::RemoveAntagonist(9));
    assert!(sent::<GuildAllianceRequested>(&app).is_empty());
    assert!(sent::<GuildAntagonistRemoveRequested>(&app).is_empty());
}

#[test]
fn whisper_and_party_invite_name_the_member() {
    let mut app = guilded_as(ME);
    press(&mut app, GuildAction::Whisper(FREYJA));
    assert_eq!(
        ui(&app).feedback.as_deref(),
        Some("Chat set to whisper Freyja")
    );
    press(&mut app, GuildAction::PartyInvite(FREYJA));
    assert_eq!(sent::<PartyInviteRequested>(&app)[0].target_char_id, FREYJA);
}

#[test]
fn the_overview_sums_the_guild_up() {
    let app = guilded_as(ME);
    let summary = seen_by(&app, ME, view::summary);
    assert_eq!((summary.exp, summary.exp_label.as_str()), (64.0, "64%"));
    assert!(!summary.can_upload && seen_by(&app, ASTRID, view::summary).can_upload);
    let stats: Vec<_> = summary
        .stats
        .iter()
        .map(|(label, value, _)| (*label, value.as_str()))
        .collect();
    assert_eq!(
        stats,
        [
            ("Members", "4 / 22"),
            ("Online now", "3"),
            ("Average level", "202"),
            ("Castle", "Bastion"),
            ("Allies", "1"),
            ("Antagonists", "1"),
        ]
    );

    let tabs = seen_by(&app, ME, |guild| view::tabs(guild, &GuildUi::default()));
    let notes: Vec<_> = tabs.iter().map(|tab| tab.note.as_str()).collect();
    assert_eq!(notes, ["", "3 online", "4", "2 pts", "2"]);
    assert!(tabs[0].on && !tabs[1].on);
}

#[test]
fn activity_reads_as_a_sentence_with_its_age() {
    let entry = |kind, subject: &str, actor: &str, detail: &str, level| GuildActivityInfo {
        kind,
        subject: subject.into(),
        actor: actor.into(),
        detail: detail.into(),
        skill_id: 10_003,
        level,
        at: NOW - 90,
    };
    let mut app = app_as(ME);
    let mut info = wardens();
    info.activity = vec![
        entry(GuildActivityKind::Created, "", "Astrid", "", 0),
        entry(GuildActivityKind::Left, "Vali", "", "", 0),
        entry(GuildActivityKind::Expelled, "Loki", "Freyja", "", 0),
        entry(
            GuildActivityKind::PositionChanged,
            "Torvald",
            "Astrid",
            "Vanguard",
            0,
        ),
        entry(GuildActivityKind::SkillLearned, "", "Astrid", "", 3),
        entry(GuildActivityKind::LevelUp, "", "", "", 13),
        entry(GuildActivityKind::AllianceFormed, "Ravens", "", "", 0),
        entry(GuildActivityKind::CastleTaken, "Bastion", "", "", 0),
        entry(GuildActivityKind::Unknown(99), "", "", "", 0),
    ];
    ingress(&mut app, GuildIngressPayload::Info(info));

    let lines = seen_by(&app, ME, view::activity);
    let texts: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Astrid founded the guild",
            "Vali left the guild",
            "Loki was expelled \u{b7} By Freyja",
            "Torvald was made Vanguard",
            "Astrid learned Skill 10003 Lv 3",
            "The guild reached Lv 13",
            "Alliance formed with Ravens",
            "The guild took Bastion",
        ],
        "an unknown kind is left out"
    );
    assert!(lines.iter().all(|line| line.when == "1 min ago"));
}

#[test]
fn ago_steps_from_minutes_to_weeks() {
    let cases = [
        (30, "just now"),
        (59 * 60, "59 min ago"),
        (3 * 3600, "3 h ago"),
        (2 * 86_400, "2 d ago"),
        (15 * 86_400, "2 wk ago"),
    ];
    for (age, expected) in cases {
        assert_eq!(view::ago(NOW, NOW - age), expected);
    }
    assert_eq!(
        view::ago(NOW, NOW + 500),
        "just now",
        "a clock ahead of ours"
    );
}

fn invitation() -> GuildInviteInfo {
    GuildInviteInfo {
        guild_id: 7,
        guild_name: "Wardens".into(),
        inviter_name: "Freyja".into(),
        inviter_char_id: FREYJA,
        guild: wardens(),
    }
}

#[test]
fn the_invitation_shows_the_guild_one_would_join() {
    let view = view::invite(&invitation(), None);
    assert_eq!(view.about, "Base Lv 203 \u{b7} Officer of the guild");
    assert_eq!((view.level, view.master.as_str()), (12, "Astrid"));
    assert_eq!(
        (view.members.as_str(), view.online.as_str()),
        ("4 / 22", "3")
    );
    assert_eq!(
        (view.join_as.as_str(), view.tax.as_str()),
        ("Recruit", "10%")
    );
    assert_eq!(
        (view.allies.as_str(), view.antagonists.as_str()),
        ("Ravens", "Ash")
    );
    assert_eq!(view.castle.as_deref(), Some("Bastion"));

    let bare = view::invite(
        &GuildInviteInfo {
            guild_id: 7,
            guild_name: "Wardens".into(),
            inviter_name: "Freyja".into(),
            ..default()
        },
        None,
    );
    assert!(bare.job.is_none() && bare.about.is_empty());
    assert_eq!(bare.allies, "None");
}

#[test]
fn an_accepted_invitation_becomes_the_welcome() {
    let mut app = app_as(ME);
    ingress(&mut app, GuildIngressPayload::InviteNotified(invitation()));
    assert!(filled(&mut app, GuildHost::Invite));
    assert!(has_text(&mut app, "Join Guild") && has_text(&mut app, "Recruit"));

    press(&mut app, GuildAction::AnswerInvite(true));
    app.update();
    let answers = sent::<GuildInviteResponded>(&app);
    assert_eq!((answers[0].guild_id, answers[0].accept), (7, true));
    assert!(
        !filled(&mut app, GuildHost::Invite),
        "until the guild arrives"
    );

    ingress(&mut app, GuildIngressPayload::Info(wardens()));
    app.update();
    assert!(has_text(&mut app, "Welcome to Wardens."));
    assert_eq!(window(&mut app), Visibility::Hidden);

    press(&mut app, GuildAction::OpenGuild);
    app.update();
    assert_eq!(window(&mut app), Visibility::Visible);
    assert!(!filled(&mut app, GuildHost::Invite));
}

#[test]
fn a_declined_or_expired_invitation_goes_away() {
    let mut app = app_as(ME);
    ingress(&mut app, GuildIngressPayload::InviteNotified(invitation()));
    press(&mut app, GuildAction::AnswerInvite(false));
    app.update();
    assert!(!sent::<GuildInviteResponded>(&app)[0].accept);
    assert!(!filled(&mut app, GuildHost::Invite));
    press(&mut app, GuildAction::AnswerInvite(true));
    assert_eq!(
        sent::<GuildInviteResponded>(&app).len(),
        1,
        "nothing pending"
    );

    ingress(&mut app, GuildIngressPayload::InviteNotified(invitation()));
    pass(&mut app, Duration::from_secs(31));
    assert!(!app.world().resource::<PendingGuildInvite>().is_pending());

    app.world_mut().write_message(GuildIngress {
        generation: ZoneSessionGeneration(8),
        payload: GuildIngressPayload::InviteNotified(invitation()),
    });
    app.update();
    assert!(
        !app.world().resource::<PendingGuildInvite>().is_pending(),
        "an earlier session's invitation"
    );
}

#[test]
fn switching_character_starts_the_ui_over() {
    let mut app = guilded_as(ME);
    press(&mut app, GuildAction::Tab(GuildTab::Relations));
    typed::<GuildInviteNameField>(&mut app, "Odin");
    press(&mut app, GuildAction::Invite);
    assert!(ui(&app).pending.is_some());

    app.world_mut().resource_mut::<ZoneSession>().char_id = FREYJA;
    app.update();
    assert_eq!(*ui(&app), GuildUi::default());
    assert_eq!(window(&mut app), Visibility::Hidden);
    assert!(app.world().resource::<GuildUiSession>().blocked);

    ingress(&mut app, GuildIngressPayload::InviteNotified(invitation()));
    assert!(
        !app.world().resource::<PendingGuildInvite>().is_pending(),
        "the old session is over"
    );
}

fn toggle_app(visibility: Visibility) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<GameState>();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();
    app.init_resource::<GuildState>();
    app.init_resource::<InputFocus>();
    app.init_resource::<UiFocus>();
    app.add_plugins(crate::focus::UiFocusMirrorPlugin);
    let field = app
        .world_mut()
        .spawn((GuildInviteNameField, EditableText::new("")))
        .id();
    app.world_mut().spawn((GuildWindowRoot, visibility));
    let player = app
        .world_mut()
        .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
        .id();
    app.add_systems(
        Update,
        (toggle_guild_window, release_hidden_guild_focus)
            .chain()
            .run_if(in_state(GameState::InGame)),
    );
    (app, field, player)
}

fn press_guild(app: &mut App, player: Entity) {
    app.world_mut()
        .entity_mut(player)
        .get_mut::<ActionState<PlayerAction>>()
        .unwrap()
        .press(&PlayerAction::Guild);
}

#[test]
fn the_hotkey_keeps_the_window_closed_while_guildless() {
    let (mut app, _, player) = toggle_app(Visibility::Hidden);
    press_guild(&mut app, player);
    app.update();
    assert_eq!(window(&mut app), Visibility::Hidden);
    assert!(!app.world().resource::<UiFocus>().text_input_active);
}

#[test]
fn closing_the_window_releases_the_focus_of_its_fields() {
    let (mut app, field, player) = toggle_app(Visibility::Visible);
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    press_guild(&mut app, player);
    app.update();
    assert_eq!(window(&mut app), Visibility::Hidden);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);

    let elsewhere = app.world_mut().spawn(EditableText::new("")).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, FocusCause::Navigated);
    app.update();
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(elsewhere),
        "another widget's field keeps its focus"
    );
}

#[test]
fn submit_name_trims_and_caps() {
    assert_eq!(submit_name("  Wardens  "), Some("Wardens".to_string()));
    assert_eq!(submit_name("   "), None);
    assert_eq!(submit_name(&"x".repeat(40)).unwrap().len(), NAME_MAX_CHARS);
}
