# ECS, App Structure, States & Time (Bevy 0.19)

Distilled from the Bevy 0.19 examples tree. Bevy 0.19: bundles are replaced by required components, buffered events are `Message`s, observers take `On<E>`.

## System Ordering & System Sets

Systems in one schedule run in parallel unless ordered. Order with `.chain()`, `.before()`, `.after()`, or group into `SystemSet`s and order the sets once with `configure_sets`.

```rust
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum MySystems { BeforeRound, Round, AfterRound }

app.configure_sets(
    Update,
    (MySystems::BeforeRound, MySystems::Round, MySystems::AfterRound).chain(),
)
.add_systems(Update, (
    ((new_round, new_player).chain(), exclusive_player_system).in_set(MySystems::BeforeRound),
    score_system.in_set(MySystems::Round),
    (score_check, game_over.after(score_check)).in_set(MySystems::AfterRound),
));
```

`examples/ecs/ecs_guide.rs`

## Ambiguity Detection

Two systems with conflicting data access and no ordering constraint have nondeterministic relative order. Detect this per schedule; silence known false positives with `.ambiguous_with(other)`.

```rust
app.edit_schedule(Update, |schedule| {
    schedule.set_build_settings(ScheduleBuildSettings {
        ambiguity_detection: LogLevel::Warn,
        ..default()
    });
});
```

`examples/ecs/nondeterministic_system_order.rs`

## Run Conditions

Any read-only system returning `bool` is a run condition. Combinators: `.and_then()` (short-circuit &&), `.or_else()` (||), `not(...)`. Common ones from the prelude: `resource_exists::<T>`, `in_state(...)`, `input_just_pressed(...)`, `on_timer(...)`.

```rust
fn has_user_input(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>) -> bool {
    keys.just_pressed(KeyCode::Space) || mouse.just_pressed(MouseButton::Left)
}

// A condition factory (closure with captured config + Local state):
fn time_passed(t: f32) -> impl FnMut(Local<f32>, Res<Time>) -> bool {
    move |mut timer: Local<f32>, time: Res<Time>| { *timer += time.delta_secs(); *timer >= t }
}

app.add_systems(Update, (
    increment.run_if(resource_exists::<Counter>).run_if(has_user_input),
    print.run_if(resource_exists::<Counter>.and_then(|c: Res<Counter>| c.is_changed())),
    banner.run_if(time_passed(2.0)).run_if(not(time_passed(2.5))),
));
```

`examples/ecs/run_conditions.rs`

## Custom Schedules in the Main Loop

Register a `Schedule` with its own `ScheduleLabel`, then splice it into `MainScheduleOrder` relative to built-in schedules. Must be done in `main`, not from a system inside `Main`.

```rust
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct SingleThreadedUpdate;

let mut sched = Schedule::new(SingleThreadedUpdate);
sched.set_executor(SingleThreadedExecutor::new()); // optional
app.add_schedule(sched);
app.world_mut().resource_mut::<MainScheduleOrder>().insert_after(Update, SingleThreadedUpdate);
// Startup-phase schedules use insert_startup_after(PreStartup, MyStartup) instead.
```

`examples/ecs/custom_schedule.rs`, `examples/ecs/custom_executor.rs`

## Fixed Timestep Basics

`FixedUpdate` runs zero-to-many times per frame to catch up. Inside it, `Res<Time>` *is* `Time<Fixed>` (constant delta). Configure the rate with a resource.

```rust
app.add_systems(FixedUpdate, fixed_update)
   .insert_resource(Time::<Fixed>::from_seconds(0.5));

fn fixed_update(time: Res<Time>, fixed: Res<Time<Fixed>>) {
    let dt = time.delta_secs();                       // always the fixed step
    let leftover = fixed.overstep().as_secs_f32();    // time accrued toward next tick
}
```

`examples/ecs/fixed_timestep.rs`

## Fixed Timestep Physics + Render Interpolation

