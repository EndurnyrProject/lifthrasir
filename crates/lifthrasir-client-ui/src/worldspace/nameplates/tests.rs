use super::*;
use bevy::scene::ScenePlugin;
use lifthrasir_client::domain::guild::GuildPlugin;
use lifthrasir_client::domain::guild::emblems::test_emblem_bmp;
use lifthrasir_client::domain::input::LockedTarget;
use lifthrasir_ui::theme::ro_dark;
use net_contract::{
    dto::GuildInfo,
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::ZoneSessionGeneration,
};

fn unit(kind: PlateKind) -> Unit<'static> {
    Unit {
        kind,
        name: "Poring",
        party: Some("Wolfpack"),
        guild: Some(valkyries(false)),
    }
}

fn valkyries(allied: bool) -> GuildLine {
    GuildLine {
        name: "Valkyries".into(),
        emblem: EmblemKey::new(7, 3),
        allied,
    }
}

const HOVERED: Focus = Focus {
    hovered: true,
    targeted: false,
    show_all: false,
};

#[test]
fn a_monster_shows_only_while_pointed_at_or_asked_for() {
    let mob = PlateKind::Mob { aggressive: false };
    assert_eq!(PlateView::of(unit(mob), Focus::default()), None);

    let hovered = PlateView::of(unit(mob), HOVERED).unwrap();
    assert!(hovered.backed);

    let targeted = Focus {
        targeted: true,
        ..default()
    };
    assert!(PlateView::of(unit(mob), targeted).unwrap().backed);

    let asked = Focus {
        show_all: true,
        ..default()
    };
    assert!(!PlateView::of(unit(mob), asked).unwrap().backed);
}

#[test]
fn npcs_party_members_bosses_and_the_hero_always_show() {
    for kind in [
        PlateKind::Npc,
        PlateKind::Party,
        PlateKind::Boss,
        PlateKind::Own,
    ] {
        let view = PlateView::of(unit(kind), Focus::default());
        assert!(view.is_some_and(|view| !view.backed), "{kind:?}");
    }
    assert_eq!(
        PlateView::of(unit(PlateKind::Player), Focus::default()),
        None
    );
}

#[test]
fn the_hero_plate_leaves_the_party_out_and_names_the_guild_only_on_hover() {
    let resting = PlateView::of(unit(PlateKind::Own), Focus::default()).unwrap();
    assert_eq!((resting.party, resting.guild), (None, None));

    let hovered = PlateView::of(unit(PlateKind::Own), HOVERED).unwrap();
    assert_eq!(
        (hovered.party, hovered.guild),
        (None, Some(valkyries(false)))
    );

    let member = PlateView::of(unit(PlateKind::Party), Focus::default()).unwrap();
    assert_eq!(member.party.as_deref(), Some("Wolfpack"));
    assert_eq!(member.guild, Some(valkyries(false)));
}

#[test]
fn a_script_suffix_is_cut_from_the_name_and_a_nameless_unit_shows_nothing() {
    let named = |name| Unit {
        name,
        ..unit(PlateKind::Npc)
    };
    let kafra = PlateView::of(named("Kafra Employee#prt"), Focus::default()).unwrap();
    assert_eq!(kafra.name, "Kafra Employee");
    assert_eq!(PlateView::of(named("#hidden"), HOVERED), None);
    assert_eq!(PlateView::of(named(""), HOVERED), None);
}

#[test]
fn names_and_guilds_are_coloured_by_standing() {
    let passive = PlateKind::Mob { aggressive: false };
    let hostile = PlateKind::Mob { aggressive: true };
    assert_eq!(passive.name_color(), ro_dark::STRONG);
    assert_ne!(hostile.name_color(), passive.name_color());
    assert_eq!(PlateKind::Boss.name_color(), ro_dark::RARE_RING);
    assert_eq!(valkyries(true).color(), ro_dark::SELECT_RING);
    assert_ne!(valkyries(false).color(), valkyries(true).color());
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>().init_asset::<Font>();
    app.init_resource::<EntityRegistry>()
        .init_resource::<PartyState>()
        .init_resource::<LockedTarget>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<GraphicsSettings>()
        .init_resource::<GuildEmblemImages>()
        .add_message::<GuildIngress>()
        .add_message::<ZoneDisconnected>()
        .insert_resource(ZoneSessionGeneration(1))
        .add_plugins(GuildPlugin)
        .add_systems(
            Update,
            (
                sync_nameplates,
                fill_vitals,
                request_visible_emblems,
                sync_plate_emblems,
            )
                .chain()
                .after(GuildSystems::UiSync)
                .before(GuildSystems::EmblemSend),
        );
    app
}

fn named(app: &mut App, name: &str, extra: impl Bundle) -> Entity {
    app.world_mut()
        .spawn((EntityName::new(name.into()), Visibility::Inherited, extra))
        .id()
}

fn register(app: &mut App, gid: u32, unit: Entity) {
    app.world_mut()
        .resource_mut::<EntityRegistry>()
        .register_entity(gid, unit);
}

