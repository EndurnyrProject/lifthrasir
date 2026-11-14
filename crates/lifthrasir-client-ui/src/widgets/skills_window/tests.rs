use super::*;
use bevy::scene::ScenePlugin;
use lifthrasir_client::domain::skill::SkillNode;
use lifthrasir_ui::theme::ro_dark;

use crate::widgets::skills_window::looks::{Dim, Edge};
use crate::widgets::skills_window::scene::{
    SkillCanvasViewport, SkillLine, SkillNodeLevel, SkillPip,
};

#[test]
fn register_initializes_skills_tab_resources() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    register(&mut app);

    assert!(app.world().contains_resource::<SkillPanelUi>());
    assert!(app.world().contains_resource::<SkillPanelStaging>());
    assert!(app.world().contains_resource::<LastSkillPanelClick>());
    assert!(app.world().contains_resource::<SkillGateSnapshot>());
    assert!(app.world().contains_resource::<SkillStructureFingerprint>());
}

fn node(level: u32, max_level: u32, job_id: u32) -> SkillNode {
    SkillNode {
        level,
        max_level,
        upgradable: true,
        requires: vec![],
        req_base_level: 0,
        req_job_level: 0,
        sp: 0,
        range: 0,
        inf_type: 0,
        job_id,
        splash_radius: 0,
        cast_time: vec![],
        cooldown: vec![],
    }
}

fn tree(entries: &[(u32, SkillNode)]) -> SkillTreeState {
    let mut state = SkillTreeState::default();
    for (id, n) in entries {
        state.skills.insert(
            *id,
            SkillNode {
                level: n.level,
                max_level: n.max_level,
                upgradable: n.upgradable,
                requires: n.requires.clone(),
                req_base_level: n.req_base_level,
                req_job_level: n.req_job_level,
                sp: n.sp,
                range: n.range,
                inf_type: n.inf_type,
                job_id: n.job_id,
                splash_radius: n.splash_radius,
                cast_time: n.cast_time.clone(),
                cooldown: n.cooldown.clone(),
            },
        );
    }
    state
}

fn with_requires(mut n: SkillNode, requires: Vec<(u32, u32)>) -> SkillNode {
    n.requires = requires;
    n
}

fn with_levels(mut n: SkillNode, base: u32, job: u32) -> SkillNode {
    n.req_base_level = base;
    n.req_job_level = job;
    n
}

fn with_upgradable(mut n: SkillNode, upgradable: bool) -> SkillNode {
    n.upgradable = upgradable;
    n
}

fn with_inf(mut n: SkillNode, inf_type: u32) -> SkillNode {
    n.inf_type = inf_type;
    n
}

fn status(base_level: u32, job_level: u32) -> CharacterStatus {
    CharacterStatus {
        base_level,
        job_level,
        ..default()
    }
}

#[test]
fn double_click_same_skill_within_window_is_true() {
    let last = LastSkillPanelClick {
        skill_id: 5,
        at: Duration::from_millis(100),
    };
    assert!(is_cast_double_click(&last, 5, Duration::from_millis(350)));
}

#[test]
fn click_different_skill_is_false() {
    let last = LastSkillPanelClick {
        skill_id: 5,
        at: Duration::from_millis(100),
    };
    assert!(!is_cast_double_click(&last, 6, Duration::from_millis(200)));
}

#[test]
fn double_click_too_far_apart_is_false() {
    let last = LastSkillPanelClick {
        skill_id: 2,
        at: Duration::from_millis(100),
    };
    assert!(!is_cast_double_click(&last, 2, Duration::from_millis(500)));
}

#[test]
fn can_raise_blocked_without_points() {
    let t = tree(&[(1, node(0, 5, 7))]);
    let staging = SkillPanelStaging::default();
    assert!(!staging.can_raise(1, &t, &status(100, 50), 0));
    assert!(staging.can_raise(1, &t, &status(100, 50), 1));
}

#[test]
fn can_raise_blocked_at_max_level() {
    let t = tree(&[(1, node(5, 5, 7))]);
    let staging = SkillPanelStaging::default();
    assert!(!staging.can_raise(1, &t, &status(100, 50), 99));
}

