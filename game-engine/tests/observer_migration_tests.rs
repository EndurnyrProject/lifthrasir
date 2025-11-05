use bevy::prelude::*;

#[derive(EntityEvent, Clone, Debug)]
struct TestEntityEvent {
    #[event_target]
    entity: Entity,
    value: i32,
}

#[derive(Component)]
struct TestComponent {
    counter: i32,
}

#[test]
fn test_observer_registration_and_triggering() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let entity = app.world_mut().spawn(TestComponent { counter: 0 }).id();

    app.world_mut().add_observer(
        |trigger: On<TestEntityEvent>, mut query: Query<&mut TestComponent>| {
            let event = trigger.event();
            if let Ok(mut component) = query.get_mut(event.entity) {
                component.counter += event.value;
            }
        },
    );

    app.world_mut().flush();

    app.world_mut().trigger(TestEntityEvent { entity, value: 5 });

    app.update();

    let component = app.world().get::<TestComponent>(entity).unwrap();
    assert_eq!(component.counter, 5);
}

#[test]
fn test_entity_targeted_observer() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let entity1 = app
        .world_mut()
        .spawn(TestComponent { counter: 0 })
        .observe(
            |trigger: On<TestEntityEvent>, mut query: Query<&mut TestComponent>| {
                let event = trigger.event();
                if let Ok(mut component) = query.get_mut(event.entity) {
                    component.counter += event.value;
                }
            },
        )
        .id();

    let entity2 = app.world_mut().spawn(TestComponent { counter: 0 }).id();

    app.world_mut().flush();

    app.world_mut().trigger(TestEntityEvent {
        entity: entity1,
        value: 10,
    });

    app.update();

    let component1 = app.world().get::<TestComponent>(entity1).unwrap();
    assert_eq!(component1.counter, 10, "entity1 should be updated");

    let component2 = app.world().get::<TestComponent>(entity2).unwrap();
    assert_eq!(component2.counter, 0, "entity2 should not be updated");
}

#[test]
fn test_observer_cleanup_on_despawn() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let entity = app
        .world_mut()
        .spawn(TestComponent { counter: 0 })
        .observe(
            |trigger: On<TestEntityEvent>, mut query: Query<&mut TestComponent>| {
                let event = trigger.event();
                if let Ok(mut component) = query.get_mut(event.entity) {
                    component.counter += event.value;
                }
            },
        )
        .id();

    app.world_mut().flush();

    app.world_mut().trigger(TestEntityEvent { entity, value: 5 });

    app.update();

    let component = app.world().get::<TestComponent>(entity).unwrap();
    assert_eq!(component.counter, 5);

    app.world_mut().despawn(entity);
    app.update();

    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn test_multiple_observers_same_event() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    #[derive(Resource, Default)]
    struct EventLog {
        count: i32,
    }

    app.insert_resource(EventLog::default());

    let entity = app.world_mut().spawn(TestComponent { counter: 0 }).id();

    app.world_mut().add_observer(
        |_trigger: On<TestEntityEvent>, mut log: ResMut<EventLog>| {
            log.count += 1;
        },
    );

    app.world_mut().add_observer(
        |trigger: On<TestEntityEvent>, mut query: Query<&mut TestComponent>| {
            if let Ok(mut component) = query.get_mut(trigger.event().entity) {
                component.counter += trigger.event().value;
            }
        },
    );

    app.world_mut().flush();

    app.world_mut().trigger(TestEntityEvent { entity, value: 3 });

    app.update();

    let component = app.world().get::<TestComponent>(entity).unwrap();
    assert_eq!(component.counter, 3);

    let log = app.world().resource::<EventLog>();
    assert_eq!(log.count, 1, "Global observer should have been triggered");
}

#[derive(Message, Clone, Debug)]
struct TestMessage {
    entity: Entity,
    value: i32,
}

#[test]
fn test_message_vs_observer_equivalence() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_message::<TestMessage>();

    #[derive(Resource, Default)]
    struct MessageLog {
        message_count: i32,
        observer_count: i32,
    }

    app.insert_resource(MessageLog::default());

    let entity = app.world_mut().spawn(TestComponent { counter: 0 }).id();

    app.world_mut()
        .add_observer(|_trigger: On<TestEntityEvent>, mut log: ResMut<MessageLog>| {
            log.observer_count += 1;
        });

    fn message_system(mut reader: MessageReader<TestMessage>, mut log: ResMut<MessageLog>) {
        for _msg in reader.read() {
            log.message_count += 1;
        }
    }

    app.add_systems(Update, message_system);

    app.world_mut().flush();

    app.world_mut().trigger(TestEntityEvent { entity, value: 1 });

    let mut writer = app.world_mut().resource_mut::<Messages<TestMessage>>();
    writer.write(TestMessage { entity, value: 1 });

    app.update();

    let log = app.world().resource::<MessageLog>();
    assert_eq!(log.message_count, 1, "Message should be received");
    assert_eq!(log.observer_count, 1, "Observer should be triggered");
}
