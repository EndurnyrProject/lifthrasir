use bevy::ecs::system::RunSystemOnce;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::scene::ScenePlugin;
use bevy::text::EditableText;
use bevy::ui_widgets::TextInput;
use lifthrasir_client::domain::emote::EmoteRequested;
use net_contract::commands::{ChatSent, GmCommandSent, MemoRequested, MountPeco};
use net_contract::dto::{CHAT_LINK_MARK, ChatChannel, NavigationTarget};
use net_contract::events::ZoneInventoryItem;

use super::view::{Outgoing, bind_links, head, matching_commands, popup, route, speech};
use super::*;
use crate::widgets::guild_window::slash::GuildSlashSubmitted;
use crate::widgets::navigation::slash::NaviSlash;
use crate::widgets::party::slash::{PartySlash, PartySlashSubmitted};
use crate::widgets::trade_window::slash::TradeSlashSubmitted;

const ME: &str = "Ygor";

fn heard(channel: ChatChannel, from: &str, text: &str) -> ChatHeard {
    ChatHeard {
        channel,
        sender_gid: 7,
        sender_name: from.into(),
        text: text.into(),
        ..default()
    }
}

fn say(scope: Scope, text: &str) -> Option<Outgoing> {
    Some(Outgoing::Say {
        scope,
        text: text.into(),
    })
}

#[test]
fn a_draft_goes_where_its_prefix_says() {
    let target = Scope::Trade;
    assert_eq!(
        route("  selling leaves ", &target),
        say(Scope::Trade, "selling leaves")
    );
    assert_eq!(route("% pull now", &target), say(Scope::Party, "pull now"));
    assert_eq!(
        route("$war tonight", &target),
        say(Scope::Guild, "war tonight")
    );
    assert_eq!(
        route("@item 501 10", &target),
        Some(Outgoing::Gm("item 501 10".into()))
    );
    assert_eq!(
        route("/navi geffen", &target),
        Some(Outgoing::Slash("/navi geffen".into()))
    );
    for blank in ["", "   ", "%", "$  ", "@"] {
        assert_eq!(route(blank, &target), None, "{blank:?} goes nowhere");
    }
}

#[test]
fn whisper_command_names_its_hero_with_or_without_quotes() {
    let whisper = |name: &str| Scope::Whisper(name.into());
    assert_eq!(
        route("/w Freyja  on my way", &Scope::Local),
        say(whisper("Freyja"), "on my way")
    );
    assert_eq!(
        route("/w \"Lady Amy\" hello", &Scope::Local),
        say(whisper("Lady Amy"), "hello")
    );
    for malformed in ["/w", "/w Freyja", "/w \"Lady Amy hello"] {
        assert!(
            matches!(route(malformed, &Scope::Local), Some(Outgoing::Refused(_))),
            "{malformed:?} is refused, never said aloud"
        );
    }
}

#[test]
fn links_are_bound_in_the_order_they_appear() {
    let pending = [
        ("[Elunium]".to_string(), 4),
        ("[Muffler [1]]".to_string(), 9),
    ];
    let (text, indices) = bind_links("S> [Muffler [1]] and [Elunium]", &pending);
    assert_eq!(text, format!("S> {CHAT_LINK_MARK} and {CHAT_LINK_MARK}"));
    assert_eq!(indices, [9, 4]);
}

#[test]
fn an_edited_label_and_a_typed_mark_stay_plain_text() {
    let pending = [("[Elunium]".to_string(), 4)];
    let (text, indices) = bind_links(&format!("want [Eluniu]{CHAT_LINK_MARK}?"), &pending);
    assert_eq!(text, "want [Eluniu]?");
    assert!(indices.is_empty());
}

#[test]
fn a_line_links_at_most_three_items() {
    let pending: Vec<_> = (0..4).map(|slot| (format!("[i{slot}]"), slot)).collect();
    let (text, indices) = bind_links("[i0][i1][i2][i3]", &pending);
    assert_eq!(indices, [0, 1, 2]);
    assert!(text.ends_with("[i3]"));
}