The canonical pattern for movement (very relevant to prediction/steering): simulate in `FixedUpdate` on dedicated position components, gather input every frame *before* the fixed loop, interpolate `Transform` *after* it using `overstep_fraction()`.

```rust
#[derive(Component, Default, Deref, DerefMut)] struct AccumulatedInput { movement: Vec2 }
#[derive(Component, Default, Deref, DerefMut)] struct Velocity(Vec3);
#[derive(Component, Default, Deref, DerefMut)] struct PhysicalTranslation(Vec3);
#[derive(Component, Default, Deref, DerefMut)] struct PreviousPhysicalTranslation(Vec3);

app.add_systems(FixedUpdate, advance_physics)
   .add_systems(RunFixedMainLoop, (
       (rotate_camera, accumulate_input).chain()
           .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
       (clear_input.run_if(did_fixed_timestep_run_this_frame),
        interpolate_rendered_transform, translate_camera).chain()
           .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
   ));

fn advance_physics(t: Res<Time<Fixed>>,
    mut q: Query<(&mut PhysicalTranslation, &mut PreviousPhysicalTranslation, &Velocity)>) {
    for (mut cur, mut prev, v) in &mut q { prev.0 = cur.0; cur.0 += v.0 * t.delta_secs(); }
}

fn interpolate_rendered_transform(t: Res<Time<Fixed>>,
    mut q: Query<(&mut Transform, &PhysicalTranslation, &PreviousPhysicalTranslation)>) {
    let alpha = t.overstep_fraction(); // 0..1 between two fixed ticks
    for (mut tf, cur, prev) in &mut q { tf.translation = prev.lerp(cur.0, alpha); }
}
```

Key rules: input read in `Update`/`FixedUpdate` misses or double-counts frames — read it `BeforeFixedMainLoop`; a "did fixed run this frame" flag resource (cleared in `PreUpdate`, set in `FixedPreUpdate`) gates input clearing.

`examples/movement/physics_in_fixed_timestep.rs`

## Smooth Following (frame-rate-independent damping)

`smooth_nudge` is the built-in exponential-decay follow — great for cameras and remote-entity smoothing.

```rust
fn move_follower(mut follower: Single<&mut Transform, With<Follower>>,
    target: Single<&Transform, (With<Target>, Without<Follower>)>,
    decay: Res<DecayRate>, time: Res<Time>) {
    follower.translation.smooth_nudge(&target.translation, decay.0, time.delta_secs());
}
```

`examples/movement/smooth_follow.rs`

## States

Derive `States`, init with `init_state::<S>()` (or `insert_state(value)`). `OnEnter`/`OnExit` schedules run during `StateTransition` (exit of old first, then enter of new). Gate `Update` systems with `in_state`. Change via `ResMut<NextState<S>>`. State variants may carry data (`GameState::C(u8)`).

```rust
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState { #[default] Menu, InGame }

app.init_state::<AppState>()
   .add_systems(OnEnter(AppState::Menu), setup_menu)
   .add_systems(OnExit(AppState::Menu), cleanup_menu)
   .add_systems(Update, menu.run_if(in_state(AppState::Menu)));

fn menu(mut next: ResMut<NextState<AppState>>) { next.set(AppState::InGame); }
// Debugging: add_systems(Update, bevy::dev_tools::states::log_transitions::<AppState>)
```

`examples/state/states.rs`

## Sub-States

A `SubStates` only exists while its source state matches; the `State<IsPaused>` resource is created/destroyed automatically on entering/leaving `AppState::InGame`.

```rust
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, SubStates)]
#[source(AppState = AppState::InGame)]
#[states(scoped_entities)]           // enables DespawnOnExit/DespawnOnEnter for this state
enum IsPaused { #[default] Running, Paused }

app.init_state::<AppState>().add_sub_state::<IsPaused>()
   .add_systems(Update, (movement, change_color).run_if(in_state(IsPaused::Running)))
   .add_systems(OnEnter(IsPaused::Paused), setup_paused_screen);
```

`examples/state/sub_states.rs`

## Computed States

