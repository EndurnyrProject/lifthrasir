use std::time::SystemTime;

use bevy::scene::ScenePlugin;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{ChatRoomMember, ChatRoomSettings, ChatRoomSign};

use super::view::{LineView, SignTone};
use super::*;

const ME: u32 = 42;
const ROOM: u32 = 9;
const FREYJA: u32 = 5;

fn member(char_id: u32, name: &str) -> ChatRoomMember {
    ChatRoomMember {
        char_id,
        name: name.into(),
    }
}

fn sign(owner_gid: u32) -> ChatRoomSign {
    ChatRoomSign {
        room_id: ROOM,
        title: "Glast Heim run".into(),
        member_count: 2,
        seats: 6,
        public: true,
        owner_gid,
        owner_is_npc: false,
    }
}

/// An app holding the chat room UI under a bare HUD root, as the local player [`ME`].
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<ChatRooms>()
        .init_resource::<ChatRoomUi>()
        .init_resource::<EntityRegistry>()
        .insert_resource(ZoneSession {
            char_id: ME,
            ..default()
        })
        .add_message::<ChatRoomEvent>()
        .add_message::<ChatRoomCommand>()
        .add_message::<ChatNotice>()
        .add_systems(
            Update,
            (
                follow_rooms,
                sync_card,
                mirror_form,
                rebuild_chat_room_ui,
                show_window,
                show_password_row,
            )
                .chain(),
        );
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_chat_room(&mut commands, root);
    app.world_mut().flush();
    app
}

/// Replays `events` into the domain and hands them to the UI, as one frame would.
fn push(app: &mut App, events: impl IntoIterator<Item = ChatRoomEvent>) {
    for event in events {
        app.world_mut().resource_mut::<ChatRooms>().apply(
            &event,
            &member(ME, "Ygor"),
            SystemTime::UNIX_EPOCH,
        );
        app.world_mut().write_message(event);
    }
    app.update();
}

/// Sits [`ME`] in Freyja's room, or in their own when `own`.
fn join(app: &mut App, own: bool) {
    let owner = if own { ME } else { FREYJA };
    let mut members = vec![member(FREYJA, "Freyja"), member(ME, "Ygor")];
    if own {
        members.reverse();
    }
    push(
        app,
        [
            ChatRoomEvent::SignShown(sign(owner)),
            ChatRoomEvent::Joined {
                room_id: ROOM,
                owner_gid: owner,
                members,
            },
        ],
    );
}