#[test]
fn can_raise_blocked_when_prereq_unmet() {
    let t = tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]);
    let staging = SkillPanelStaging::default();
    assert!(!staging.can_raise(2, &t, &status(100, 50), 99));
}

#[test]
fn can_raise_allowed_when_prereq_staged_in_same_batch() {
    let t = tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]);
    let mut staging = SkillPanelStaging::default();
    staging.raise(1, &t, &status(100, 50), 99);
    assert!(staging.can_raise(2, &t, &status(100, 50), 99));
}

#[test]
fn can_raise_blocked_when_base_or_job_level_too_low() {
    let t = tree(&[(1, with_levels(node(0, 5, 7), 50, 20))]);
    let staging = SkillPanelStaging::default();
    assert!(!staging.can_raise(1, &t, &status(49, 99), 99));
    assert!(!staging.can_raise(1, &t, &status(99, 19), 99));
    assert!(staging.can_raise(1, &t, &status(50, 20), 99));
}

#[test]
fn false_upgradable_is_overridden_only_by_staged_prerequisites() {
    let t = tree(&[
        (1, node(0, 5, 7)),
        (
            2,
            with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
        ),
    ]);
    let mut staging = SkillPanelStaging::default();

    assert!(!staging.can_raise(2, &t, &status(100, 50), 99));
    staging.raise(1, &t, &status(100, 50), 99);
    assert!(staging.can_raise(2, &t, &status(100, 50), 99));
}

#[test]
fn false_upgradable_stays_blocked_when_server_prerequisites_pass() {
    let t = tree(&[
        (1, node(1, 5, 7)),
        (
            2,
            with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
        ),
    ]);

    assert!(!SkillPanelStaging::default().can_raise(2, &t, &status(100, 50), 99));
}

#[test]
fn false_upgradable_does_not_override_other_neutral_gates() {
    let mut t = tree(&[
        (1, node(0, 5, 7)),
        (
            2,
            with_upgradable(
                with_levels(with_requires(node(0, 5, 7), vec![(1, 1)]), 50, 20),
                false,
            ),
        ),
    ]);
    let mut staging = SkillPanelStaging::default();
    staging.raise(1, &t, &status(100, 50), 99);

    assert!(!staging.can_raise(2, &t, &status(49, 20), 99));
    assert!(!staging.can_raise(2, &t, &status(50, 19), 99));
    assert!(!staging.can_raise(2, &t, &status(50, 20), 1));

    t.skills.get_mut(&2).unwrap().level = 5;
    assert!(!staging.can_raise(2, &t, &status(50, 20), 99));
}

#[test]
fn lowering_a_staged_prerequisite_requires_lowering_dependents_first() {
    let t = tree(&[
        (1, node(0, 5, 7)),
        (
            2,
            with_upgradable(with_requires(node(0, 5, 7), vec![(1, 1)]), false),
        ),
    ]);
    let mut staging = SkillPanelStaging::default();
    staging.raise(1, &t, &status(100, 50), 99);
    staging.raise(2, &t, &status(100, 50), 99);

    assert!(!staging.can_lower(1, &t));
    staging.lower(1, &t);
    assert_eq!(staging.staged(1), 1);

    staging.lower(2, &t);
    assert!(staging.can_lower(1, &t));
    staging.lower(1, &t);
    assert!(staging.is_empty());
}

#[test]
fn lower_clamps_at_zero_and_removes_entry() {
    let t = tree(&[(1, node(0, 5, 7))]);
    let mut staging = SkillPanelStaging::default();
    staging.lower(1, &t);
    assert_eq!(staging.staged(1), 0);
    assert!(staging.is_empty());

    staging.raise(1, &t, &status(100, 50), 99);
    staging.raise(1, &t, &status(100, 50), 99);
    assert_eq!(staging.staged(1), 2);
    staging.lower(1, &t);
    assert_eq!(staging.staged(1), 1);
    staging.lower(1, &t);
    assert_eq!(staging.staged(1), 0);
    assert!(staging.is_empty());
}

#[test]
fn lower_never_changes_the_authoritative_level() {
    let t = tree(&[(1, node(2, 5, 7))]);
    let mut staging = SkillPanelStaging::default();
    staging.raise(1, &t, &status(100, 50), 99);

    staging.lower(1, &t);

    assert_eq!(staging.effective_level(1, &t), 2);
    assert!(!staging.can_lower(1, &t));
}