#[test]
fn someone_elses_party_line_is_tagged_clickable_and_flags_a_mention() {
    let line = speech(
        &heard(ChatChannel::Party, "Bjorn", "Ygor pull, Ygorson waits"),
        ME,
        &[],
    );

    assert_eq!(line.scope, Scope::Party);
    assert!(line.mention && !line.mine);
    let texts: Vec<&str> = line.runs.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(
        texts,
        ["[Party] ", "Bjorn", " : ", "Ygor", " pull, Ygorson waits"]
    );
    assert!(line.runs[0].tag);
    assert_eq!(
        line.runs[1].action,
        Some(RunAction::Whisper("Bjorn".into()))
    );
    assert_eq!(line.runs[3].color, ro_dark::FROST_GLOW);
}

#[test]
fn the_players_own_line_is_not_a_mention_and_its_name_does_nothing() {
    let line = speech(&heard(ChatChannel::Local, ME, "Ygor here"), ME, &[]);
    assert!(line.mine && !line.mention);
    assert!(line.runs.iter().all(|run| run.action.is_none()));
}

#[test]
fn a_whisper_belongs_to_the_other_hero_whichever_way_it_went() {
    let sent = ChatHeard {
        whisper_to: "Freyja".into(),
        ..heard(ChatChannel::Whisper, ME, "leaf?")
    };
    let received = ChatHeard {
        whisper_to: ME.into(),
        ..heard(ChatChannel::Whisper, "Freyja", "yes")
    };
    let sent = speech(&sent, ME, &[]);
    let received = speech(&received, ME, &[]);

    assert_eq!(sent.scope, Scope::Whisper("Freyja".into()));
    assert_eq!(received.scope, sent.scope);
    assert_eq!(sent.runs[0].text, "(To Freyja)");
    assert_eq!(received.runs[0].text, "(From Freyja)");
    assert_eq!(
        sent.runs[0].action,
        Some(RunAction::Whisper("Freyja".into()))
    );
}

#[test]
fn a_link_mark_becomes_a_clickable_item_name() {
    let line = speech(
        &heard(
            ChatChannel::Trade,
            "Hakon",
            &format!("S> {CHAT_LINK_MARK} 40k {CHAT_LINK_MARK}"),
        ),
        ME,
        &[("Muffler [1]".into(), false)],
    );
    let link = line
        .runs
        .iter()
        .find(|run| run.action == Some(RunAction::Link(0)))
        .expect("the first mark is a link");
    assert_eq!(link.text, "[Muffler [1]]");
    // The second mark has no item behind it and draws nothing.
    assert_eq!(line.runs.last().unwrap().text, " 40k ");
}

#[test]
fn a_nameless_speaker_has_no_separator() {
    let line = speech(&heard(ChatChannel::Local, "", "beware"), ME, &[]);
    let texts: Vec<&str> = line.runs.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(texts, ["beware"]);
}

