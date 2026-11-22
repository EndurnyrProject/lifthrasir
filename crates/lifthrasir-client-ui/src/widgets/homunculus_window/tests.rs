use bevy::scene::ScenePlugin;
use net_contract::dto::{
    HomunculusActivity, HomunculusAiConfig, HomunculusAiSkillConfig, HomunculusDisplayedStats,
    HomunculusIntimacyGrade, HomunculusLifecycle, HomunculusSkillMetadata,
};

use super::view::{BodyView, Control, Ctx, PageView};
use super::*;

#[test]
fn care_confirmation_is_bound_to_durable_id() {
    let prompt = Confirmation {
        id: 31,
        delete: true,
    };
    let mut companion = HomunculusSnapshot {
        durable_id: 31,
        ..default()
    };
    assert_eq!(
        confirm(prompt, &companion),
        Some(Action::Delete { confirmed: true })
    );
    companion.durable_id = 32;
    assert_eq!(confirm(prompt, &companion), None);
    let feed = Confirmation {
        id: 31,
        delete: false,
    };
    assert_eq!(confirm(feed, &companion), None);
}

#[test]
fn status_and_context_menu_are_owner_scoped() {
    let state = HomunculusState::default();
    let actor = Entity::from_raw_u32(7).unwrap();
    assert!(!owns_pick(&state, actor, Some(&ActorVisualRoot(actor))));
    assert_eq!(view::body(state.presence()), Some(BodyView::Loading));
    assert_eq!(notice(&state), None);
}

fn skill(skill_id: u32, level: u32, max_level: u32) -> HomunculusSkillMetadata {
    HomunculusSkillMetadata {
        skill_id,
        level,
        max_level,
        ..default()
    }
}

fn rule(skill_id: u32, mode: Mode, priority: u32) -> HomunculusAiSkillConfig {
    HomunculusAiSkillConfig {
        skill_id,
        mode,
        priority,
        ..default()
    }
}

/// A loyal evolved Lif that is out and following: one cast skill on auto, one on
/// manual, a passive, and one it has the intimacy and the points to learn.
fn pip() -> HomunculusSnapshot {
    HomunculusSnapshot {
        durable_id: 31,
        name: "Pip".into(),
        rename_eligible: true,
        appearance_id: 6009,
        evolved: true,
        lifecycle: HomunculusLifecycle::Active,
        activity: HomunculusActivity::Following,
        level: 99,
        exp: 724,
        next_exp: 1000,
        skill_points: 2,
        hp: 6820,
        max_hp: 7410,
        sp: 512,
        max_sp: 680,
        stats: Some(HomunculusDisplayedStats::default()),
        hunger: 48,
        intimacy_hundredths: 91250,
        intimacy_grade: HomunculusIntimacyGrade::Loyal,
        food_item_id: 537,
        skills: vec![
            skill(8001, 5, 5),
            skill(8002, 1, 5),
            skill(8003, 5, 5),
            HomunculusSkillMetadata {
                learnable: true,
                intimacy_required_hundredths: 91000,
                ..skill(8004, 0, 3)
            },
        ],
        ai_config: Some(HomunculusAiConfig {
            stance: Stance::Assist,
            leash_distance: 8,
            auto_feed: true,
            auto_feed_threshold: 30,
            skills: vec![
                rule(8002, Mode::Manual, 1),
                HomunculusAiSkillConfig {
                    owner_hp_threshold: Some(60),
                    ..rule(8001, Mode::Auto, 2)
                },
                rule(8003, Mode::Manual, 1),
                rule(8004, Mode::Manual, 1),
            ],
            ..default()
        }),
        ..default()
    }
}

fn ctx(ui: &OwnerUi) -> Ctx<'_> {
    Ctx {
        ui,
        skills: None,
        items: None,
        food: 3,
        out: true,
        active_left: Duration::from_secs(90),
        refusal: None,
        plate_minimal: false,
    }
}

#[test]
fn a_name_is_sent_only_when_it_is_new_and_not_blank() {
    assert_eq!(new_name("  Mochi ", "Pip"), Some("Mochi".into()));
    assert_eq!(new_name("Pip", "Pip"), None);
    assert_eq!(new_name("   ", "Pip"), None);
}