fn filled(app: &mut App, host: ChatRoomHost) -> bool {
    let world = app.world_mut();
    world
        .query::<(&ChatRoomHost, Option<&Children>)>()
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

fn has_action(app: &mut App, action: ChatRoomAction) -> bool {
    let world = app.world_mut();
    world
        .query::<&ChatRoomAction>()
        .iter(world)
        .any(|found| *found == action)
}

fn press(app: &mut App, action: ChatRoomAction) {
    let button = app.world_mut().spawn(action).observe(on_room_button).id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
}

fn type_in(app: &mut App, which: RoomField, text: &str) {
    let world = app.world_mut();
    let mut fields = world.query::<(&RoomField, &mut EditableText)>();
    let (_, mut field) = fields
        .iter_mut(world)
        .find(|(field, _)| **field == which)
        .unwrap_or_else(|| panic!("no {which:?} field on screen"));
    crate::screens::shared::set_field_text(&mut field, text);
    app.update();
}

fn sent(app: &App) -> Vec<ChatRoomCommand> {
    let messages = app.world().resource::<Messages<ChatRoomCommand>>();
    messages.get_cursor().read(messages).cloned().collect()
}

fn window_shown(app: &mut App) -> bool {
    let world = app.world_mut();
    let mut window = world.query_filtered::<&Visibility, With<ChatRoomWindow>>();
    *window.single(world).expect("one window") != Visibility::Hidden
}

#[test]
fn the_window_shows_exactly_while_in_a_room() {
    let mut app = app();
    app.update();
    assert!(!window_shown(&mut app));
    for host in [ChatRoomHost::Head, ChatRoomHost::Log, ChatRoomHost::Side] {
        assert!(!filled(&mut app, host), "{host:?} outside a room");
    }

    join(&mut app, false);
    assert!(window_shown(&mut app));
    assert!(has_text(&mut app, "Glast Heim run") && has_text(&mut app, "2 / 6"));
    assert!(has_text(&mut app, "Ygor (you)") && has_text(&mut app, "Leave Room"));

    press(&mut app, ChatRoomAction::Leave);
    assert_eq!(sent(&app), [ChatRoomCommand::Leave]);
    // The domain takes the player out when it sees the command.
    app.world_mut().resource_mut::<ChatRooms>().leave();
    app.update();
    assert!(!window_shown(&mut app));
}

#[test]
fn only_the_owner_manages_the_room() {
    let mut app = app();
    join(&mut app, false);
    assert!(!has_action(&mut app, ChatRoomAction::ToggleSettings));
    assert!(!has_action(&mut app, ChatRoomAction::Kick(FREYJA)));

    let mut app = self::app();
    join(&mut app, true);
    assert!(has_action(&mut app, ChatRoomAction::ToggleSettings));
    assert!(has_action(&mut app, ChatRoomAction::Kick(FREYJA)));
    assert!(
        !has_action(&mut app, ChatRoomAction::Kick(ME)),
        "nobody kicks themselves"
    );
    assert!(has_text(&mut app, "Leave \u{b7} Freyja takes over"));

    press(&mut app, ChatRoomAction::Kick(FREYJA));
    press(&mut app, ChatRoomAction::MakeOwner(FREYJA));
    assert_eq!(
        sent(&app),
        [
            ChatRoomCommand::Kick("Freyja".into()),
            ChatRoomCommand::MakeOwner("Freyja".into()),
        ]
    );
}

#[test]
fn a_said_line_is_trimmed_sent_and_cleared() {
    let mut app = app();
    join(&mut app, false);
    type_in(&mut app, RoomField::Message, "   ");
    press(&mut app, ChatRoomAction::Send);
    assert!(sent(&app).is_empty(), "a blank line says nothing");

    type_in(&mut app, RoomField::Message, "  on my way ");
    press(&mut app, ChatRoomAction::Send);
    assert_eq!(sent(&app), [ChatRoomCommand::Say("on my way".into())]);
    let world = app.world_mut();
    let mut fields = world.query::<(&RoomField, &EditableText)>();
    let (_, draft) = fields.single(world).expect("only the message field");
    assert!(draft.value().to_string().is_empty());
}

#[test]
fn a_new_room_opens_once_it_has_a_title_and_any_password_it_asks_for() {
    let mut app = app();
    app.world_mut().resource_mut::<ChatRoomUi>().creating = true;
    app.update();
    app.update();
    assert!(filled(&mut app, ChatRoomHost::Dialog));
    assert!(
        filled(&mut app, ChatRoomHost::FormOptions) && filled(&mut app, ChatRoomHost::FormFoot)
    );
    assert!(has_text(&mut app, "Your room title") && has_text(&mut app, "1/6"));

    press(&mut app, ChatRoomAction::SubmitForm);
    assert!(sent(&app).is_empty(), "a room needs a title");

    type_in(&mut app, RoomField::Title, " Wolfpack ");
    assert!(
        has_text(&mut app, "10 / 36"),
        "the count is of what is typed"
    );
    press(&mut app, ChatRoomAction::Access(true));
    press(&mut app, ChatRoomAction::Seats(-1));
    app.update();
    assert!(has_text(&mut app, "Wolfpack") && has_text(&mut app, "1/5"));
    press(&mut app, ChatRoomAction::SubmitForm);
    assert!(sent(&app).is_empty(), "a locked room needs its password");

    type_in(&mut app, RoomField::Password, "wolf");
    press(&mut app, ChatRoomAction::SubmitForm);
    assert_eq!(
        sent(&app),
        [ChatRoomCommand::Create(ChatRoomSettings {
            title: "Wolfpack".into(),
            password: "wolf".into(),
            seats: 5,
            public: false,
        })]
    );
    app.update();
    assert!(!filled(&mut app, ChatRoomHost::Dialog), "the dialog closes");
}

#[test]
fn the_owner_edits_the_room_and_typing_survives_people_coming_in() {
    let mut app = app();
    join(&mut app, true);
    press(&mut app, ChatRoomAction::ToggleSettings);
    app.update();
    app.update();
    assert!(has_action(&mut app, ChatRoomAction::SubmitForm));
    assert!(!has_text(&mut app, "PEOPLE"), "the panel replaces the list");

    type_in(&mut app, RoomField::Title, "Crypts tonight");
    push(
        &mut app,
        [ChatRoomEvent::MemberJoined {
            room_id: ROOM,
            member: member(7, "Orm"),
        }],
    );
    let world = app.world_mut();
    let mut fields = world.query::<(&RoomField, &EditableText)>();
    let typed = fields
        .iter(world)
        .find(|(field, _)| **field == RoomField::Title)
        .map(|(_, text)| text.value().to_string());
    assert_eq!(typed.as_deref(), Some("Crypts tonight"));

    // Three people sit in the room, so it cannot shrink below three seats.
    for _ in 0..6 {
        press(&mut app, ChatRoomAction::Seats(-1));
    }
    press(&mut app, ChatRoomAction::SubmitForm);
    assert_eq!(
        sent(&app),
        [ChatRoomCommand::ChangeSettings(ChatRoomSettings {
            title: "Crypts tonight".into(),
            password: String::new(),
            seats: 3,
            public: true,
        })]
    );
}

#[test]
fn a_card_joins_with_the_typed_password_and_shows_the_refusal() {
    let mut app = app();
    let owner = app.world_mut().spawn(GlobalTransform::default()).id();
    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .register_entity(FREYJA, owner);
    let locked = ChatRoomSign {
        public: false,
        ..sign(FREYJA)
    };
    push(&mut app, [ChatRoomEvent::SignShown(locked)]);
    app.world_mut()
        .resource_mut::<ChatRoomUi>()
        .toggle_card(ROOM);
    app.update();
    app.update();
    assert!(filled(&mut app, ChatRoomHost::CardHead) && filled(&mut app, ChatRoomHost::CardFoot));
    assert!(has_text(&mut app, "Password room") && has_text(&mut app, "Join"));

    type_in(&mut app, RoomField::CardPassword, "wolf");
    press(&mut app, ChatRoomAction::Join);
    assert_eq!(
        sent(&app),
        [ChatRoomCommand::Join {
            room_id: ROOM,
            password: "wolf".into(),
        }]
    );

    push(
        &mut app,
        [ChatRoomEvent::JoinRefused {
            room_id: ROOM,
            reason: ChatRoomJoinError::WrongPassword,
        }],
    );
    assert!(has_text(&mut app, "That\u{2019}s not the password"));

    // The room going away takes its card along.
    push(&mut app, [ChatRoomEvent::SignRemoved { room_id: ROOM }]);
    app.update();
    let world = app.world_mut();
    assert_eq!(world.query::<&RoomCard>().iter(world).count(), 0);
}

#[test]
fn refusals_without_a_card_are_told_in_the_chat() {
    let mut app = app();
    join(&mut app, false);
    push(
        &mut app,
        [
            ChatRoomEvent::CreateRefused,
            ChatRoomEvent::MemberLeft {
                room_id: ROOM,
                member: member(ME, "Ygor"),
                kicked: true,
            },
        ],
    );
    let notices = app.world().resource::<Messages<ChatNotice>>();
    let told: Vec<_> = notices.get_cursor().read(notices).cloned().collect();
    assert_eq!(told.len(), 2);
    assert!(told.iter().all(|notice| notice.error));
    assert!(!window_shown(&mut app));
}

#[test]
fn a_sign_is_ringed_by_what_the_room_is() {
    let tone = |room: ChatRoomSign, own| view::sign(&room, own).tone;
    assert_eq!(tone(sign(FREYJA), false), SignTone::Open);
    assert_eq!(tone(sign(ME), true), SignTone::Own);
    let full = ChatRoomSign {
        member_count: 6,
        ..sign(FREYJA)
    };
    assert_eq!(tone(full.clone(), false), SignTone::Full);
    let locked = ChatRoomSign {
        public: false,
        ..full
    };
    assert_eq!(tone(locked.clone(), false), SignTone::Private);
    let npc = ChatRoomSign {
        owner_is_npc: true,
        ..locked
    };
    assert_eq!(tone(npc, false), SignTone::Npc);
}

#[test]
fn a_card_says_why_a_room_cannot_be_joined() {
    let open = view::card_foot(&sign(FREYJA), None, false);
    assert_eq!((open.join, open.enabled, open.note), ("Join", true, None));

    let full = ChatRoomSign {
        member_count: 6,
        ..sign(FREYJA)
    };
    let full = view::card_foot(&full, None, false);
    assert_eq!((full.join, full.enabled), ("Full", false));
    assert!(
        full.note
            .is_some_and(|(_, tone)| tone == ro_dark::TONE_ERROR)
    );

    let seated = view::card_foot(&sign(FREYJA), None, true);
    assert!(!seated.enabled && seated.note.is_some());

    let npc = ChatRoomSign {
        owner_is_npc: true,
        ..sign(FREYJA)
    };
    assert_eq!(view::card_foot(&npc, None, false).join, "Take a Place");
    assert_eq!(
        view::card_head(&npc, Some("Arena Guide")).meta,
        "Event waiting room"
    );
    assert_eq!(
        view::card_head(&sign(FREYJA), Some("Freyja")).meta,
        "Open room \u{b7} by Freyja"
    );
}

#[test]
fn the_log_tells_who_spoke_and_what_happened_to_the_room() {
    let mut app = app();
    join(&mut app, false);
    push(
        &mut app,
        [
            ChatRoomEvent::Said {
                room_id: ROOM,
                char_id: FREYJA,
                name: "Freyja".into(),
                text: "chapel first".into(),
            },
            ChatRoomEvent::MemberJoined {
                room_id: ROOM,
                member: member(7, "Orm"),
            },
        ],
    );
    let rooms = app.world().resource::<ChatRooms>();
    let lines = view::log(rooms.joined().expect("in the room"), ME);
    assert!(matches!(
        &lines[0],
        LineView::Said { name, crown: true, me: false, text, .. }
            if name == "Freyja" && text == "chapel first"
    ));
    assert_eq!(
        lines[1],
        LineView::Notice {
            text: "Orm joined".into(),
            tone: ro_dark::TONE_OK,
        }
    );
    assert!(has_text(&mut app, "Orm joined"));
}

#[test]
fn enter_in_the_message_field_says_the_line_and_is_not_left_for_the_chat() {
    let mut app = app();
    app.init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, room_keys.before(follow_rooms));
    join(&mut app, false);
    type_in(&mut app, RoomField::Message, "hello");

    let world = app.world_mut();
    let mut fields = world.query_filtered::<Entity, With<RoomField>>();
    let field = fields.single(world).expect("only the message field");
    world
        .resource_mut::<InputFocus>()
        .set(field, bevy::input_focus::FocusCause::Navigated);
    world
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();

    assert_eq!(sent(&app), [ChatRoomCommand::Say("hello".into())]);
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    assert!(
        !keys.just_pressed(KeyCode::Enter),
        "the chat must not see it"
    );
}