#[test]
fn the_palette_lists_commands_until_an_argument_is_typed() {
    let names = |draft| {
        matching_commands(draft).map(|commands| {
            commands
                .into_iter()
                .map(|command| command.name)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        names("/p"),
        Some(vec!["/pcreate", "/pinvite", "/paccept", "/pdecline"])
    );
    assert_eq!(names("/zz"), Some(vec![]));
    assert_eq!(names("/w Freyja"), None);
    assert_eq!(names("hello"), None);
}

#[test]
fn unseen_lines_count_on_the_tabs_that_are_not_on_show() {
    let mut ui = ChatUi::default();
    let party = speech(&heard(ChatChannel::Party, "Bjorn", "Ygor pull"), ME, &[]);
    for _ in 0..12 {
        ui.hear(&party);
    }
    ui.hear(&speech(&heard(ChatChannel::Local, "Ulf", "hi"), ME, &[]));
    ui.hear(&speech(&heard(ChatChannel::Guild, ME, "mine"), ME, &[]));

    let view = head(&ui);
    let tab = |label: &str| view.tabs.iter().find(|tab| tab.label() == label).unwrap();
    assert_eq!(tab("Party").badge().as_deref(), Some("9+"));
    assert!(tab("Party").unread.ping);
    assert_eq!(
        tab("Guild").badge(),
        None,
        "the player's own line is not unread"
    );
    assert_eq!(tab("All").badge(), None);

    ui.show_tab(Some(Scope::Party));
    assert_eq!(head(&ui).tabs[1].badge(), None);
    assert_eq!(
        ui.target,
        Scope::Party,
        "a channel tab aims the input at it"
    );
    ui.hear(&party);
    assert_eq!(
        head(&ui).tabs[1].badge(),
        None,
        "the shown tab never counts"
    );
}

#[test]
fn a_whisper_opens_a_tab_and_closing_it_falls_back() {
    let mut ui = ChatUi::default();
    ui.hear(&speech(
        &heard(ChatChannel::Whisper, "Freyja", "psst"),
        ME,
        &[],
    ));
    let whisper = Scope::Whisper("Freyja".into());
    assert_eq!(head(&ui).tabs.last().unwrap().scope, Some(whisper.clone()));

    ui.show_tab(Some(whisper.clone()));
    ui.close_whisper("Freyja");
    assert_eq!(ui.tab, None);
    assert_eq!(ui.target, Scope::Local);
    assert_eq!(head(&ui).tabs.len(), 4);
}

#[test]
fn the_channel_menu_wins_over_the_palette() {
    let mut ui = ChatUi::default();
    assert!(matches!(popup(&ui, "/p"), Some(PopupView::Palette { .. })));
    assert_eq!(popup(&ui, "hello"), None);
    ui.menu_open = true;
    let Some(PopupView::Menu(rows)) = popup(&ui, "/p") else {
        panic!("the open menu is shown");
    };
    assert_eq!(rows.len(), 4);
    assert!(rows[0].active, "Local is where a fresh input sends");
}

/// An app with the whole panel under a bare HUD root.
fn panel_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    input_messages(&mut app);
    app.init_resource::<ChatUi>()
        .add_message::<ChatHeard>()
        .add_message::<ShowInfoModal>()
        .add_systems(
            Update,
            (
                ingest_chat,
                rebuild_chat_ui,
                sync_lines,
                follow_log,
                sync_hint,
            )
                .chain(),
        );
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_chat_box(&mut commands, root);
    app.world_mut().flush();
    app
}

fn input_messages(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<InputFocus>()
        .add_message::<ChatSent>()
        .add_message::<GmCommandSent>()
        .add_message::<MemoRequested>()
        .add_message::<ChatNotice>()
        .add_message::<PartySlashSubmitted>()
        .add_message::<TradeSlashSubmitted>()
        .add_message::<GuildSlashSubmitted>()
        .add_message::<EmoteRequested>()
        .add_message::<MountPeco>()
        .add_message::<NaviSlash>();
}

/// The log's lines in order: whether each takes room, and its full text.
fn log_lines(app: &mut App) -> Vec<(bool, String)> {
    let world = app.world_mut();
    let container = world
        .query_filtered::<Entity, With<ChatHistory>>()
        .single(world)
        .unwrap();
    let lines: Vec<Entity> = world
        .get::<Children>(container)
        .into_iter()
        .flatten()
        .copied()
        .collect();
    lines
        .into_iter()
        .map(|line| {
            let mut text = world.get::<Text>(line).unwrap().0.clone();
            for span in world.get::<Children>(line).into_iter().flatten() {
                text.push_str(&world.get::<TextSpan>(*span).unwrap().0);
            }
            (
                world.get::<Node>(line).unwrap().display != Display::None,
                text,
            )
        })
        .collect()
}

#[test]
fn chat_input_is_a_text_input_widget() {
    let mut app = panel_app();
    let inputs = app
        .world_mut()
        .query_filtered::<(), (With<ChatInput>, With<TextInput>)>()
        .iter(app.world())
        .count();
    assert_eq!(
        inputs, 1,
        "0.20 TextInputPlugin ignores fields without TextInput"
    );
}

#[test]
fn a_tab_hides_other_channels_and_drops_its_own_tag() {
    let mut app = panel_app();
    app.world_mut()
        .write_message(heard(ChatChannel::Party, "Bjorn", "ready"));
    app.world_mut()
        .write_message(heard(ChatChannel::Local, "Ulf", "hi"));
    app.world_mut().write_message(ChatNotice {
        error: false,
        message: "Trade completed.".into(),
    });
    app.world_mut().write_message(ChatNotice {
        error: true,
        message: "Freyja is not online.".into(),
    });
    app.update();
    assert_eq!(
        log_lines(&mut app),
        [
            (true, "[Party] Bjorn : ready".to_string()),
            (true, "Ulf : hi".to_string()),
            (true, "Trade completed.".to_string()),
            (true, "Freyja is not online.".to_string()),
        ]
    );

    app.world_mut()
        .resource_mut::<ChatUi>()
        .show_tab(Some(Scope::Party));
    app.update();
    assert_eq!(
        log_lines(&mut app),
        [
            (true, "Bjorn : ready".to_string()),
            (false, "Ulf : hi".to_string()),
            (false, "Trade completed.".to_string()),
            // A refusal shows on every tab, so it is seen where it was caused.
            (true, "Freyja is not online.".to_string()),
        ]
    );
}

#[test]
fn timestamps_stand_in_front_of_every_line_while_on() {
    let mut app = panel_app();
    app.world_mut()
        .write_message(heard(ChatChannel::Local, "Ulf", "hi"));
    app.world_mut().write_message(ChatNotice {
        error: false,
        message: "Trade completed.".into(),
    });
    app.update();
    app.world_mut().resource_mut::<ChatUi>().show_time = true;
    app.update();

    let lines = log_lines(&mut app);
    let stamped = |text: &str, rest: &str| {
        let (time, tail) = text.split_at(5);
        time.as_bytes()[2] == b':' && tail == format!(" {rest}")
    };
    assert!(stamped(&lines[0].1, "Ulf : hi"), "{:?}", lines[0].1);
    assert!(stamped(&lines[1].1, "Trade completed."), "{:?}", lines[1].1);
}

#[test]
fn a_linked_item_rides_its_span() {
    let mut app = panel_app();
    app.world_mut().write_message(ChatHeard {
        links: vec![ZoneInventoryItem {
            index: 0,
            nameid: 2503,
            type_: 4,
            amount: 0,
            location: 0,
            identified: true,
            attribute: 0,
            refine: 7,
            cards: vec![],
            options: vec![],
            expire_time: 0,
            bound: 0,
            favorite: false,
            look: 0,
        }],
        ..heard(ChatChannel::Trade, "Hakon", &format!("S> {CHAT_LINK_MARK}"))
    });
    app.update();

    let links: Vec<u32> = app
        .world_mut()
        .query::<&SpanAction>()
        .iter(app.world())
        .filter_map(|action| match action {
            SpanAction::Link(item) => Some(item.item_id),
            SpanAction::Whisper(_) => None,
        })
        .collect();
    assert_eq!(links, [2503]);
}

#[test]
fn the_log_keeps_only_the_newest_lines() {
    let mut app = panel_app();
    for line in 0..(MAX_CHAT_LINES + 5) {
        app.world_mut().write_message(ChatNotice {
            error: false,
            message: format!("line{line}"),
        });
    }
    app.update();

    let lines = log_lines(&mut app);
    assert_eq!(lines.len(), MAX_CHAT_LINES);
    assert_eq!(lines[0].1, "line5");
    assert_eq!(
        lines.last().unwrap().1,
        format!("line{}", MAX_CHAT_LINES + 4)
    );
}

#[test]
fn a_notice_keeps_its_inline_colour_runs() {
    let mut app = panel_app();
    app.world_mut().write_message(ChatNotice {
        error: false,
        message: "Use ^ff0000fire^000000 here".into(),
    });
    app.update();
    assert_eq!(log_lines(&mut app), [(true, "Use fire here".to_string())]);
}

#[test]
fn the_hint_names_where_the_input_sends() {
    let mut app = panel_app();
    app.update();
    let hint = |app: &mut App| {
        app.world_mut()
            .query_filtered::<&Text, With<ChatHint>>()
            .single(app.world())
            .unwrap()
            .0
            .clone()
    };
    assert_eq!(hint(&mut app), "Say to Local \u{b7} / for commands");

    app.world_mut().resource_mut::<ChatUi>().target = Scope::Whisper("Freyja".into());
    app.update();
    assert_eq!(hint(&mut app), "Whisper to Freyja \u{b7} / for commands");
}

/// A bare chat input holding `draft`, driven by [`chat_input_control`] alone.
fn input_app(draft: &str) -> (App, Entity) {
    let mut app = App::new();
    input_messages(&mut app);
    app.init_resource::<ChatUi>()
        .add_systems(Update, chat_input_control);
    let field = app
        .world_mut()
        .spawn((ChatInput, EditableText::new(draft)))
        .id();
    (app, field)
}

fn press(app: &mut App, key: KeyCode) {
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.clear();
    keys.release_all();
    keys.clear();
    keys.press(key);
    app.update();
}

/// Submits `draft` from a focused input.
fn submit(draft: &str) -> (App, Entity) {
    let (mut app, field) = input_app(draft);
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    press(&mut app, KeyCode::Enter);
    (app, field)
}

fn sent<M: Message + Clone>(app: &App) -> Vec<M> {
    app.world()
        .resource::<Messages<M>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn draft(app: &App, field: Entity) -> String {
    app.world()
        .get::<EditableText>(field)
        .unwrap()
        .value()
        .to_string()
}

#[test]
fn enter_focuses_the_idle_input() {
    let (mut app, field) = input_app("");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    assert!(
        sent::<ChatSent>(&app).is_empty(),
        "the opening Enter sends nothing"
    );
}

#[test]
fn enter_says_the_draft_and_hands_the_keyboard_back() {
    let (mut app, field) = input_app("hello world");
    app.world_mut().resource_mut::<ChatUi>().target = Scope::Party;
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    press(&mut app, KeyCode::Enter);

    assert_eq!(
        sent::<ChatSent>(&app),
        [ChatSent {
            channel: ChatChannel::Party,
            text: "hello world".into(),
            ..default()
        }]
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(draft(&app, field), "");
}

#[test]
fn a_whisper_command_sends_to_its_hero_and_stays_aimed_at_them() {
    let (app, _) = submit("/w Freyja want my leaf?");
    assert_eq!(
        sent::<ChatSent>(&app),
        [ChatSent {
            channel: ChatChannel::Whisper,
            text: "want my leaf?".into(),
            whisper_to: "Freyja".into(),
            link_indices: vec![],
        }]
    );
    assert_eq!(
        app.world().resource::<ChatUi>().target,
        Scope::Whisper("Freyja".into())
    );
}

#[test]
fn a_linked_item_is_sent_as_a_mark_and_its_slot() {
    let (mut app, field) = input_app("B> [Elunium] x20");
    app.world_mut().resource_mut::<ChatUi>().links = vec![("[Elunium]".into(), 12)];
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    press(&mut app, KeyCode::Enter);

    let said = &sent::<ChatSent>(&app)[0];
    assert_eq!(said.text, format!("B> {CHAT_LINK_MARK} x20"));
    assert_eq!(said.link_indices, [12]);
    assert!(app.world().resource::<ChatUi>().links.is_empty());
}

#[test]
fn a_gm_command_is_its_own_request_never_speech() {
    let (app, _) = submit("@item 501 10");
    assert_eq!(
        sent::<GmCommandSent>(&app),
        [GmCommandSent {
            command: "item 501 10".into()
        }]
    );
    assert!(sent::<ChatSent>(&app).is_empty());
}

#[test]
fn a_client_command_reaches_its_parser_and_is_never_said() {
    let (app, _) = submit("/pcreate Wolfpack");
    assert_eq!(
        sent::<PartySlashSubmitted>(&app)[0].0,
        PartySlash::Create("Wolfpack".into())
    );
    assert!(sent::<ChatSent>(&app).is_empty());

    let (app, _) = submit("/surprise");
    assert_eq!(sent::<EmoteRequested>(&app)[0].emote_type, 0);
    assert!(sent::<PartySlashSubmitted>(&app).is_empty());

    let (app, _) = submit("/unmount");
    assert!(!sent::<MountPeco>(&app)[0].mount);

    let (app, _) = submit("/trade Bob");
    assert_eq!(sent::<TradeSlashSubmitted>(&app)[0].0, "Bob");

    let (app, _) = submit("/navi geffen");
    assert_eq!(
        sent::<NaviSlash>(&app),
        [NaviSlash::To(NavigationTarget::Map("geffen".into()))]
    );

    let (app, _) = submit("/guild \"Viking Guild\"");
    assert_eq!(
        sent::<GuildSlashSubmitted>(&app),
        [GuildSlashSubmitted(Ok("Viking Guild".into()))]
    );

    let (app, _) = submit("/guild missing quotes");
    assert!(sent::<GuildSlashSubmitted>(&app)[0].0.is_err());
    assert!(sent::<ChatSent>(&app).is_empty());
}

#[test]
fn memo_takes_no_argument() {
    let (app, _) = submit("/memo");
    assert_eq!(sent::<MemoRequested>(&app).len(), 1);
    assert!(sent::<ChatNotice>(&app).is_empty());

    let (app, _) = submit("/memo prontera");
    assert!(sent::<MemoRequested>(&app).is_empty());
    assert_eq!(sent::<ChatNotice>(&app)[0].message, "Usage: /memo");
}

#[test]
fn an_unknown_command_is_refused_not_said_aloud() {
    let (app, field) = submit("/zzz secret");
    let notices = sent::<ChatNotice>(&app);
    assert!(notices[0].error);
    assert!(notices[0].message.contains("/zzz"));
    assert!(sent::<ChatSent>(&app).is_empty());
    assert_eq!(draft(&app, field), "");
}

#[test]
fn tab_completes_the_highlighted_command_and_keeps_typing() {
    let (mut app, field) = input_app("/p");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    press(&mut app, KeyCode::ArrowDown);
    // Tab navigation takes the focus away before the chat sees the key.
    app.world_mut().resource_mut::<InputFocus>().clear();
    press(&mut app, KeyCode::Tab);

    assert_eq!(draft(&app, field), "/pinvite ");
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    assert!(sent::<PartySlashSubmitted>(&app).is_empty());
}

#[test]
fn enter_on_an_unfinished_command_completes_it_instead_of_sending() {
    let (app, field) = submit("/mem");
    assert_eq!(draft(&app, field), "/memo");
    assert!(sent::<MemoRequested>(&app).is_empty());
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
}

#[test]
fn escape_clears_a_command_draft_and_keeps_the_input() {
    let (mut app, field) = input_app("/pcr");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    app.update();
    // `TextInput` releases the focus on Escape before the chat sees the key.
    app.world_mut().resource_mut::<InputFocus>().clear();
    press(&mut app, KeyCode::Escape);

    assert_eq!(draft(&app, field), "");
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
}

#[test]
fn escape_on_plain_text_leaves_the_input_released() {
    let (mut app, field) = input_app("hello");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Navigated);
    app.update();
    app.world_mut().resource_mut::<InputFocus>().clear();
    press(&mut app, KeyCode::Escape);

    assert_eq!(draft(&app, field), "hello");
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
}

#[test]
fn linking_an_item_appends_its_label_to_the_draft() {
    let (mut app, field) = input_app("B>");
    app.world_mut()
        .run_system_once(|mut desk: ChatDesk| desk.link("[Elunium]".into(), 12))
        .unwrap();

    assert_eq!(draft(&app, field), "B> [Elunium] ");
    assert_eq!(
        app.world().resource::<ChatUi>().links,
        [("[Elunium]".to_string(), 12)]
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
}
