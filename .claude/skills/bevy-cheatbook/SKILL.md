---
name: bevy-cheatbook
description: Use when writing or reviewing Bevy 0.18 ECS code (systems, queries, components, bundles, plugins, states, events/messages, observers, transforms, hierarchy, change detection, run conditions, fixed timestep) or when adapting examples from the outdated Unofficial Bevy Cheat Book to current 0.18 APIs.
---

# Bevy Cheatbook (verified for Bevy 0.18)

## Overview

Distilled ECS concepts and pitfalls from the Unofficial Bevy Cheat Book, with all API syntax verified against **Bevy 0.18.1**.

**The Cheat Book is unmaintained — its pages target ~Bevy 0.14–0.15.** Its *concepts* (ECS mental model, scheduling, change detection, transform/visibility gotchas) are still correct. Its *API names* are largely stale. When you read the book online, trust the ideas, not the code. This skill carries the current syntax.

**Before quoting any cheatbook code snippet, run the triage table below.** If an identifier matches a "stale" cell, the snippet is wrong for 0.18.

## Stale → 0.18 Triage (top offenders)

| If you see (stale) | It's now (0.18) | Topic |
|---|---|---|
| `*Bundle` (`SpriteBundle`, `NodeBundle`, `PbrBundle`, `Camera2dBundle`, `TextBundle`…) | spawn the marker component directly; required components pull deps | Bundles |
| `EventReader` / `EventWriter` / `add_event` / `.send(` | `MessageReader` / `MessageWriter` / `add_message` / `.write(` | Buffered events |
| `Trigger<E>` (observer param) | `On<E>` | Observers |
| `Trigger<OnAdd<C>>` | `On<Add<C>>` (also `Insert`, `Replace`, `Remove`, `Despawn`) | Lifecycle |
| `Parent` / `.set_parent` / `.push_children` / `.despawn_recursive` | `ChildOf` / `.insert(ChildOf(p))` / `.add_children` / `.despawn` (recursive by default) | Hierarchy |
| `time.delta_seconds()` / `elapsed_seconds()` | `time.delta_secs()` / `elapsed_secs()` | Time |
| `query.get_single()` / panicking `query.single()` | `query.single()` → **returns `Result`**, use `?` | Queries |
| `Style { .. }` + `NodeBundle` | `Node { .. }` (style fields live on `Node`) | UI |
| `TextStyle` / `Text::from_section` | `TextFont` + `TextColor` + `Text::new` (or `Text2d::new`) | Text |
| `StateScoped` | `DespawnOnExit` (also `DespawnOnEnter`) | States |

Full mapping with snippets and citations: **see `bevy-0.18-migration.md` in this skill directory.**

---

## ECS Data Model

**Entity** = just an ID labelling a set of components. **Component** = plain data attached to an entity. Mental model: a database — components are columns, entities are rows, any entity can hold any combination.

- Decompose data into granular components so unrelated fields can be accessed (and scheduled) independently. Split when fields are accessed independently; combine when they inherently belong together.
- **Avoid** OOP-style monolithic `Player` structs holding everything — that serializes access and kills parallelism.
- Empty marker components are tags for query filtering.

**Resource** = a global singleton keyed by type, accessible from any system. Good for config/settings/external-handle wrappers. Don't overuse: "exists once" ≠ "should be a resource" — a single player is better as an entity with components.

```rust
#[derive(Component)]
struct Health(f32);

#[derive(Resource, Default)]
struct GameSettings { /* ... */ }

// init from Default/FromWorld, or insert a concrete value:
app.init_resource::<GameSettings>();
app.insert_resource(GameSettings::default());
```

**Required components** (0.16+) replace bundles: a component declares its dependencies and they're auto-inserted. The local codebase composes tuples manually instead — both are valid:

```rust
#[derive(Component)]
#[require(Transform)]      // 0.18 form: #[require(Foo = expr())] for non-default init
struct Player;
```

## Systems & Scheduling

A system is a plain `fn` whose **parameter list declares exactly what World data it touches**. Bevy reads that signature to schedule systems with non-conflicting access **in parallel automatically**. Narrow systems → more parallelism; god-systems serialize.

```rust
app.add_systems(Startup, spawn_world);
app.add_systems(Update, (start_fade, tick_fade).chain());   // .chain() = run in order
app.add_systems(Update, move_player.run_if(in_state(GameState::InGame)));
app.add_systems(Update, raycast.after(gather_input).before(apply_input));
```

