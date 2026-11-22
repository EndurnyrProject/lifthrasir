use super::*;

const GEN: ZoneSessionGeneration = ZoneSessionGeneration(7);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((net_contract::NetContractPlugin, HomunculusPlugin))
        .insert_resource(GEN)
        .init_resource::<EntityRegistry>()
        .insert_resource(Time::<Real>::default());
    app
}

fn ingest(app: &mut App, sequence: u32, payload: Ingress) {
    app.world_mut().write_message(HomunculusIngress {
        generation: GEN,
        sequence,
        payload,
    });
}

fn snap(durable_id: u64) -> HomunculusSnapshot {
    HomunculusSnapshot {
        durable_id,
        name: "Lif".into(),
        ..default()
    }
}

fn state(app: &App) -> &HomunculusState {
    app.world().resource::<HomunculusState>()
}

fn ready(app: &mut App) {
    app.world_mut()
        .write_message(HomunculusReady { generation: GEN });
}

#[test]
fn removal_and_warp_reject_only_stale_states() {
    let mut app = app();
    ready(&mut app);
    ingest(&mut app, 0, Ingress::State(snap(1)));
    app.update();
    assert_eq!(state(&app).snapshot().unwrap().durable_id, 1);

    ingest(&mut app, 9, Ingress::Removed { durable_id: 1 });
    ingest(&mut app, 8, Ingress::State(snap(1)));
    app.update();
    assert!(matches!(state(&app).presence(), HomunculusPresence::Absent));

    ingest(&mut app, 14, Ingress::State(snap(2)));
    ingest(&mut app, 12, Ingress::MapBarrier);
    ingest(&mut app, 10, Ingress::State(snap(1)));
    app.update();
    assert_eq!(state(&app).snapshot().unwrap().durable_id, 2);
    assert!(!state(&app).can_mutate());

    app.world_mut().insert_resource(ZoneSessionGeneration(8));
    ingest(&mut app, 20, Ingress::State(snap(3)));
    app.update();
    assert!(matches!(
        state(&app).presence(),
        HomunculusPresence::Unknown
    ));
}

#[test]
fn result_feedback_is_correlated_separately_from_state() {
    let mut app = app();
    ready(&mut app);
    ingest(&mut app, 2, Ingress::State(snap(1)));
    app.world_mut().write_message(HomunculusIntent::Act {
        durable_id: 1,
        action: Action::Rename { name: "New".into() },
    });
    app.update();
    let id = state(&app).pending_mutation().unwrap();
    let mut newest = snap(1);
    newest.name = "Latest".into();
    ingest(&mut app, 7, Ingress::State(newest));
    ingest(
        &mut app,
        4,
        Ingress::Result {
            request_id: id,
            success: true,
            error: HomunculusError::None,
            state: Some(snap(1)),
        },
    );
    ingest(
        &mut app,
        8,
        Ingress::Result {
            request_id: id + 1,
            success: false,
            error: HomunculusError::WrongChannel,
            state: None,
        },
    );
    app.update();
    assert_eq!(state(&app).snapshot().unwrap().name, "Latest");
    assert_eq!(
        state(&app).last_feedback(),
        Some(&HomunculusFeedbackKind::Completed)
    );
    assert!(state(&app).pending_mutation().is_none());
}

#[test]
fn mutation_timeout_inspects_without_mutation_replay() {
    let mut app = app();
    ready(&mut app);
    ingest(&mut app, 1, Ingress::State(snap(1)));
    app.world_mut().write_message(HomunculusIntent::Act {
        durable_id: 1,
        action: Action::Delete { confirmed: true },
    });
    app.update();
    let id = state(&app).pending_mutation().unwrap();
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(TIMEOUT);
    app.update();
    let commands = app
        .world()
        .resource::<Messages<HomunculusCommand>>()
        .iter_current_update_messages()
        .collect::<Vec<_>>();
    let [inspect] = commands.as_slice() else {
        panic!("expected just the recovery Inspect")
    };
    assert_eq!(inspect.action, Action::Inspect);
    assert_ne!(inspect.request_id, id);
    assert!(!state(&app).can_mutate());
    assert_eq!(
        state(&app).last_feedback(),
        Some(&HomunculusFeedbackKind::TimedOut)
    );
}