#[test]
fn the_plate_shows_only_a_companion_that_is_out_and_warns_when_it_is_hungry() {
    let ui = OwnerUi::default();
    let mut pip = pip();

    let plate = view::hud(&pip, &ctx(&ui), Default::default()).unwrap();
    assert_eq!(plate.activity, "Following");
    assert_eq!(plate.order, Some(view::Order::Follow));
    assert_eq!(plate.auto.as_deref(), Some("Auto Skill #8001 <60%"));
    assert!(!plate.hungry);

    pip.hunger = 20;
    let plate = view::hud(&pip, &ctx(&ui), Default::default()).unwrap();
    assert!(plate.hungry);
    assert_eq!(plate.activity, "Following \u{b7} Hungry");

    pip.lifecycle = HomunculusLifecycle::Rested;
    assert_eq!(view::hud(&pip, &ctx(&ui), Default::default()), None);
}

#[test]
fn feeding_needs_food_and_says_when_it_would_hurt() {
    let ui = OwnerUi::default();
    let mut pip = pip();

    let care = view::care(&pip, &ctx(&ui));
    assert!(care.can_feed);
    assert_eq!(care.feed, "Feed \u{b7} Item #537 \u{d7}3");
    assert_eq!(care.note, "Eats by itself at 30");
    assert_eq!(care.feed_line, Some(30.0));
    assert_eq!(care.intimacy, "912.50");

    let empty = Ctx {
        food: 0,
        ..ctx(&ui)
    };
    assert!(!view::care(&pip, &empty).can_feed);

    pip.hunger = 80;
    let care = view::care(&pip, &ctx(&ui));
    assert_eq!(care.note, "Not hungry \u{b7} feeding now lowers intimacy");
}

#[test]
fn a_skill_row_offers_only_what_the_skill_allows() {
    let ui = OwnerUi::default();
    let mut pip = pip();
    let rows = |pip: &HomunculusSnapshot| match view::page(
        pip,
        &Ctx {
            ui: &OwnerUi {
                tab: HomunculusTab::Skills,
                ..default()
            },
            ..ctx(&ui)
        },
    ) {
        PageView::Skills(skills) => skills.rows,
        other => panic!("expected the skills page, got {other:?}"),
    };

    let shown = rows(&pip);
    let [heal, escape, passive, unlearned] = shown.as_slice() else {
        panic!("expected four rows");
    };
    assert!(heal.can_cast && heal.auto && !heal.can_learn);
    assert_eq!(heal.rank, Some(5));
    assert_eq!(escape.rank, None);
    assert!(!escape.auto);
    assert!(passive.passive && !passive.can_cast);
    assert!(unlearned.can_learn && !unlearned.can_cast);
    assert_eq!(unlearned.lock.as_ref().unwrap().0, "Ready to learn");

    pip.intimacy_hundredths = 50000;
    let shown = rows(&pip);
    assert_eq!(shown[3].lock.as_ref().unwrap().0, "Needs 910 intimacy");
}

#[test]
fn the_behaviour_page_follows_the_draft() {
    let pip = pip();
    let config = pip.ai_config.clone().unwrap();
    let mut ui = OwnerUi {
        tab: HomunculusTab::Tactics,
        ..default()
    };
    ui.tactics.follow(pip.durable_id, &config);
    let page = |ui: &OwnerUi| match view::page(&pip, &ctx(ui)) {
        PageView::Tactics(tactics) => tactics,
        other => panic!("expected the behaviour page, got {other:?}"),
    };

    let clean = page(&ui);
    assert_eq!(clean.state.0, "Pip knows these rules");
    assert!(!clean.can_teach);
    let body = clean.body.unwrap();
    assert_eq!(body.rows[2].control, Control::Flag(Flag::Retaliate, false));
    // The passive 8003 has no rule to show; the unlearned 8004 is cast on itself.
    let order: Vec<_> = body.rules.iter().map(|rule| rule.skill_id).collect();
    assert_eq!(order, [8001, 8002, 8004]);
    assert_eq!(body.rules[0].dials.len(), 2);
    assert!(body.rules[1].dials.is_empty());

    ui.tactics.flip(Flag::Retaliate);
    let edited = page(&ui);
    assert_eq!(edited.state.0, "Changes not taught yet");
    assert!(edited.can_teach);
    assert!(view::tabs(&pip, &ctx(&ui)).unsaved);
}