#[test]
fn points_left_is_plain_subtraction() {
    let t = tree(&[(1, node(0, 5, 7)), (2, node(0, 5, 7))]);
    let mut staging = SkillPanelStaging::default();
    staging.raise(1, &t, &status(100, 50), 10);
    staging.raise(1, &t, &status(100, 50), 10);
    staging.raise(2, &t, &status(100, 50), 10);
    assert_eq!(staging.spent(), 3);
    assert_eq!(staging.points_left(10), 7);
    assert_eq!(staging.points_left(2), 0);
}

#[test]
fn apply_order_emits_prereq_before_dependent() {
    let t = tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]);
    let pending = HashMap::from([(1, 1), (2, 1)]);
    assert_eq!(apply_order(&pending, &t), vec![1, 2]);
}

#[test]
fn apply_order_repeats_per_staged_level() {
    let t = tree(&[(1, node(0, 5, 7))]);
    let pending = HashMap::from([(1, 3)]);
    assert_eq!(apply_order(&pending, &t), vec![1, 1, 1]);
}

#[test]
fn apply_order_does_not_hang_on_cycle() {
    let t = tree(&[
        (1, with_requires(node(0, 5, 7), vec![(2, 1)])),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]);
    let pending = HashMap::from([(1, 1), (2, 1)]);
    assert_eq!(apply_order(&pending, &t).len(), 2);
}

fn gates(job_level: u32, skill_point: u32) -> SkillGateSnapshot {
    SkillGateSnapshot {
        values: Some(SkillGates {
            base_level: 99,
            job_level,
            skill_point,
        }),
    }
}

#[test]
fn block_reason_names_the_gate_that_refuses_a_raise() {
    let staging = SkillPanelStaging::default();
    let reason = |t: &SkillTreeState, gates: SkillGateSnapshot| gates.block_reason(2, t, &staging);
    let open = tree(&[(2, node(0, 5, 7))]);

    assert_eq!(reason(&open, gates(50, 3)), None);
    assert!(gates(50, 3).can_raise(2, &open, &staging));
    assert_eq!(
        reason(&open, gates(50, 0)),
        Some(("No skill points left".to_string(), true))
    );
    assert_eq!(reason(&tree(&[(2, node(5, 5, 7))]), gates(50, 0)), None);
    assert_eq!(
        reason(
            &tree(&[(2, with_requires(node(0, 5, 7), vec![(1, 1)]))]),
            gates(50, 3)
        ),
        Some(("Requirements not met yet".to_string(), true))
    );
    assert_eq!(
        reason(
            &tree(&[(2, with_levels(node(0, 5, 7), 0, 60))]),
            gates(50, 3)
        ),
        Some(("Needs Job Lv 60".to_string(), true))
    );
    assert_eq!(
        reason(
            &tree(&[(2, with_upgradable(node(0, 5, 7), false))]),
            gates(50, 3)
        ),
        Some(("Not learned with skill points".to_string(), false))
    );
}

/// Job 1 holds skills 10 and 11; job 7 holds 20, which needs 10 at level 3, and 21,
/// which needs 20.
fn two_jobs() -> SkillTreeState {
    tree(&[
        (10, node(0, 5, 1)),
        (11, node(0, 5, 1)),
        (20, with_requires(node(0, 5, 7), vec![(10, 3)])),
        (21, with_requires(node(0, 5, 7), vec![(20, 1)])),
    ])
}

#[test]
fn hover_rings_the_requirement_chain_and_dims_the_rest() {
    let t = tree(&[
        (10, node(1, 5, 7)),
        (20, with_requires(node(0, 5, 7), vec![(10, 1)])),
        (30, node(5, 5, 7)),
        (40, with_requires(node(0, 5, 7), vec![(30, 6)])),
    ]);
    let ui = SkillPanelUi {
        hovered: Some(20),
        ..default()
    };
    let looks = looks::looks(&t, &ui, &SkillPanelStaging::default(), &gates(50, 3));

    assert_eq!(looks.chain, std::collections::HashSet::from([10, 20]));
    assert_eq!(looks.skills[&10].edge, Edge::Chain);
    assert_eq!(looks.skills[&20].edge, Edge::Hover);
    assert_eq!(looks.skills[&20].dim, Dim::None);
    assert_eq!(looks.skills[&30].dim, Dim::Unrelated);
    assert!(looks.skills[&40].locked);

    let resting = looks::looks(
        &t,
        &SkillPanelUi::default(),
        &SkillPanelStaging::default(),
        &gates(50, 3),
    );
    assert_eq!(resting.skills[&30].edge, Edge::Max);
    assert_eq!(resting.skills[&30].dim, Dim::None);
    assert!(resting.chain.is_empty());
}