/// System parameter conflicts and ordering cycles only show when the schedule first
/// runs, so run the real plugins once in game, with a room on screen.
#[test]
fn the_plugins_run_in_game_and_the_chat_room_key_opens_the_dialog() {
    use bevy::state::app::StatesPlugin;
    use lifthrasir_client::domain::input::UiFocus;

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        StatesPlugin,
    ));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.insert_state(GameState::InGame)
        .init_resource::<ChatRooms>()
        .init_resource::<EntityRegistry>()
        .init_resource::<ZoneSession>()
        .init_resource::<InputFocus>()
        .init_resource::<UiFocus>()
        .init_resource::<UiScale>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<ChatRoomEvent>()
        .add_message::<ChatRoomCommand>()
        .add_message::<ChatNotice>()
        .add_plugins((
            ChatRoomWidgetPlugin,
            crate::worldspace::chat_room_signs::ChatRoomSignPlugin,
        ));
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_chat_room(&mut commands, root);
    app.world_mut().flush();
    push(&mut app, [ChatRoomEvent::SignShown(sign(FREYJA))]);

    let player = app
        .world_mut()
        .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
        .id();
    let actions = |app: &mut App, press: bool| {
        let mut player = app.world_mut().entity_mut(player);
        let mut actions = player.get_mut::<ActionState<PlayerAction>>().unwrap();
        if press {
            actions.press(&PlayerAction::ChatRoom);
        } else {
            actions.release(&PlayerAction::ChatRoom);
        }
    };
    actions(&mut app, true);
    app.update();
    // No input plugin ends the frame here, so the key is let go by hand.
    actions(&mut app, false);
    app.update();

    assert!(app.world().resource::<ChatRoomUi>().creating);
    assert!(filled(&mut app, ChatRoomHost::Dialog));
}