**"does not implement `System`" / `IntoSystem` error**: the compiler blames the `add_systems` call, but the real bug is an invalid **parameter type** in the fn signature. Every param must be a system param — wrap components in `Query`, resources in `Res`/`ResMut`, take owned `Commands`, write multi-component access as a tuple `Query<(&A, &B)>` not `Query<A, B>`.

**Ordering is non-deterministic by default** and can change each frame. Add explicit `.before`/`.after`/`.chain` only when there's a real data/event/change-detection dependency. Wrong ordering usually surfaces as a subtle **one-frame delay**, not a crash.

**Run conditions** are read-only `fn`s returning `bool` that gate a system or set. Multiple conditions combine as logical **AND**. Pitfall: a gated system **misses messages/events** sent while it wasn't running.

**System sets** group systems for shared ordering/conditions (`in_set`). Set config does **not** carry across schedules — a set configured for `Update` won't apply in `OnEnter`.

**States** are named runtime modes. Attach systems via `OnEnter`/`OnExit`/`run_if(in_state(..))` rather than checking state inside the system. Transition order: `OnExit(old)` → `OnEnter(new)` (cleanup before setup), applied once per frame before gameplay systems.

```rust
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState { #[default] Loading, InGame, Paused }

app.init_state::<GameState>();
app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen);
// transition:
fn advance(mut next: ResMut<NextState<GameState>>) { next.set(GameState::InGame); }
```

0.18: `OnEnter`/`OnExit` fire even when setting the **same** state — use `next.set_if_neq(s)` for old "only-on-change" behavior.

## Data Access Within Systems

**Queries** = which components + optional filters. Tuples = AND; `Or<(..)>` = OR; `With`/`Without` test has/lacks; `Has<C>` yields a bool; `Option<&C>` for maybe-present.

```rust
Query<(Entity, &Transform), With<LocalPlayer>>
Query<&mut Transform, (With<Billboard>, Without<Camera3d>)>
Query<(Has<Stunned>, &Health)>
```

Single match: prefer the `Single<T>` system param (skips the run cleanly if wrapped `Option<Single<T>>`), or `query.single()?` which **returns `Result`** in 0.18.

```rust
fn tick(mut fade: Single<(&mut ScreenFade, &mut BackgroundColor)>) { /* deref directly */ }
```

**Never query for a bundle/required-component group — query the individual components.**

**Commands** queue **deferred** structural changes (spawn/despawn, add/remove component, insert/remove resource). They apply at sync points — schedule end, or *between* two systems if you ordered them. Don't assume they take effect immediately; for synchronous World mutation use an exclusive/one-shot system.

```rust
commands.spawn((Health(100.0), Transform::default(), Name::new("Mob")));
commands.entity(e).insert(Stunned);
commands.entity(e).despawn();              // recursive in 0.18
```

**`Local<T>`** = per-system persistent private state; two systems with the same `Local<T>` get **separate** instances. Must be `Default`/`FromWorld`. Watch for silent accumulation if you forget to reset.

**Custom `SystemParam`** structs group params — for reuse and to exceed the per-system parameter-count (tuple-arity) cap.

## Reactivity

**Change detection**: `Added<C>` (newly inserted/spawned) and `Changed<C>` query filters.

- Triggered by **`DerefMut`, not actual mutation** — taking `&mut C` flags it changed even if the value is identical. Guard with a compare-before-write when it matters.
- One-frame lag if the detecting system runs before the mutating one this frame.
- More robust than messages for intermittent/gated systems — the flag persists until *that* system observes it.

**Removal detection**: `RemovedComponents<C>`. The signal is **ephemeral/frame-scoped** — the detecting system must run *after* the remover **within the same frame**, or it's lost. Don't gate a removal-detecting system behind run conditions.

**Messages** (buffered, the old "events"): writer appends cheaply, each reader tracks its own cursor. Retained ~2 update cycles then auto-dropped. A reader running *before* the writer sees it next frame; a reader that doesn't run every ~2 cycles **misses messages entirely**.

```rust
#[derive(Message)]
struct DamageEvent { amount: u32 }
app.add_message::<DamageEvent>();

fn writer(mut w: MessageWriter<DamageEvent>) { w.write(DamageEvent { amount: 10 }); }
fn reader(mut r: MessageReader<DamageEvent>) { for m in r.read() { /* ... */ } }
```