#[test]
fn filter_fades_the_other_form_and_other_tabs_report_their_plan() {
    let t = tree(&[
        (10, node(0, 5, 1)),
        (20, node(0, 5, 7)),
        (21, with_inf(node(0, 5, 7), 1)),
    ]);
    let ui = SkillPanelUi {
        filter: SkillFilter::Passive,
        ..default()
    };
    let staging = SkillPanelStaging {
        pending: HashMap::from([(10, 1), (20, 1)]),
    };
    let looks = looks::looks(&t, &ui, &staging, &gates(50, 3));

    assert_eq!(looks.skills.len(), 2, "only the open job is painted");
    assert_eq!(looks.skills[&20].dim, Dim::None);
    assert_eq!(looks.skills[&21].dim, Dim::Faded);
    assert_eq!(looks.planned_jobs, std::collections::HashSet::from([1]));
}

#[test]
fn details_explain_an_unmet_requirement_from_another_job() {
    let t = two_jobs();
    let (staging, gates, cast_levels) = (
        SkillPanelStaging::default(),
        gates(50, 3),
        SkillCastLevels::default(),
    );
    let sources = details::Sources {
        tree: &t,
        staging: &staging,
        gates: &gates,
        cast_levels: &cast_levels,
        catalog: None,
        registry: None,
    };
    let view = details::detail_view(20, &sources).expect("a tree skill");

    assert!(view.locked);
    assert_eq!(view.category, "PASSIVE");
    assert_eq!(
        view.block,
        Some(("Requirements not met yet".to_string(), true))
    );
    assert_eq!(view.requires[0].label, "#10 \u{b7} Job #1 Lv 3");
    assert_eq!(view.requires[0].status, "Lv 0 of 3");
    assert_eq!(view.cast, None);
    assert!(details::detail_view(999, &sources).is_none());
}

#[test]
fn an_active_skill_above_level_one_offers_its_cast_level() {
    let t = tree(&[(42, with_inf(node(5, 10, 7), 1))]);
    let (staging, gates) = (SkillPanelStaging::default(), gates(50, 3));
    let mut cast_levels = SkillCastLevels::default();
    cast_levels.set(42, 3, 5);
    let sources = details::Sources {
        tree: &t,
        staging: &staging,
        gates: &gates,
        cast_levels: &cast_levels,
        catalog: None,
        registry: None,
    };
    let view = details::detail_view(42, &sources).expect("a tree skill");

    assert_eq!(view.category, "ACTIVE \u{b7} OFFENSIVE");
    assert_eq!(
        view.cast,
        Some(details::CastView {
            level: 3,
            learned: 5
        })
    );
    assert!(view.stats.contains(&("Target", "Enemy".to_string())));
}

#[test]
fn details_time_a_skill_at_its_cast_level() {
    let mut bolt = with_inf(node(3, 10, 7), 1);
    bolt.cast_time = vec![800, 1200, 1600];
    bolt.cooldown = vec![0, 0, 2500];
    let t = tree(&[(19, bolt), (42, with_inf(node(1, 10, 7), 1))]);
    let (staging, gates) = (SkillPanelStaging::default(), gates(50, 3));
    let mut cast_levels = SkillCastLevels::default();
    let timing = |skill, cast_levels: &SkillCastLevels| {
        let sources = details::Sources {
            tree: &t,
            staging: &staging,
            gates: &gates,
            cast_levels,
            catalog: None,
            registry: None,
        };
        let stats = details::detail_view(skill, &sources).unwrap().stats;
        let value = |label| {
            stats
                .iter()
                .find(|(name, _)| *name == label)
                .unwrap()
                .1
                .clone()
        };
        (value("Cast"), value("Cooldown"))
    };

    assert_eq!(timing(19, &cast_levels), ("1.6s".into(), "2.5s".into()));
    assert_eq!(
        timing(42, &cast_levels),
        ("Instant".into(), "\u{2014}".into()),
        "the server lists no timing for it"
    );

    cast_levels.set(19, 2, 3);
    assert_eq!(timing(19, &cast_levels), ("1.2s".into(), "\u{2014}".into()));
}