Derive marker/enum states from one root state (or a tuple of states) — the `State<T>` resource only exists when `compute` returns `Some`. Perfect for collapsing a data-carrying state (`InGame { paused, turbo }`) into orthogonal checkable flags.

```rust
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct InGame; // ZST marker state

impl ComputedStates for InGame {
    type SourceStates = AppState;
    const ALLOW_SAME_STATE_TRANSITIONS: bool = false; // don't re-run OnEnter when only fields change
    fn compute(s: AppState) -> Option<Self> {
        matches!(s, AppState::InGame { .. }).then_some(Self)
    }
}

// Multi-source (tuples, Option<S> wraps make computation run even when absent):
impl ComputedStates for Tutorial {
    type SourceStates = (TutorialState, InGame, Option<IsPaused>);
    fn compute((tut, _ingame, paused): (TutorialState, InGame, Option<IsPaused>)) -> Option<Self> {
        if !matches!(tut, TutorialState::Active) { return None; }
        match paused? {
            IsPaused::NotPaused => Some(Tutorial::MovementInstructions),
            IsPaused::Paused => Some(Tutorial::PauseInstructions),
        }
    }
}

app.add_computed_state::<InGame>();
// then use OnEnter(InGame), in_state(InGame) exactly like normal states.
```

`examples/state/computed_states.rs`

## State-Scoped Entities

Attach despawn markers to auto-clean state content. Duplicates deep in hierarchies are safe (already-despawned is not an error).

```rust
commands.spawn((DespawnOnExit(GameState::A), Text::new("in A")));
commands.spawn((DespawnOnEnter(GameState::A), Text::new("shown while not in A")));
// Predicate form, e.g. despawn when leaving any C(_) variant:
commands.spawn((DespawnWhen::new(|t| matches!(t.exited, Some(GameState::C(_)))), ...));
```

`examples/ecs/state_scoped.rs`, `examples/state/sub_states.rs`

## Custom State Transitions (OnReenter)

Transitions are three ordered `StateTransition` phases: `ExitSchedules<S>` (leaf→root), `TransitionSchedules<S>`, `EnterSchedules<S>` (root→leaf). Pipe `last_transition::<S>` into a runner to add e.g. identity-transition schedules.

```rust
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct OnReenter<S: States>(pub S);

app.add_systems(StateTransition,
    last_transition::<S>.pipe(run_reenter::<S>).in_set(EnterSchedules::<S>::default()));

fn run_reenter<S: States>(transition: In<Option<StateTransitionEvent<S>>>, world: &mut World) {
    let Some(t) = transition.0 else { return };
    let Some(entered) = t.entered else { return };
    let _ = world.try_run_schedule(OnReenter(entered));
}
```

`examples/state/custom_transitions.rs`

## Buffered Messages (0.19 naming)

`#[derive(Message)]` + `add_message::<T>()`. Write with `MessageWriter` (`write` / `write_default`), read with `MessageReader::read()`, read-and-mutate with `MessageMutator` (a reader+writer of the same T in one system would conflict). Chain writer systems before reader systems or readers get a one-frame delay.

```rust
#[derive(Message, Debug)] struct DealDamage { amount: i32 }

fn deal(mut w: MessageWriter<DealDamage>) { w.write(DealDamage { amount: 10 }); }
fn armor(mut m: MessageMutator<DealDamage>, mut blocked: MessageWriter<ArmorBlockedDamage>) {
    for msg in m.read() { msg.amount -= 1; if msg.amount <= 0 { blocked.write(ArmorBlockedDamage); } }
}
fn apply(mut r: MessageReader<DealDamage>) { for d in r.read() { info!("{}", d.amount); } }

app.add_message::<DealDamage>()
   .add_systems(Update, (deal, armor, apply).chain());
```

`examples/ecs/message.rs`

## Observers

`#[derive(Event)]` for global events, `#[derive(EntityEvent)]` (with an `entity: Entity` field) for entity-targeted ones. Trigger with `commands.trigger(event)`. Observers run immediately (push-based), can take any system params, and support `.run_if(...)`.