/// An app that can spawn the window's scenes.
fn scene_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app
}

#[test]
fn the_minified_plate_is_only_the_portrait_which_brings_the_plate_back() {
    let ui = OwnerUi::default();
    let minimal = Ctx {
        plate_minimal: true,
        ..ctx(&ui)
    };
    let view = view::hud(&pip(), &minimal, Default::default()).unwrap();
    assert!(view.minimal);
    let mut app = scene_app();
    app.world_mut().commands().spawn_scene(hud::plate(view));
    app.world_mut().flush();
    app.update();

    let world = app.world_mut();
    let acts: Vec<Act> = world.query::<&Act>().iter(world).copied().collect();
    assert_eq!(acts, [Act::TogglePlate]);
    assert_eq!(world.query::<&Portrait>().iter(world).count(), 1);
    // The level under the portrait is all it says.
    let texts: Vec<&str> = world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.as_str())
        .collect();
    assert_eq!(texts, ["99"]);
}

/// Spawns every scene of the window and the plate, so a malformed one panics here
/// instead of only at runtime.
#[test]
fn every_region_spawns() {
    let mut app = scene_app();
    let pip = pip();
    let mut ui = OwnerUi::default();
    ui.tactics
        .follow(pip.durable_id, pip.ai_config.as_ref().unwrap());
    let renaming = OwnerUi {
        renaming: true,
        ..default()
    };
    let resting = Ctx {
        out: false,
        ..ctx(&ui)
    };
    let on_tab = |tab| OwnerUi { tab, ..default() };
    let (skills, tactics) = (on_tab(HomunculusTab::Skills), {
        let mut ui = on_tab(HomunculusTab::Tactics);
        ui.tactics
            .follow(pip.durable_id, pip.ai_config.as_ref().unwrap());
        ui
    });
    let prompt = |delete| Confirmation { id: 31, delete };
    let contents = vec![
        Content::Lead("Tove\u{2019}s companion".into(), ro_dark::MUTED),
        Content::Body(BodyView::Loading),
        Content::Body(BodyView::Absent),
        Content::Portrait(view::portrait(&pip)),
        Content::Name(view::name(&pip, &ctx(&ui))),
        Content::Name(view::name(&pip, &ctx(&renaming))),
        Content::Care(view::care(&pip, &ctx(&ui))),
        Content::Orders(view::orders(&pip, &ctx(&ui))),
        Content::Orders(view::orders(&pip, &resting)),
        Content::Tabs(view::tabs(&pip, &ctx(&tactics))),
        Content::Page(view::page(&pip, &ctx(&ui))),
        Content::Page(view::page(&pip, &ctx(&skills))),
        Content::Page(view::page(&pip, &ctx(&tactics))),
        Content::Confirm(view::confirm(prompt(true), &pip, &ctx(&ui))),
        Content::Confirm(view::confirm(prompt(false), &pip, &ctx(&ui))),
        Content::Hud(view::hud(&pip, &ctx(&ui), Default::default()).unwrap()),
    ];
    let root = app.world_mut().spawn_empty().id();
    let mut commands = app.world_mut().commands();
    spawn_window(&mut commands, root);
    commands.spawn_scene(scene::context_menu(Vec2::ZERO));
    for content in contents {
        content.spawn(&mut commands);
    }
    app.world_mut().flush();
    app.update();

    let world = app.world_mut();
    let texts: Vec<String> = world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect();
    for shown in [
        "Homunculus",
        "Pip",
        "Lif \u{b7} Evolved",
        "Send to Rest",
        "Pip takes orders once it is out with you",
        "6,820 / 7,410",
        "Use now",
        "HOW PIP FIGHTS",
        "Teach Pip",
        "Delete Pip?",
        "Feed Anyway",
    ] {
        assert!(texts.iter().any(|text| text == shown), "missing {shown:?}");
    }
    let hosts = world.query::<&Host>().iter(world).count();
    assert_eq!(hosts, HOSTS);
}