fn skills_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.init_asset::<Font>();
    app.init_resource::<SkillTreeState>();
    app.init_resource::<SkillCastLevels>();
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<SkillPanelStaging>();
    app.init_resource::<SkillGateSnapshot>();
    app.init_resource::<SkillStructureFingerprint>();
    app.init_resource::<looks::SkillLooks>();
    app.add_systems(Update, pipeline());
    app.world_mut().spawn(SkillsBody);
    app.world_mut().spawn((SkillsDetails, Node::default()));
    app.world_mut().spawn(SkillsFooter);
    app.world_mut().spawn((
        CharacterStatus {
            base_level: 99,
            job_level: 50,
            skill_point: 5,
            ..default()
        },
        LocalPlayer,
    ));
    app
}

fn cells(app: &mut App) -> HashMap<u32, Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &SkillPanelCell)>()
        .iter(world)
        .map(|(entity, cell)| (cell.0, entity))
        .collect()
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

fn viewport(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<SkillCanvasViewport>>()
        .single(world)
        .expect("one tree viewport")
}

/// The button whose label reads `label`.
fn button(app: &mut App, label: &str) -> Entity {
    let world = app.world_mut();
    world
        .query::<(&Text, &ChildOf)>()
        .iter(world)
        .find_map(|(text, parent)| (text.0 == label).then_some(parent.parent()))
        .unwrap_or_else(|| panic!("no {label:?} button"))
}

fn click_event(target: Entity, window: Entity, button: PointerButton) -> PointerClick {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::WindowRef;
    PointerClick {
        entity: target,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
        ),
        button,
        hit: HitData::new(target, 0.0, None, None),
        duration: Duration::ZERO,
        count: 1,
    }
}

fn drag_start_event(target: Entity, window: Entity) -> PointerDragStart {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::WindowRef;
    PointerDragStart {
        entity: target,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
        ),
        button: PointerButton::Primary,
        hit: HitData::new(target, 0.0, None, None),
    }
}

#[test]
fn the_last_job_opens_first_and_a_tab_click_opens_another() {
    let mut app = skills_app();
    app.insert_resource(two_jobs());
    app.update();

    let mut shown: Vec<u32> = cells(&mut app).into_keys().collect();
    shown.sort_unstable();
    assert_eq!(shown, [20, 21]);
    let tabs: HashMap<u32, Entity> = {
        let world = app.world_mut();
        world
            .query::<(Entity, &SkillJobTab)>()
            .iter(world)
            .map(|(entity, tab)| (tab.0, entity))
            .collect()
    };
    assert_eq!(tabs.len(), 2);
    for label in ["Job #1", "Job #7", "All", "Active", "Passive"] {
        assert!(texts(&mut app).contains(&label.to_string()), "{label}");
    }

    let window = app.world_mut().spawn(Window::default()).id();
    app.world_mut()
        .trigger(click_event(tabs[&1], window, PointerButton::Primary));
    app.update();

    let mut shown: Vec<u32> = cells(&mut app).into_keys().collect();
    shown.sort_unstable();
    assert_eq!(shown, [10, 11]);
    assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(10));
}