```rust
#[derive(Event)]       struct ExplodeMines { pos: Vec2, radius: f32 }
#[derive(EntityEvent)] struct Explode { entity: Entity }

app.add_observer(|e: On<ExplodeMines>, mines: Query<&Mine>, mut commands: Commands| {
    // e.pos derefs to the event; can trigger cascades:
    commands.trigger(Explode { entity });
}.run_if(|enabled: Res<ExplosionsEnabled>| enabled.0));

// Entity-scoped observer (only fires for this entity):
commands.spawn(Mine::random(&mut rng)).observe(explode_mine);

// Reuse one observer across many entities (observers are entities with an Observer component):
let mut observer = Observer::new(explode_mine);
for _ in 0..1000 { observer.watch_entity(commands.spawn(Mine::random(&mut rng)).id()); }
commands.spawn(observer);

// Lifecycle events: On<Add, T>, On<Insert, T>, On<Remove, T>
fn on_add_mine(add: On<Add, Mine>, q: Query<&Mine>, mut idx: ResMut<SpatialIndex>) {
    let mine = q.get(add.entity).unwrap(); // add.entity is the target
}
```

Observers vs Messages: messages are buffered, batched, frame-delayed, and cheap for high-volume streams (network events); observers are immediate, targetable, and cascade — best for reactions, lifecycle sync, and indexes.

`examples/ecs/observers.rs`

## Observer Propagation (bubbling)

`EntityEvent`s can bubble up `ChildOf` toward the root. Mutate the event in flight; stop with `propagate(false)`.

```rust
#[derive(Clone, Component, EntityEvent)]
#[entity_event(propagate, auto_propagate)]
struct Attack { entity: Entity, damage: u16 }

fn block_attack(mut attack: On<Attack>, armor: Query<(&Armor, &Name)>) {
    let (armor, _) = armor.get(attack.entity).unwrap();
    let damage = attack.damage.saturating_sub(**armor);
    if damage > 0 { attack.damage = damage; }      // continues to parent
    else { attack.propagate(false); }              // absorbed
}
// child: .observe(block_attack); parent (goblin): .observe(take_damage)
```

`examples/ecs/observer_propagation.rs`

## Removal Detection

React to component removal (including despawn) with an `On<Remove, T>` observer — runs immediately after removal.

```rust
app.add_observer(react_on_removal);
fn react_on_removal(remove: On<Remove, MyComponent>, mut q: Query<&mut Sprite>) {
    if let Ok(mut sprite) = q.get_mut(remove.entity) { sprite.color = Color::srgb(0.5, 1., 1.); }
}
```

`examples/ecs/removal_detection.rs`

## Component Hooks

One hook per component per kind; lower overhead than observers. Kinds: `on_add` (insert onto entity lacking it), `on_insert` (every insert, after on_add), `on_discard` (before value replaced or removed), `on_remove` (before removal — data still readable). Register via derive attrs `#[component(on_add = ..., on_insert = ...)]` or `world.register_component_hooks::<T>()`. Hooks get `DeferredWorld` + `HookContext { entity, component_id, caller, .. }`.

```rust
world.register_component_hooks::<MyComponent>()
    .on_add(|mut world, HookContext { entity, .. }| {
        let value = world.get::<MyComponent>(entity).unwrap().0;
        world.resource_mut::<MyComponentIndex>().insert(value, entity);
        world.write_message(MyMessage);                 // hooks can write messages
    })
    .on_remove(|mut world, HookContext { entity, .. }| {
        world.commands().entity(entity).despawn();      // and queue commands
    });
```

`examples/ecs/component_hooks.rs`

## Immutable Components + Index Pattern