**Observers** (push-based, immediate) use `#[derive(Event)]` (global) or `#[derive(EntityEvent)]` (entity-targeted), the `On<E>` param, and `.trigger(..)`:

```rust
#[derive(EntityEvent)]
struct Click { #[event_target] entity: Entity }

commands.entity(e).observe(|click: On<Click>| { info!("clicked {}", click.entity); });
commands.trigger(Click { entity: e });

// lifecycle hooks:
app.add_observer(|add: On<Add<Health>>| { /* ... */ });
```

Rule of thumb: **Message** = many readers, frame-buffered, polled in systems. **Observer/Event** = react immediately to a specific occurrence.

## Plugins

A plugin bundles app-builder additions (systems, resources, messages) so apps compose from independent modules. Plain `fn(&mut App)` for simple cases; a `struct` impl'ing `Plugin` when it needs config fields. `PluginGroup` batches related plugins (members can be disabled). Organize around states/subsystems, not one monolith.

```rust
struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageEvent>()
           .add_systems(Update, apply_damage);
    }
}
app.add_plugins((CombatPlugin, InputPlugin));
```

## Time & Fixed Timestep

Always scale motion by delta time — "units per second", not per frame — so behavior is frame-rate independent. **Use `Res<Time>`, never `Instant::now()`**: the parallel scheduler samples engine time once per frame for consistency.

```rust
fn mover(time: Res<Time>, mut q: Query<&mut Transform, With<Mob>>) {
    for mut t in &mut q { t.translation.x += SPEED * time.delta_secs(); }
}
// Timers/Stopwatches must be ticked to advance:
timer.tick(time.delta());
```

`FixedUpdate` runs at a constant interval (physics, AI, netcode, determinism). Its accumulator may run it 0..n times per render frame to catch up — **not** tied to wall-clock instants. Do simulation in `FixedUpdate`, then interpolate for rendering in `Update`. Frame-based input (`just_pressed`) is **unreliable inside `FixedUpdate`** — capture it in a per-frame schedule.

## Transforms, Hierarchy, Visibility

- `Transform` = local TRS relative to parent (you write this). `GlobalTransform` = absolute world-space, **computed by Bevy in PostUpdate, read-only**.
- **Staleness gotcha**: mutating `Transform` does NOT immediately update `GlobalTransform`. Reading `GlobalTransform` in the same system you just moved the entity gives stale data.
- Hierarchy is flat storage + relationship components: `ChildOf` on the child, `Children` on the parent, maintained automatically. Children inherit transform and visibility.

```rust
commands.spawn((Transform::default(), children![
    (Sprite::from_image(handle), Transform::from_xyz(0.0, 1.0, 0.0)),
]));
// read parent in a system:
fn sync(q: Query<&ChildOf>, parents: Query<&RoSprite>) {
    if let Ok(child_of) = q.single() { let _ = parents.get(child_of.parent()); }
}
```

- **Despawn pitfall**: despawning a child without detaching can leave a stale ref in the parent's `Children` → later panic. Despawn the parent (recursive) or detach first.
- **Visibility** is three components: `Visibility` (your toggle: `Inherited`/`Visible`/`Hidden`), `InheritedVisibility` and `ViewVisibility` (both computed, read-only). Hiding ≠ despawning — hidden entities stay alive and cheap to re-show.

## Common Pitfalls

- **Wrong schedule**: a system in `Update` reading data propagated in `PostUpdate` (e.g. `GlobalTransform`) sees stale values. Match the schedule to the data lifecycle.
- **Debug builds are brutally slow** with Bevy. Optimize dependencies in `Cargo.toml` (`[profile.dev.package."*"] opt-level = 3`) while leaving your crate unoptimized — fast iteration, playable FPS. Reserve full release/LTO for distribution.
- **Gated systems lose messages and removal signals** (see Reactivity). Prefer change detection for state that must survive missed runs.
- **One-frame delays** from ordering: if A must read what B wrote this frame, order them.

## When NOT to use the online Cheat Book verbatim

Any rendering/UI/text/events/hierarchy/input example — these are the areas that churned most across 0.15→0.18. Verify against `bevy-0.18-migration.md`, docs.rs/bevy/0.18, or the actual codebase before trusting it.