#[test]
fn planning_repaints_the_tree_in_place() {
    let mut app = skills_app();
    app.insert_resource(tree(&[(1, node(1, 5, 7))]));
    app.update();
    let cell = cells(&mut app)[&1];
    let steppers = |app: &mut App| -> HashMap<bool, Visibility> {
        let world = app.world_mut();
        world
            .query::<(&SkillPanelStepper, &Visibility)>()
            .iter(world)
            .map(|(stepper, visibility)| (stepper.raise, *visibility))
            .collect()
    };
    assert_eq!(steppers(&mut app)[&true], Visibility::Inherited);
    assert_eq!(steppers(&mut app)[&false], Visibility::Hidden);

    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 2)]);
    app.update();

    assert_eq!(cells(&mut app)[&1], cell, "the tree is not respawned");
    assert_eq!(steppers(&mut app)[&false], Visibility::Inherited);
    let world = app.world_mut();
    let level = world
        .query::<(&SkillNodeLevel, &Text)>()
        .single(world)
        .expect("one level badge");
    assert_eq!(level.1.0, "3/5");
    let mut pips: Vec<(u32, Color)> = world
        .query::<(&SkillPip, &BackgroundColor)>()
        .iter(world)
        .map(|(pip, fill)| (pip.index, fill.0))
        .collect();
    pips.sort_unstable_by_key(|(index, _)| *index);
    let fills: Vec<Color> = pips.into_iter().map(|(_, fill)| fill).collect();
    assert_eq!(
        fills,
        [
            ro_dark::BASE_EXP,
            ro_dark::FROST_GLOW,
            ro_dark::FROST_GLOW,
            ro_dark::BAR_TROUGH,
            ro_dark::BAR_TROUGH,
        ]
    );
    let texts = texts(&mut app);
    assert!(texts.contains(&"+2 planned".to_string()), "{texts:?}");
    assert!(texts.contains(&"3".to_string()), "points left: {texts:?}");
}

#[test]
fn a_requirement_line_is_dashed_until_met_and_frost_inside_the_focus_chain() {
    let mut app = skills_app();
    app.insert_resource(tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]));
    app.update();
    let line = |app: &mut App| -> (Visibility, Color) {
        let world = app.world_mut();
        let (_, visibility, fill) = world
            .query::<(&SkillLine, &Visibility, &BackgroundColor)>()
            .single(world)
            .expect("a straight link is one segment");
        (*visibility, fill.0)
    };
    assert_eq!(line(&mut app).0, Visibility::Hidden);

    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);
    app.update();
    assert_eq!(line(&mut app), (Visibility::Inherited, ro_dark::DIVIDER));

    app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(2);
    app.update();
    assert_eq!(line(&mut app), (Visibility::Inherited, ro_dark::FROST_GLOW));
}

#[test]
fn the_tree_keeps_its_scroll_until_another_job_opens() {
    let mut app = skills_app();
    app.insert_resource(two_jobs());
    app.update();
    let first = viewport(&mut app);
    app.world_mut()
        .entity_mut(first)
        .insert(ScrollPosition(Vec2::new(0.0, 40.0)));

    app.world_mut().resource_mut::<SkillPanelUi>().filter = SkillFilter::Passive;
    app.update();
    let second = viewport(&mut app);
    assert_ne!(first, second);
    assert_eq!(app.world().get::<ScrollPosition>(second).unwrap().y, 40.0);

    app.world_mut().resource_mut::<SkillPanelUi>().job = Some(1);
    app.update();
    let third = viewport(&mut app);
    assert_eq!(app.world().get::<ScrollPosition>(third).unwrap().y, 0.0);
}

#[test]
fn details_follow_the_selection_and_a_requirement_opens_its_job() {
    let mut app = skills_app();
    app.insert_resource(two_jobs());
    app.update();
    let hint = "Select a skill to see its details".to_string();
    assert!(texts(&mut app).contains(&hint));

    app.world_mut().resource_mut::<SkillPanelUi>().selected = Some(20);
    app.update();
    let shown = texts(&mut app);
    assert!(!shown.contains(&hint), "{shown:?}");
    for expected in ["Lv 0 / 5", "Requirements not met yet", "Lv 0 of 3"] {
        assert!(
            shown.contains(&expected.to_string()),
            "{expected}: {shown:?}"
        );
    }

    let link = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<RequirementLink>>()
            .single(world)
            .expect("one requirement")
    };
    let window = app.world_mut().spawn(Window::default()).id();
    app.world_mut()
        .trigger(click_event(link, window, PointerButton::Primary));

    let ui = app.world().resource::<SkillPanelUi>();
    assert_eq!((ui.job, ui.selected), (Some(1), Some(10)));
}

