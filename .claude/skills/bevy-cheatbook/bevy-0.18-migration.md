# Bevy 0.15 → 0.18 stale-pattern reference

Exhaustive map of cheatbook-era (~0.14–0.15) patterns to correct **Bevy 0.18.1** syntax. Version tags mark which release introduced the change. All snippets are 0.18.1-correct.

## 1. Bundles → required components

Bundles deprecated 0.15, removed by 0.16. Spawn the marker component directly; its `#[require(...)]` deps auto-insert.

| Stale (≤0.15) | Current (0.18) |
|---|---|
| `SpriteBundle { texture, .. }` | `Sprite::from_image(handle)` |
| `Camera2dBundle::default()` | `Camera2d` |
| `Camera3dBundle::default()` | `Camera3d` |
| `PbrBundle { mesh, material, .. }` | `(Mesh3d(mesh), MeshMaterial3d(material))` |
| `MaterialMesh2dBundle` | `(Mesh2d(mesh), MeshMaterial2d(material))` |
| `NodeBundle { style, .. }` | `Node { ..default() }` |
| `TextBundle::from_section(..)` | `(Text::new("hi"), TextFont { .. }, TextColor(..))` |
| `PointLightBundle` | `PointLight { ..default() }` |
| `TransformBundle` | `Transform` (requires `GlobalTransform`) |
| `SpatialBundle` | `Transform` + `Visibility` |

```rust
commands.spawn((
    Mesh3d(meshes.add(Cuboid::default())),
    MeshMaterial3d(materials.add(Color::WHITE)),
    Transform::from_xyz(0.0, 0.5, 0.0),
));
commands.spawn(Sprite::from_image(asset_server.load("player.png")));
commands.spawn(Camera2d);
```

`#[require]` syntax also changed (0.15 → 0.16):
```rust
// 0.15:  #[require(A(returns_a))]   or  #[require(A(SomeValue))]
// 0.18:  #[require(A = returns_a())] or #[require(A = SomeValue)]
```

## 2. Parent/child hierarchy (0.16 rework)

`Parent` is gone; it's the `ChildOf` relationship component.

| Stale | Current (0.18) |
|---|---|
| `Parent` component | `ChildOf` |
| `*parent` / `parent.get()` | `child_of.parent()` |
| `.set_parent(p)` | `.insert(ChildOf(p))` |
| `.push_children(&[..])` / `BuildChildren` | `.add_children(&[..])` |
| `.replace_children(&[..])` | `.remove::<Children>().add_children(&[..])` |
| `.despawn_recursive()` | `.despawn()` (recursive by default) |
| `.despawn_descendants()` | `.despawn_related::<Children>()` |
| `ChildBuilder` | `ChildSpawnerCommands` |
| `builder.parent_entity()` | `spawner.target_entity()` |

`.with_children(|p| { .. })` and `.add_child(e)` still exist. New 0.16 `children!` macro:
```rust
commands.spawn((
    Node { ..default() },
    children![
        (Text::new("Score"), TextColor(Color::WHITE)),
        Button,
    ],
));
```

## 3. Buffered events → Messages (0.17 rename)

0.17 split the old monolithic `Event` into **`Message`** (buffered, reader/writer) and **`Event`** (observable, trigger/observer). This is the most confusing area for stale code.

| Stale (≤0.16) | Current (0.18) |
|---|---|
| `#[derive(Event)]` (buffered use) | `#[derive(Message)]` |
| `EventReader<E>` | `MessageReader<M>` |
| `EventWriter<E>` | `MessageWriter<M>` |
| `Events<E>` | `Messages<M>` |
| `app.add_event::<E>()` | `app.add_message::<M>()` |
| `writer.send(e)` | `writer.write(m)` |
| `send_batch` / `send_default` | `write_batch` / `write_default` |
| `world.send_event(e)` | `world.write_message(m)` |

(`.send()` → `.write()` landed deprecated in 0.16; full `Message` rename in 0.17.)

```rust
#[derive(Message)]
struct DamageEvent { amount: u32 }
// app.add_message::<DamageEvent>();
fn write_sys(mut w: MessageWriter<DamageEvent>) { w.write(DamageEvent { amount: 10 }); }
fn read_sys(mut r: MessageReader<DamageEvent>) { for m in r.read() { /* */ } }
```

