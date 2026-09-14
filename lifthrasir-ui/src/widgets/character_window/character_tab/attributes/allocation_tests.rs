use super::*;
use net_contract::events::StatRaised;

fn click(target: Entity) -> Pointer<Click> {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::WindowRef;

    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Primary.normalize(Some(target)).unwrap(),
            ),
            position: Vec2::ZERO,
        },
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(target, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        },
        target,
    )
}

fn allocation_app() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .insert_state(GameState::InGame)
        .add_message::<StatIncreaseRequested>()
        .add_message::<StatRaised>();
    register(&mut app);
    app.world_mut().spawn((
        CharacterStatus {
            str: 10,
            agi: 10,
            status_point: 100,
            ..default()
        },
        LocalPlayer,
    ));
    let save = app.world_mut().spawn_empty().observe(on_char_save).id();
    let raise = app
        .world_mut()
        .spawn(CharStepper {
            stat: StatusParameter::Str,
            raise: true,
        })
        .observe(on_char_stepper)
        .id();
    (app, save, raise)
}

#[test]
fn unrelated_and_duplicate_replies_do_not_unlock_other_pending_stats() {
    let (mut app, save, raise) = allocation_app();
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    for stat_id in [
        StatusParameter::Agi as u32,
        u32::MAX,
        65_536 + StatusParameter::Str as u32,
    ] {
        app.world_mut().write_message(StatRaised {
            stat_id,
            ok: false,
            value: 0,
        });
    }
    app.update();
    app.world_mut().trigger(click(raise));
    assert!(app.world().resource::<CharStatStaging>().is_empty());

    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Str as u32,
        ok: false,
        value: 10,
    });
    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Str as u32,
        ok: false,
        value: 10,
    });
    let feedback = app
        .world_mut()
        .spawn((Text::new(""), CharAllocationFeedback))
        .id();
    app.update();
    assert_eq!(
        app.world()
            .get::<Text>(feedback)
            .unwrap()
            .0
            .matches("rejected")
            .count(),
        1
    );
}

#[test]
fn authoritative_param_updates_are_not_reapplied_from_the_acknowledgment() {
    use game_engine::domain::entities::{
        character::events::StatusParameterChanged,
        character::systems::status_update::{PendingStatusParams, update_character_status_system},
        registry::EntityRegistry,
    };
    use net_contract::events::ParamChanged;

    let (mut app, save, raise) = allocation_app();
    app.init_resource::<PendingStatusParams>()
        .init_resource::<EntityRegistry>()
        .add_message::<StatusParameterChanged>()
        .add_message::<ParamChanged>()
        .add_systems(
            Update,
            update_character_status_system.before(consume_stat_results),
        );
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Str as u32,
        ok: true,
        value: 11,
    });
    for (param, value) in [
        (StatusParameter::Str, 11),
        (StatusParameter::StatusPoint, 98),
    ] {
        app.world_mut().write_message(ParamChanged {
            var: param as u32,
            value,
        });
    }
    app.update();
    let player = app
        .world_mut()
        .query_filtered::<&CharacterStatus, With<LocalPlayer>>()
        .single(app.world())
        .unwrap();
    assert_eq!((player.str, player.status_point), (11, 98));
    app.world_mut().trigger(click(raise));
    assert_eq!(
        app.world()
            .resource::<CharStatStaging>()
            .staged_value(StatusParameter::Str),
        1
    );
}

#[test]
fn leaving_gameplay_clears_pending_allocations_before_the_next_session() {
    let (mut app, save, raise) = allocation_app();
    app.update();
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::CharacterSelection);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();
    app.world_mut().trigger(click(raise));
    assert_eq!(
        app.world()
            .resource::<CharStatStaging>()
            .staged_value(StatusParameter::Str),
        1
    );
}

#[test]
fn pending_allocations_dim_the_locked_stepper() {
    let (mut app, save, raise) = allocation_app();
    app.world_mut()
        .entity_mut(raise)
        .insert(BackgroundColor(theme::FIELD));
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    app.update();
    assert!(app.world().get::<BackgroundColor>(raise).unwrap().0.alpha() < 1.0);
}

#[test]
fn mixed_results_keep_rejection_visible_and_wait_for_every_submitted_stat() {
    let (mut app, save, raise) = allocation_app();
    let feedback = app
        .world_mut()
        .spawn((Text::new(""), CharAllocationFeedback))
        .id();
    app.world_mut().trigger(click(raise));
    app.world_mut().resource_mut::<CharStatStaging>().raise(
        StatusParameter::Agi,
        100,
        &HashMap::from([(StatusParameter::Str, 10), (StatusParameter::Agi, 10)]),
    );
    app.world_mut().trigger(click(save));
    app.update();
    assert!(!app.world().get::<Text>(feedback).unwrap().0.is_empty());

    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Str as u32,
        ok: false,
        value: 10,
    });
    app.update();
    let rejected = &app.world().get::<Text>(feedback).unwrap().0;
    assert!(rejected.contains("STR") && rejected.contains("rejected"));
    app.world_mut().trigger(click(raise));
    assert!(app.world().resource::<CharStatStaging>().is_empty());

    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Agi as u32,
        ok: true,
        value: 11,
    });
    app.update();
    assert!(
        app.world()
            .get::<Text>(feedback)
            .unwrap()
            .0
            .contains("rejected")
    );
    app.world_mut().trigger(click(raise));
    assert!(!app.world().resource::<CharStatStaging>().is_empty());
}

#[test]
fn acknowledgment_unlocks_editing_without_applying_its_capped_value() {
    let (mut app, save, raise) = allocation_app();
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    let mut cursor = app
        .world()
        .resource::<Messages<StatIncreaseRequested>>()
        .get_cursor_current();
    app.world_mut().write_message(StatRaised {
        stat_id: StatusParameter::Str as u32,
        ok: true,
        value: 255,
    });
    app.update();
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    assert_eq!(
        cursor
            .read(app.world().resource::<Messages<StatIncreaseRequested>>())
            .count(),
        1
    );
    let player = app
        .world_mut()
        .query_filtered::<&CharacterStatus, With<LocalPlayer>>()
        .single(app.world())
        .unwrap();
    assert_eq!((player.str, player.status_point), (10, 100));
}

#[test]
fn save_blocks_further_allocations_until_acknowledged() {
    let (mut app, save, raise) = allocation_app();
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));
    app.world_mut().trigger(click(raise));
    app.world_mut().trigger(click(save));

    let messages = app.world().resource::<Messages<StatIncreaseRequested>>();
    assert_eq!(messages.len(), 1);
    let player = app
        .world_mut()
        .query_filtered::<&CharacterStatus, With<LocalPlayer>>()
        .single(app.world())
        .unwrap();
    assert_eq!((player.str, player.status_point), (10, 100));
}