#[test]
fn rendered_cell_casts_on_double_click_inspects_and_arms_the_hotbar() {
    let mut app = skills_app();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<VendingIntent>();
    app.add_message::<SkillCastRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<HotbarDrag>();
    app.insert_resource(tree(&[(42, node(1, 5, 7))]));
    let window = app.world_mut().spawn(Window::default()).id();
    app.update();
    let cell = cells(&mut app)[&42];

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Primary));
    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Primary));
    assert_eq!(
        app.world()
            .resource::<Messages<SkillCastRequested>>()
            .iter_current_update_messages()
            .count(),
        1
    );

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Secondary));
    assert!(
        app.world()
            .resource::<Messages<ShowInfoModal>>()
            .iter_current_update_messages()
            .any(|message| matches!(
                &message.content,
                InfoContent::Skill {
                    raise: Some(42),
                    ..
                }
            ))
    );

    app.world_mut().trigger(drag_start_event(cell, window));
    assert_eq!(
        app.world().resource::<HotbarDrag>().payload,
        Some(HotbarSlot::Skill(42))
    );
}

#[test]
fn rendered_reset_plan_discards_staging_without_a_request() {
    let mut app = skills_app();
    app.insert_resource(tree(&[(1, node(0, 5, 7))]));
    app.update();
    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);
    app.update();

    let reset = button(&mut app, "Reset Plan");
    app.world_mut().trigger(Activate { entity: reset });

    assert!(app.world().resource::<SkillPanelStaging>().is_empty());
}

#[test]
fn cast_steps_move_the_cast_level_within_the_learned_range() {
    let mut app = App::new();
    app.init_resource::<SkillCastLevels>();
    app.insert_resource(tree(&[(42, node(3, 10, 7))]));
    let mut step = |up: bool| {
        app.world_mut()
            .spawn(CastStep { skill_id: 42, up })
            .observe(on_cast_step)
            .id()
    };
    let (down, up) = (step(false), step(true));
    let level = |app: &App| app.world().resource::<SkillCastLevels>().level(42, 3);

    for _ in 0..5 {
        app.world_mut().trigger(Activate { entity: down });
    }
    assert_eq!(level(&app), 1);
    app.world_mut().trigger(Activate { entity: up });
    assert_eq!(level(&app), 2);
    for _ in 0..5 {
        app.world_mut().trigger(Activate { entity: up });
    }
    assert_eq!(level(&app), 3);
}

#[test]
fn secondary_stepper_click_bubbles_to_cell_without_changing_staging() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    app.add_message::<SkillCastRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<SkillPanelStaging>();
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<Time>();
    app.insert_resource(tree(&[(1, node(0, 5, 7))]));
    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1)]);
    let cell = app
        .world_mut()
        .spawn(SkillPanelCell(1))
        .observe(on_cell_click)
        .id();
    let stepper = app
        .world_mut()
        .spawn((
            SkillPanelStepper {
                skill_id: 1,
                raise: false,
            },
            ChildOf(cell),
        ))
        .observe(on_stepper)
        .id();
    let window = app.world_mut().spawn(Window::default()).id();
    app.world_mut().flush();

    app.world_mut()
        .trigger(click_event(stepper, window, PointerButton::Secondary));

    assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);
    let messages = app.world().resource::<Messages<ShowInfoModal>>();
    assert!(
        messages
            .iter_current_update_messages()
            .any(|message| matches!(&message.content, InfoContent::Skill { raise: Some(1), .. }))
    );
}

#[test]
fn primary_stepper_click_changes_staging_without_reaching_the_cell() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    app.add_message::<SkillCastRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<SkillPanelStaging>();
    app.init_resource::<SkillPanelUi>();
    app.insert_resource(LastSkillPanelClick {
        skill_id: 1,
        at: Duration::ZERO,
    });
    app.init_resource::<Time>();
    app.insert_resource(tree(&[(1, node(0, 5, 7))]));
    app.world_mut().spawn((
        CharacterStatus {
            base_level: 1,
            job_level: 1,
            skill_point: 1,
            ..default()
        },
        LocalPlayer,
    ));
    let cell = app
        .world_mut()
        .spawn(SkillPanelCell(1))
        .observe(on_cell_click)
        .id();
    let stepper = app
        .world_mut()
        .spawn((
            SkillPanelStepper {
                skill_id: 1,
                raise: true,
            },
            ChildOf(cell),
        ))
        .observe(on_stepper)
        .id();
    let window = app.world_mut().spawn(Window::default()).id();
    app.world_mut().flush();

    app.world_mut()
        .trigger(click_event(stepper, window, PointerButton::Primary));

    assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);
    assert_eq!(app.world().resource::<SkillPanelUi>().selected, None);
    assert_eq!(
        app.world()
            .resource::<Messages<SkillCastRequested>>()
            .iter_current_update_messages()
            .count(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<Messages<ShowInfoModal>>()
            .iter_current_update_messages()
            .count(),
        0
    );
}