## 4. Observers + Event (0.17 `On<>` rename, 0.18 `EntityEvent`)

`Trigger<E>` → `On<E>` (0.17; `Trigger` removed in 0.18). Observable events use `#[derive(Event)]` (global) or `#[derive(EntityEvent)]` (entity-targeted).

```rust
#[derive(Event)]
struct GameOver { score: u32 }
world.add_observer(|over: On<GameOver>| info!("scored {}", over.score));
world.trigger(GameOver { score: 100 });

#[derive(EntityEvent)]
struct Click { entity: Entity }     // entity is a FIELD now (no trigger_targets)
commands.entity(e).observe(|click: On<Click>| info!("clicked {}", click.entity));
commands.trigger(Click { entity: e });
```

- Custom target field: `#[event_target]`.
- Propagation/bubbling: `#[entity_event(propagate)]`; original target via `On::original_event_target()`.
- **0.18**: `EntityEvent`s are immutable by default; mutating methods moved to the `SetEntityEventTarget` trait.
- Lifecycle hooks: `Trigger<OnAdd<C>>` → `On<Add<C>>` (also `Insert`, `Replace`, `Remove`, `Despawn`).

## 5. Time API (`*_secs`) — 0.18 confirmed

`delta_seconds()` / `elapsed_seconds()` removed.

| Stale | Current (0.18) |
|---|---|
| `time.delta_seconds()` | `time.delta_secs()` (`f32`) |
| `time.delta_seconds_f64()` | `time.delta_secs_f64()` |
| `time.elapsed_seconds()` | `time.elapsed_secs()` |
| `time.elapsed_seconds_f64()` | `time.elapsed_secs_f64()` |
| `time.elapsed_seconds_wrapped()` | `time.elapsed_secs_wrapped()` |

`delta()` and `elapsed()` (returning `Duration`) unchanged.

## 6. Query single → Result (0.16)

`single()` now returns `Result`; `get_single()` removed (folded in).

| Stale | Current (0.18) |
|---|---|
| `query.single()` (panics) | `query.single()?` |
| `query.get_single()` | `query.single()` |
| `query.single_mut()` | `query.single_mut()?` |
| `query.many([..])` | `query.get_many([..])?` |

```rust
fn move_player(mut q: Query<&mut Transform, With<Player>>) -> Result {
    let mut t = q.single_mut()?;
    t.translation.x += 1.0;
    Ok(())
}
// or the Single<T> param (validation-skips when wrapped Option<Single<T>>):
fn move_player(player: Single<&mut Transform, With<Player>>) { /* deref */ }
```

## 7. States (through 0.18)

`States` / `init_state` / `SubStates` / computed states keep their shape. Changes:

- State-scoped entities renamed (0.17): `StateScoped` → `DespawnOnExit` (also `DespawnOnEnter`). `add_state_scoped_event` removed — use `add_message` + `clear_messages_on_exit`.
- **0.18**: transitions always trigger — `OnEnter`/`OnExit` fire even on same-state set. Use `next.set_if_neq(s)` for old only-on-change behavior.

```rust
commands.spawn((Node::default(), DespawnOnExit(GameState::Menu)));
```

## 8. Commands / system params / error handling (0.16)

- Systems can return `-> Result` and use `?`; idiomatic for fallible logic, pairs with `single()?`.
- A default error handler routes returned errors (panics by default; configurable via `App::set_error_handler` / `GLOBAL_ERROR_HANDLER`).
- `commands.spawn(..)` returns `EntityCommands`; `spawn_empty()` unchanged. `commands.add(..)` → prefer `commands.queue(..)` for custom commands.

## 9. Text model (0.15+)

Single-`Text`-with-sections replaced by entity + component model.

| Stale | Current (0.18) |
|---|---|
| `Text::from_section(s, style)` | `Text::new(s)` (UI) / `Text2d::new(s)` (world) |
| `TextStyle { font, font_size, color }` | `TextFont { font, font_size, .. }` + `TextColor(Color)` |
| multiple sections in one `Text` | child entities with `TextSpan` |