fn view_of(app: &mut App, unit: Entity) -> Option<PlateView> {
    let world = app.world_mut();
    let mut views: Vec<PlateView> = world
        .query::<(&WorldAnchor, &Nameplate)>()
        .iter(world)
        .filter(|(anchor, _)| anchor.target == unit)
        .map(|(_, plate)| plate.view.clone())
        .collect();
    assert!(views.len() <= 1, "a unit never carries two plates");
    views.pop()
}

fn fill(app: &mut App, vital: Vital) -> Val {
    let world = app.world_mut();
    world
        .query::<(&Vital, &Node)>()
        .iter(world)
        .find(|(found, _)| **found == vital)
        .map(|(_, node)| node.width)
        .expect("the plate has that bar")
}

fn wolfpack(member_char_id: u32, hp: u64) -> PartyState {
    PartyState {
        party_id: 7,
        name: "Wolfpack".into(),
        members: vec![PartyMemberInfo {
            char_id: member_char_id,
            name: "Solveig".into(),
            base_level: 99,
            online: true,
            map: "prontera".into(),
            job_id: 0,
            hp,
            max_hp: 200,
            sp: 0,
            max_sp: 0,
            ap: 0,
            max_ap: 0,
        }],
        ..default()
    }
}

fn join_guild(app: &mut App, guild_id: u32, emblem_id: u32) {
    app.world_mut().write_message(GuildIngress {
        generation: ZoneSessionGeneration(1),
        payload: GuildIngressPayload::Info(GuildInfo {
            guild_id,
            name: "Valkyries".into(),
            emblem_id,
            ..default()
        }),
    });
}

fn sigrun_guild(emblem_id: u32) -> GuildIdentity {
    GuildIdentity {
        guild_id: 7,
        guild_name: "Valkyries".into(),
        emblem_id,
    }
}

#[test]
fn an_npc_is_plated_at_once_and_a_monster_only_while_hovered() {
    let mut app = app();
    let kafra = named(&mut app, "Kafra Employee", Npc);
    let poring = named(&mut app, "Poring", Mob);

    app.update();
    assert_eq!(view_of(&mut app, kafra).unwrap().kind, PlateKind::Npc);
    assert_eq!(view_of(&mut app, poring), None);

    app.world_mut().entity_mut(poring).insert(HoveredEntity);
    app.update();
    let hovered = view_of(&mut app, poring).unwrap();
    assert_eq!(hovered.kind, PlateKind::Mob { aggressive: false });
    assert!(hovered.backed);

    app.world_mut().entity_mut(poring).remove::<HoveredEntity>();
    app.update();
    assert_eq!(view_of(&mut app, poring), None);
}

#[test]
fn holding_alt_plates_every_unit() {
    let mut app = app();
    let poring = named(&mut app, "Poring", (Mob, Aggressive));
    let stranger = named(&mut app, "Stranger", ());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::AltLeft);

    app.update();

    let poring = view_of(&mut app, poring).unwrap();
    assert_eq!(poring.kind, PlateKind::Mob { aggressive: true });
    assert!(!poring.backed);
    assert_eq!(view_of(&mut app, stranger).unwrap().kind, PlateKind::Player);
}

#[test]
fn the_show_all_names_setting_plates_every_unit() {
    let mut app = app();
    let poring = named(&mut app, "Poring", Mob);
    app.update();
    assert_eq!(view_of(&mut app, poring), None);

    app.world_mut()
        .resource_mut::<GraphicsSettings>()
        .show_names = true;
    app.update();

    assert!(!view_of(&mut app, poring).unwrap().backed);
}

#[test]
fn the_locked_target_and_a_boss_are_plated_without_hover() {
    let mut app = app();
    let poring = named(&mut app, "Poring", Mob);
    let lord = named(&mut app, "Dark Lord", (Mob, Boss));
    register(&mut app, 7, poring);
    app.world_mut().resource_mut::<LockedTarget>().gid = Some(7);

    app.update();

    assert!(view_of(&mut app, poring).unwrap().backed);
    let lord = view_of(&mut app, lord).unwrap();
    assert_eq!((lord.kind, lord.backed), (PlateKind::Boss, false));
}

#[test]
fn hidden_units_and_dead_monsters_show_no_plate() {
    let mut app = app();
    let cloaked = named(&mut app, "Kafra Employee", Npc);
    app.world_mut()
        .entity_mut(cloaked)
        .insert(Visibility::Hidden);
    let corpse = named(&mut app, "Dark Lord", (Mob, Boss, DeadEntity));

    app.update();

    assert_eq!(view_of(&mut app, cloaked), None);
    assert_eq!(view_of(&mut app, corpse), None);
}

#[test]
fn a_party_member_shows_the_party_and_a_health_bar_that_follows_the_roster() {
    let mut app = app();
    let solveig = named(&mut app, "Solveig", ());
    register(&mut app, 42, solveig);
    app.insert_resource(wolfpack(42, 100));

    app.update();
    app.update();

    let view = view_of(&mut app, solveig).unwrap();
    assert_eq!(view.kind, PlateKind::Party);
    assert_eq!(view.party.as_deref(), Some("Wolfpack"));
    assert_eq!(fill(&mut app, Vital::Hp), Val::Percent(50.0));

    app.insert_resource(wolfpack(42, 50));
    app.update();
    assert_eq!(fill(&mut app, Vital::Hp), Val::Percent(25.0));
}

