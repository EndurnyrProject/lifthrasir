use std::time::Duration;

use bevy::scene::ScenePlugin;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use net_contract::dto::ChatChannel;
use net_contract::events::ChatHeard;

use super::*;

fn toast(kind: ToastKind, title: &str) -> ShowToast {
    ShowToast {
        kind,
        title: title.into(),
        sub: String::new(),
        action: None,
    }
}

/// An app running the stack, with each update a quarter second long.
fn stack_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        250,
    )));
    app.init_resource::<Toasts>();
    app.init_resource::<InventoryUi>();
    app.add_message::<ShowToast>();
    app.add_systems(Update, (ingest, age, rebuild, drain_life_bars).chain());
    let hud = app.world_mut().spawn(Node::default()).id();
    spawn_toast_stack(&mut app.world_mut().commands(), hud);
    app.update();
    app
}

fn titles(app: &App) -> Vec<&str> {
    let toasts = app.world().resource::<Toasts>();
    toasts
        .live
        .iter()
        .map(|toast| toast.show.title.as_str())
        .collect()
}

fn cards(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&ToastCard>().iter(world).count()
}

#[test]
fn a_fifth_toast_pushes_the_oldest_out() {
    let mut app = stack_app();
    for title in ["a", "b", "c", "d", "e"] {
        app.world_mut()
            .write_message(toast(ToastKind::Whisper, title));
    }
    app.update();

    assert_eq!(titles(&app), ["b", "c", "d", "e"]);
    assert_eq!(cards(&mut app), MAX_TOASTS);
}

#[test]
fn a_toast_leaves_when_its_life_runs_out() {
    let mut app = stack_app();
    app.world_mut()
        .write_message(toast(ToastKind::Zeny, "+1 z"));
    app.update();
    assert_eq!(cards(&mut app), 1);

    let updates = (ToastKind::Zeny.life() / 0.25) as usize + 2;
    for _ in 0..updates {
        app.update();
    }
    assert!(titles(&app).is_empty());
    assert_eq!(cards(&mut app), 0);
}

#[test]
fn the_stack_holds_still_while_the_pointer_is_on_a_card() {
    let mut app = stack_app();
    app.world_mut()
        .write_message(toast(ToastKind::Zeny, "+1 z"));
    app.update();
    let world = app.world_mut();
    let card = world
        .query_filtered::<Entity, With<ToastCard>>()
        .single(world)
        .unwrap();
    app.world_mut().entity_mut(card).insert(Hovered(true));

    for _ in 0..40 {
        app.update();
    }
    assert_eq!(titles(&app), ["+1 z"]);
}

#[test]
fn the_life_bar_drains_with_the_time_left() {
    let mut app = stack_app();
    app.world_mut()
        .write_message(toast(ToastKind::Zeny, "+1 z"));
    for _ in 0..9 {
        app.update();
    }

    let world = app.world_mut();
    let bar = world
        .query_filtered::<&Node, With<ToastLife>>()
        .single(world)
        .unwrap();
    let Val::Percent(width) = bar.width else {
        panic!("the life bar is sized in percent");
    };
    assert!((40.0..60.0).contains(&width), "{width}");
}

/// Activates a button of the only toast: its own action, or the dismiss mark.
fn press(app: &mut App, act: bool) {
    let id = app.world().resource::<Toasts>().live[0].id;
    let button = app
        .world_mut()
        .spawn(ToastButton { id, act })
        .observe(on_toast_button)
        .id();
    app.world_mut().trigger(Activate { entity: button });
    app.world_mut().flush();
}

#[test]
fn the_dismiss_mark_drops_the_toast_without_acting() {
    let mut app = stack_app();
    app.world_mut().write_message(ShowToast {
        action: Some(ToastAction::OpenInventory),
        ..toast(ToastKind::Weight, "heavy")
    });
    app.update();
    press(&mut app, false);

    assert!(titles(&app).is_empty());
    assert!(!app.world().resource::<InventoryUi>().open);
}

#[test]
fn the_weight_toast_opens_the_inventory() {
    let mut app = stack_app();
    app.world_mut().write_message(ShowToast {
        action: Some(ToastAction::OpenInventory),
        ..toast(ToastKind::Weight, "heavy")
    });
    app.update();
    press(&mut app, true);

    assert!(titles(&app).is_empty());
    assert!(app.world().resource::<InventoryUi>().open);
}

fn whisper(from: &str, text: &str) -> ChatHeard {
    ChatHeard {
        channel: ChatChannel::Whisper,
        sender_name: from.into(),
        text: text.into(),
        whisper_to: "Ygor".into(),
        ..default()
    }
}

#[test]
fn a_whisper_from_someone_else_is_toasted_with_a_reply_button() {
    let mut app = App::new();
    app.add_message::<ChatHeard>();
    app.add_message::<ShowToast>();
    app.add_systems(Update, feeds::whispers);
    app.world_mut().spawn((
        LocalPlayer,
        EntityName {
            name: "Ygor".into(),
            party_name: None,
            guild_name: None,
            position_name: None,
        },
    ));
    app.world_mut()
        .write_message(whisper("Halvard", "ten Elunium?"));
    app.world_mut().write_message(whisper("Ygor", "sure"));
    app.world_mut().write_message(ChatHeard {
        channel: ChatChannel::Party,
        ..whisper("Freyja", "heal incoming")
    });
    app.update();

    let messages = app.world().resource::<Messages<ShowToast>>();
    let raised: Vec<_> = messages.iter_current_update_messages().collect();
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].title, "Halvard");
    assert_eq!(raised[0].sub, "ten Elunium?");
    assert_eq!(raised[0].action, Some(ToastAction::Reply("Halvard".into())));
}