`#[component(immutable)]` forbids `&mut T` access — all mutation is remove/replace, so hooks capture *every* change. Ideal for keeping lookup indexes perfectly in sync (this is how relationships work internally).

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Component)]
#[component(immutable, on_insert = on_insert_name, on_discard = on_discard_name)]
pub struct Name(pub &'static str);

fn on_insert_name(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let &name = world.entity(entity).get::<Name>().unwrap();
    world.resource_mut::<NameIndex>().name_to_entity.insert(name, entity);
}
// To "mutate": entity.insert(Name("Steven")) — on_discard removes old index entry, on_insert adds new.
```

The same example shows dynamic runtime-registered components (`ComponentDescriptor::new_with_layout` + `insert_by_id`) — niche, needs unsafe.

`examples/ecs/immutable_components.rs`

## Entity Relationships (custom)

`ChildOf`/`Children` is one instance of the general relationship system. Define your own pair: the `#[relationship]` side is the source of truth (immutable — mutate by inserting a replacement); the `#[relationship_target]` side is maintained automatically by hooks.

```rust
#[derive(Component)]
#[relationship(relationship_target = TargetedBy)]
struct Targeting(Entity);

#[derive(Component)]
#[relationship_target(relationship = Targeting)]
struct TargetedBy(Vec<Entity>);

let alice = commands.spawn(Name::new("Alice")).id();
let bob = commands.spawn((Name::new("Bob"), Targeting(alice))).id();
commands.spawn((Name::new("Charlie"), Targeting(bob)))
    .with_related::<Targeting>(Name::new("James"))
    .with_related_entities::<Targeting>(|s| { s.spawn(Name::new("Devon")); });

commands.entity(alice).insert(Targeting(charlie));  // retarget: TargetedBy updates automatically
commands.entity(charlie).remove::<Targeting>();     // break the relationship
// Traversal helpers: targeting_query.iter_ancestors(entity), etc.
```

`examples/ecs/relationships.rs`

## Hierarchy (ChildOf/Children)

```rust
let parent = commands.spawn((Sprite::from_image(tex.clone()), Transform::default()))
    .with_children(|p| { p.spawn((Transform::from_xyz(250.0, 0.0, 0.0), Sprite { .. })); })
    .id();
commands.entity(parent).add_child(child);   // attach after the fact
// Declarative: spawn((..., children![(CompA, CompB), (CompC,)]))
// Children derefs to a slice of Entity; despawning a parent despawns descendants
// and despawning a child removes it from the parent's Children.
```

`examples/ecs/hierarchy.rs`

## Change Detection

Mutable deref marks changed regardless of equality — use `set_if_neq` to avoid spurious ticks. Filter with `Changed<T>`/`Added<T>`; inspect without filtering via `Ref<T>` (`is_added`, `is_changed`, and with the `track_location` feature, `changed_by()`).

```rust
fn change_component(time: Res<Time>, mut q: Query<&mut MyComponent>) {
    for mut c in &mut q { c.set_if_neq(MyComponent(time.elapsed_secs().round())); }
}
fn detect(changed: Query<Ref<MyComponent>, Changed<MyComponent>>, res: Res<MyResource>) {
    for c in &changed { info!("added: {} changed: {}", c.is_added(), c.is_changed()); }
    if res.is_changed() { /* resources too */ }
}
```

Gotcha (project memory): taking `&mut T` out of a `ResMut` every frame = change-tick spam even without writes.

`examples/ecs/change_detection.rs`

## One-Shot Systems

Register once, run on demand — push-based logic, callbacks stored in components, rarely-run systems out of the schedule.

```rust
let id: SystemId = commands.register_system(system_a);
commands.spawn(Callback(id));
commands.run_system(callback.0);            // deferred, from any system
world.run_system_once(my_system).unwrap();  // ad hoc, great for tests
// Callbacks can be ordinary, query-taking, or exclusive (&mut World) systems.
```

`examples/ecs/one_shot_systems.rs`, `examples/ecs/callbacks.rs`

## System Piping & Mapping

Compose systems by feeding output to `In<T>` input, or post-process with `.map`.

```rust
fn parse(msg: Res<Msg>) -> Result<usize, ParseIntError> { msg.parse() }
fn handle(In(result): In<Result<usize, ParseIntError>>) { /* match result */ }

app.add_systems(Update, (
    parse.pipe(handle),
    produce_string.map(|out| info!("{out}")),
    parse.map(drop),
));
```

`examples/ecs/system_piping.rs`

## Fallible Systems & Error Handling

Systems, observers, and commands can return `Result<(), BevyError>` (`Result` alias). Default handling panics; set an app-wide handler, adjust per-error severity, or pipe the result.

```rust
use bevy::ecs::error::warn;
app.set_error_handler(warn); // panic|error|warn|info|debug|trace|ignore

fn setup(mut meshes: ResMut<Assets<Mesh>>) -> Result {
    let mut mesh = Sphere::new(1.0).mesh().ico(7)?;   // ? just works
    mesh.generate_tangents()?;
    Ok(())
}

fn failing(world: &mut World) -> Result {
    world.get_resource::<Cfg>().ok_or("Resource not initialized")
        .with_severity(Severity::Warning)?;           // downgrade locally
    Ok(())
}

// Handle a single system's error explicitly:
app.add_systems(PostStartup, failing.pipe(|r: In<Result>| { let _ = r.0.inspect_err(|e| info!("{e}")); }));
// Per-command handler:
commands.queue_handled(|world: &mut World| -> Result { ... }, |error, ctx| error!("{error}, {ctx}"));
```

Note the project rule "critical systems fail loudly" — keep the default panic handler for those; use severities for recoverable paths.

`examples/ecs/error_handling.rs`

## Fallible System Params

Param validation can skip a system silently instead of erroring: `Single<D, F>` (exactly one match, else skip), `Option<Single>` (None unless exactly one; never skips), `Populated<D, F>` (at least one, else skip). Plain `Query` never fails.

```rust
fn track_targets(
    mut player: Single<(&mut Transform, &Player)>,                       // skip unless exactly 1
    enemy: Option<Single<&Transform, (With<Enemy>, Without<Player>)>>,   // None if 0 or >1
    time: Res<Time>,
) { let (tf, p) = &mut *player; ... }

fn move_targets(mut enemies: Populated<(&mut Transform, &mut Enemy)>) { for e in &mut *enemies {} }
```

`examples/ecs/fallible_params.rs`

## Custom SystemParam

Bundle recurring param groups into one named struct.

```rust
#[derive(SystemParam)]
struct PlayerCounter<'w, 's> {
    players: Query<'w, 's, &'static Player>,
    count: ResMut<'w, PlayerCount>,
}
impl PlayerCounter<'_, '_> { fn count(&mut self) { self.count.0 = self.players.iter().len(); } }
fn count_players(mut counter: PlayerCounter) { counter.count(); }
```

`examples/ecs/system_param.rs`

## Parallel Query Iteration

`par_iter_mut().for_each(...)` fans work over the ComputeTaskPool. Only worth it for expensive per-entity work; tune with `BatchingStrategy`.

```rust
sprites.par_iter_mut().for_each(|(mut tf, v)| { tf.translation += v.extend(0.0); });
sprites.par_iter_mut().batching_strategy(BatchingStrategy::fixed(32)).for_each(|_| { ... });
```

`examples/ecs/parallel_query.rs`

## Query Combinations & Contiguous Slices

```rust
// Pairwise n-body interactions:
let mut iter = query.iter_combinations_mut();
while let Some([(m1, t1, mut a1), (m2, t2, mut a2)]) = iter.fetch_next() { ... }

// SIMD-friendly slice access (None if the query isn't dense/archetypal):
for (mut health, decay) in query.contiguous_iter_mut().unwrap() {
    for (h, d) in health.iter_mut().zip(decay) { h.0 *= d.0; }
}
```

`examples/ecs/iter_combinations.rs`, `examples/ecs/contiguous_query.rs`

## Entity Disabling

`Disabled` component hides an entity from *all* queries by default (default query filter) without despawning — not a visibility tool. Opt back in by naming it: `Query<Entity, With<Disabled>>`.

```rust
commands.entity(e).insert(Disabled);           // disable
commands.entity(e).remove::<Disabled>();       // re-enable
```

`examples/ecs/entity_disabling.rs`

## Delayed Commands

Built-in timed command queue — no timer components needed for simple staged effects.

```rust
let mut delayed = commands.delayed();
delayed.secs(0.5).entity(e).insert(Sprite::from_color(Color::WHITE, SIZE));
delayed.secs(0.6).entity(e).insert(Sprite::from_color(Color::BLACK, SIZE));
```

`examples/ecs/delayed_commands.rs`

## Exclusive World Access & Testing

`&mut World` systems get full access (also usable as one-shots/callbacks). `world.run_system_once(system)` runs any system standalone — the standard way to unit-test systems against a bare `World`. Returns nested `Result` when the system itself is fallible. `DeferredWorld` (hooks/observers) gives structured access plus `world.commands()`.

```rust
let mut world = World::new();
world.run_system_once(spawn_things).unwrap();
let out = world.run_system_once(check_for_cycles).unwrap(); // system's own Result
```

`examples/ecs/relationships.rs`, `examples/ecs/one_shot_systems.rs`

## Generic Systems

Turbofish specializes a generic system per type — canonical for state cleanup.

```rust
fn cleanup_system<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q { commands.entity(e).despawn(); }
}
app.add_systems(OnExit(AppState::MainMenu), cleanup_system::<MenuClose>);
```

Closures (optionally with captured/moved state) are also valid systems: `app.add_systems(Update, move || info!("{captured}"))`.

`examples/ecs/generic_system.rs`, `examples/ecs/system_closure.rs`

## Plugins & Plugin Groups

Configurable plugins are plain structs; groups bundle plugins and support `disable`/`add_before` at the call site. (Project note: `add_plugins` tuples cap at 15 elements.)

```rust
struct PrintMessagePlugin { wait_duration: Duration, message: String }
impl Plugin for PrintMessagePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PrintMessageState { message: self.message.clone(),
            timer: Timer::new(self.wait_duration, TimerMode::Repeating) })
           .add_systems(Update, print_message_system);
    }
}

pub struct HelloWorldPlugins;
impl PluginGroup for HelloWorldPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(PrintHelloPlugin).add(PrintWorldPlugin)
    }
}
// app.add_plugins(HelloWorldPlugins.build().disable::<PrintWorldPlugin>())
```

`examples/app/plugin.rs`, `examples/app/plugin_group.rs`

## Custom App Runner

Drive `app.update()` yourself (headless tools, external loops). `AppExit` implements `Termination` so `main` can return `app.run()`.

```rust
fn my_runner(mut app: App) -> AppExit {
    app.finish(); app.cleanup();
    loop {
        // feed input into resources...
        app.update();
        if let Some(exit) = app.should_exit() { return exit; }
    }
}
fn main() -> AppExit { App::new().set_runner(my_runner).run() }
```

`examples/app/custom_loop.rs`

## Time: Virtual, Real, Timers

`Res<Time>` in `Update` is `Time<Virtual>` — pausable and scalable. `Time<Real>` is wall-clock. Run conditions: `on_timer(d)` uses virtual time, `on_real_timer(d)` doesn't.

```rust
fn setup(mut time: ResMut<Time<Virtual>>) { time.set_relative_speed(2.); }
// time.pause() / time.unpause() / time.is_paused()
app.add_systems(Update, (
    toggle_pause.run_if(input_just_pressed(KeyCode::Space)),
    update_ui.run_if(on_real_timer(Duration::from_millis(250))),
));

// Timers live in components or resources; you tick them manually:
#[derive(Component, Deref, DerefMut)] struct Cooldown(Timer);
fn tick(time: Res<Time>, mut q: Query<&mut Cooldown>) {
    for mut t in &mut q {
        if t.tick(time.delta()).just_finished() { info!("done"); }
    }
}
// Timer::from_seconds(5.0, TimerMode::Once | TimerMode::Repeating); .fraction(); .pause()
```

`examples/time/virtual_time.rs`, `examples/time/timers.rs`, `examples/time/time.rs`