#[test]
fn a_stranger_is_no_party_member_but_shows_the_party_the_name_reply_gave() {
    let mut app = app();
    let stranger = named(&mut app, "Stranger", HoveredEntity);
    register(&mut app, 99, stranger);
    app.insert_resource(wolfpack(42, 100));
    let mut rival = EntityName::new("Rival".into());
    rival.party_name = Some("Ravens".into());
    let rival = app
        .world_mut()
        .spawn((rival, Visibility::Inherited, HoveredEntity))
        .id();

    app.update();

    let stranger = view_of(&mut app, stranger).unwrap();
    assert_eq!((stranger.kind, stranger.party), (PlateKind::Player, None));
    let rival = view_of(&mut app, rival).unwrap();
    assert_eq!(rival.party.as_deref(), Some("Ravens"));
}

#[test]
fn the_hero_plate_bars_follow_their_status() {
    let mut app = app();
    let status = CharacterStatus {
        hp: 30,
        max_hp: 120,
        sp: 9,
        max_sp: 10,
        ..default()
    };
    let hero = named(&mut app, "Ygor", (LocalPlayer, status));

    app.update();
    app.update();

    assert_eq!(view_of(&mut app, hero).unwrap().kind, PlateKind::Own);
    assert_eq!(fill(&mut app, Vital::Hp), Val::Percent(25.0));
    assert_eq!(fill(&mut app, Vital::Sp), Val::Percent(90.0));
}

#[test]
fn the_hero_guild_comes_from_the_guild_state_and_marks_guildmates_allied() {
    let mut app = app();
    let hero = named(&mut app, "Ygor", (LocalPlayer, HoveredEntity));
    let mate = named(&mut app, "Sigrun", (HoveredEntity, sigrun_guild(3)));
    let other = named(
        &mut app,
        "Halvard",
        (
            HoveredEntity,
            GuildIdentity {
                guild_id: 8,
                guild_name: "Ravens".into(),
                emblem_id: 1,
            },
        ),
    );
    join_guild(&mut app, 7, 3);

    app.update();

    assert_eq!(
        view_of(&mut app, hero).unwrap().guild,
        Some(valkyries(true))
    );
    assert_eq!(
        view_of(&mut app, mate).unwrap().guild,
        Some(valkyries(true))
    );
    let other = view_of(&mut app, other).unwrap().guild.unwrap();
    assert_eq!((other.name.as_str(), other.allied), ("Ravens", false));
}

#[test]
fn a_changed_emblem_rebuilds_the_plate() {
    let mut app = app();
    let sigrun = named(&mut app, "Sigrun", (HoveredEntity, sigrun_guild(3)));
    app.update();
    let emblem = |app: &mut App| view_of(app, sigrun).unwrap().guild.unwrap().emblem;
    assert_eq!(emblem(&mut app), EmblemKey::new(7, 3));

    app.world_mut().entity_mut(sigrun).insert(sigrun_guild(4));
    app.update();
    app.update();

    assert_eq!(emblem(&mut app), EmblemKey::new(7, 4));
}

#[test]
fn a_plate_that_is_not_on_screen_never_asks_for_its_emblem() {
    let mut app = app();
    named(&mut app, "Sigrun", (HoveredEntity, sigrun_guild(3)));

    // Nothing places the plate here, so it stays hidden.
    app.update();
    app.update();

    let key = EmblemKey::new(7, 3).unwrap();
    assert!(
        app.world()
            .resource::<GuildEmblemImages>()
            .cached(key)
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Messages<net_contract::commands::GuildEmblemFetchRequested>>()
            .is_empty()
    );
}

#[test]
fn the_emblem_takes_room_only_while_its_image_is_cached() {
    let mut app = app();
    let key = EmblemKey::new(7, 3).unwrap();
    named(&mut app, "Sigrun", (HoveredEntity, sigrun_guild(3)));
    let emblem_display = |app: &mut App| {
        let world = app.world_mut();
        world
            .query_filtered::<&Node, With<PlateEmblem>>()
            .single(world)
            .unwrap()
            .display
    };
    app.update();
    assert_eq!(emblem_display(&mut app), Display::None);

    app.world_mut()
        .resource_mut::<GuildEmblemImages>()
        .request(key);
    app.update();
    app.world_mut().write_message(GuildIngress {
        generation: ZoneSessionGeneration(1),
        payload: GuildIngressPayload::EmblemData {
            guild_id: key.guild_id,
            emblem_id: key.emblem_id,
            data: test_emblem_bmp(24, 24),
        },
    });
    app.update();
    assert_eq!(emblem_display(&mut app), Display::Flex);

    app.world_mut().write_message(ZoneDisconnected {
        reason: "test cache reset".into(),
    });
    app.update();
    app.update();
    assert_eq!(emblem_display(&mut app), Display::None);
}