```rust
commands.spawn((
    Text::new("HP: "),
    TextFont { font_size: 24.0, ..default() },
    TextColor(Color::WHITE),
    children![( TextSpan::new("100"), TextColor(Color::srgb(0.0, 1.0, 0.0)) )],
));
```

0.18: `LineHeight` is its own component (was a `TextFont` field).

## 10. UI / bevy_ui (`Node` replaces `Style`+`NodeBundle`)

`Style` gone — its fields moved onto `Node` (0.15).

| Stale | Current (0.18) |
|---|---|
| `NodeBundle { style: Style { .. }, .. }` | `Node { .. }` |
| `Style { width, flex_direction, .. }` | same fields on `Node` |
| `ZIndex` bundle field | `ZIndex` / `GlobalZIndex` components |
| `ButtonBundle` | `Button` (requires `Node`) |
| `ImageBundle` | `ImageNode` |

```rust
commands.spawn(Node {
    width: Val::Percent(100.0),
    flex_direction: FlexDirection::Column,
    ..default()
});
```

0.18 UI tweaks: `BorderRadius` is now a field on `Node`, not a separate component; `BorderRect` changed from `{ left, right, top, bottom }` to `{ min_inset, max_inset }` (`Vec2`). Picking (stable 0.15) is observer-driven: `On<Pointer<Click>>`, `On<Pointer<Over>>`, etc.

## 11. Other 0.16–0.18 breakers worth knowing

- **`AmbientLight` split (0.18)**: global ambient is the `GlobalAmbientLight` resource; `AmbientLight` is a per-camera component.
- **`RenderTarget` is a component (0.18)**, moved off the `Camera` struct — spawn `RenderTarget::Image(h)` alongside `Camera3d`.
- **System set naming (0.17)**: consistent `*Systems` suffix (`RenderSet`→`RenderSystems`, `UiSystem`→`UiSystems`).
- **Render resource init (0.17)**: initialize in the `RenderStartup` schedule via a system, not `Plugin::finish` + `FromWorld`.
- **Rendering crate split (0.17)**: types moved to `bevy_camera`, `bevy_light`, `bevy_mesh`, `bevy_image`, `bevy_shader`, `bevy_ui_render`, `bevy_sprite_render` (prelude re-exports mostly shield you; deep paths broke).
- **Mesh mutation can fail (0.18)**: `mesh.insert_attribute(..)` → `mesh.try_insert_attribute(..)?` for render-world-only meshes.

## Quick triage rules

1. identifier ends in `Bundle` → §1
2. `Parent` / `set_parent` / `push_children` / `despawn_recursive` → §2
3. `EventReader`/`EventWriter`/`add_event`/`.send(` → §3 (buffered = `Message`)
4. `Trigger<` → `On<` (§4); `OnAdd`/`OnInsert` → `Add`/`Insert`
5. `delta_seconds`/`elapsed_seconds` → `*_secs` (§5)
6. `get_single` / bare `single()` → §6
7. `StateScoped` → `DespawnOnExit` (§7)
8. `Style` / `TextStyle` / `from_section` → `Node` / `TextFont`+`TextColor` / `Text::new` (§9–10)

## Sources

- [Migration 0.15→0.16](https://bevy.org/learn/migration-guides/0-15-to-0-16/)
- [Migration 0.16→0.17](https://bevy.org/learn/migration-guides/0-16-to-0-17/)
- [Migration 0.17→0.18](https://bevy.org/learn/migration-guides/0-17-to-0-18/)
- [Bevy 0.15 release notes](https://bevy.org/news/bevy-0-15/)
- [Bevy 0.17 release notes](https://bevy.org/news/bevy-0-17/)
- [docs.rs Time 0.18.1](https://docs.rs/bevy/0.18.1/bevy/time/struct.Time.html)

Verified against the lifthrasir codebase (Bevy 0.18.1): `Message`/`MessageReader`/`MessageWriter`, `On<E>` observers, `EntityEvent` + `#[event_target]`, `Node`/`Text`/`TextFont`/`TextColor`, `children![]`, `ChildOf` + `.parent()`, `Single<T>`, `DespawnOnExit`, `time.delta()`.