#[test]
fn decrement_observer_rejects_prerequisite_until_dependent_is_removed() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    app.init_resource::<SkillPanelStaging>();
    app.insert_resource(tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]));
    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 1), (2, 1)]);
    let prerequisite = app
        .world_mut()
        .spawn(SkillPanelStepper {
            skill_id: 1,
            raise: false,
        })
        .observe(on_stepper)
        .id();
    let dependent = app
        .world_mut()
        .spawn(SkillPanelStepper {
            skill_id: 2,
            raise: false,
        })
        .observe(on_stepper)
        .id();
    let window = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(click_event(prerequisite, window, PointerButton::Primary));
    assert_eq!(app.world().resource::<SkillPanelStaging>().staged(1), 1);

    app.world_mut()
        .trigger(click_event(dependent, window, PointerButton::Primary));
    app.world_mut()
        .trigger(click_event(prerequisite, window, PointerButton::Primary));
    assert!(app.world().resource::<SkillPanelStaging>().is_empty());
}

#[test]
fn on_apply_emits_ordered_events_and_clears() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SkillLearnRequested>();
    app.init_resource::<SkillPanelStaging>();
    app.insert_resource(tree(&[
        (1, node(0, 5, 7)),
        (2, with_requires(node(0, 5, 7), vec![(1, 1)])),
    ]));
    app.world_mut().resource_mut::<SkillPanelStaging>().pending = HashMap::from([(1, 2), (2, 1)]);

    let button = app.world_mut().spawn_empty().observe(on_apply).id();
    app.world_mut().trigger(Activate { entity: button });
    app.update();

    let messages = app.world().resource::<Messages<SkillLearnRequested>>();
    let mut reader = messages.get_cursor();
    let learned: Vec<u32> = reader.read(messages).map(|m| m.skill_id).collect();

    assert_eq!(learned, vec![1, 1, 2]);
    assert!(app.world().resource::<SkillPanelStaging>().is_empty());
}

#[test]
fn secondary_click_on_cell_opens_info_modal_without_selecting() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SkillCastRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<Time>();
    app.init_resource::<SkillPanelStaging>();
    app.insert_resource(tree(&[(42, node(1, 5, 7))]));

    let cell = app
        .world_mut()
        .spawn(SkillPanelCell(42))
        .observe(on_cell_click)
        .id();
    let window = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Secondary));

    let messages = app.world().resource::<Messages<ShowInfoModal>>();
    let mut reader = messages.get_cursor();
    let contents: Vec<_> = reader.read(messages).map(|m| m.content.clone()).collect();
    assert_eq!(contents.len(), 1);
    assert!(matches!(
        &contents[0],
        InfoContent::Skill { view, raise: Some(42) } if view.level_line == "1/5"
    ));
    assert_eq!(app.world().resource::<SkillPanelUi>().selected, None);
}

#[test]
fn primary_click_on_cell_still_selects_and_does_not_open_the_modal() {
    let mut app = App::new();
    app.init_resource::<lifthrasir_client::domain::vending::VendingState>();
    app.add_message::<lifthrasir_client::domain::vending::VendingIntent>();
    app.add_plugins(MinimalPlugins);
    app.add_message::<SkillCastRequested>();
    app.add_message::<ShowInfoModal>();
    app.init_resource::<SkillPanelUi>();
    app.init_resource::<LastSkillPanelClick>();
    app.init_resource::<Time>();
    app.init_resource::<SkillPanelStaging>();
    app.insert_resource(tree(&[(42, node(1, 5, 7))]));

    let cell = app
        .world_mut()
        .spawn(SkillPanelCell(42))
        .observe(on_cell_click)
        .id();
    let window = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(click_event(cell, window, PointerButton::Primary));

    assert_eq!(app.world().resource::<SkillPanelUi>().selected, Some(42));
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<ShowInfoModal>>()
            .drain()
            .count(),
        0
    );
}
