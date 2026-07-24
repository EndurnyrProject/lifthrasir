---
name: bevy
description: Full-faceted Bevy 0.19 reference distilled from the official examples repo. Use when writing or reviewing any Bevy code — ECS (systems, queries, observers, messages, states, fixed timestep), 2D/3D rendering, cameras, custom shaders/materials, UI layout and picking, asset loaders, animation, audio, input, windowing, diagnostics, BRP, or async tasks. The SKILL.md holds core 0.19 facts; load the referenced facet file for the area you are touching.
---

# Bevy 0.19 (examples-grounded)

Distilled from the official Bevy examples at `/Users/ygorcastor/Development/personal/bevy/examples/` (checkout pinned to the `release-0.19.0` tag — same version as this project). Every pattern in the facet files cites its source example path; open the example under that checkout when you need the full context.

## How to use this skill

1. Read the core facts below — they prevent the most common cross-version mistakes.
2. Load the facet file(s) from `references/` matching what you're touching. Load more than one when the work spans areas (e.g. a world-space health bar = `ui-picking.md` + `rendering.md`).
3. When a snippet isn't enough, open the cited example file in the local Bevy checkout.

| You are working on | Load |
|---|---|
| Systems, queries, observers, messages/events, states, schedules, fixed timestep, hierarchy, change detection | `references/ecs.md` |
| Sprites, meshes, materials (Standard/2d), lighting, transparency, cameras, render layers, transforms, gizmos | `references/rendering.md` |
| Custom WGSL, Material/Material2d/UiMaterial, ExtendedMaterial, post-processing, instancing, compute | `references/shaders.md` |
| UI nodes, layout, scrolling, text, z-order, images/9-patch, picking, drag & drop, world-to-screen | `references/ui-picking.md` |
| Custom asset loaders, hot reload, load state, embedded assets, scenes, reflection | `references/assets.md` |
| glTF loading/labels, scene instances, extras metadata, loader extension hooks, skinned meshes | `references/gltf.md` |
| AnimationPlayer/graph, animating arbitrary properties, easing/curves, animation events | `references/animation.md` |
| Audio playback, keyboard/mouse/text input, window settings, cursor, screenshots | `references/audio-input-window.md` |
| FPS overlay, diagnostics, BRP/remote, async compute tasks, bounding volumes, math sampling | `references/dev-tools.md` |

## Core 0.19 facts (read before writing any Bevy code)

These are the load-bearing differences from older Bevy that stale training data and the (0.14-era) cheat book get wrong:

- **No bundles.** `SpriteBundle`, `NodeBundle`, `PbrBundle`, `Camera3dBundle`, `TextBundle` etc. are gone. Spawn the primary component directly (`Sprite`, `Node`, `Mesh3d` + `MeshMaterial3d`, `Camera3d`, `Text`); **required components** auto-insert the rest (`Transform`, `Visibility`, …).
- **Events are Messages.** Buffered events: `#[derive(Message)]`, `app.add_message::<T>()`, `MessageWriter<T>` / `MessageReader<T>`, `.write(...)`. The words `EventReader`/`EventWriter`/`add_event`/`.send(` are 0.14-era.
- **Observers use `On<E>`**, not `Trigger<E>`. Lifecycle observers take the component as a second type parameter: `On<Add, C>`, `On<Insert, C>`, `On<Replace, C>`, `On<Remove, C>` (not the 0.18 `On<Add<C>>` nesting). Trigger with `commands.trigger(MyEvent { .. })`; entity events derive `EntityEvent` and carry their target.
- **Hierarchy:** `ChildOf(parent)` relationship component + auto-maintained `Children`. `commands.entity(e).insert(ChildOf(p))`, `.add_children(&[...])`, `.with_children(|p| ...)`. `.despawn()` is recursive by default.
- **Queries:** `query.single()` and `single_mut()` return `Result` — use `?` in fallible systems or `let Ok(x) = ... else { return; }`. `get_single` is gone.
- **Time:** `time.delta_secs()`, `time.elapsed_secs()` (not `delta_seconds`).
- **UI:** style fields live on `Node` itself. Text = `Text::new("...")` + `TextFont` + `TextColor`; world text = `Text2d`. UI rotation/scale via `UiTransform` (`Rot2`), not `Transform`.
- **States:** `#[derive(States)]` + `app.init_state::<S>()`; scoped despawn via `DespawnOnExit(state)` / `DespawnOnEnter(state)` (formerly `StateScoped`).
- **Fallible systems** returning `Result` are idiomatic; `?` on `single()`, asset lookups, etc.

## Project-local conventions (Lifthrasir)

- World up is **-Y** — lift things above units with negative Y offsets.
- Companion skills: `bevy-feathers-bsn` (BSN scenes + feathers widgets — use it for UI authoring style), `lifthrasir-bevy-plugins` (third-party crate APIs: auto_plugin, kira audio, hanabi, persistent, moonshine), `bevy-cheatbook` (0.18-era triage tables; this skill supersedes it for 0.19 API questions).
- Audio goes through `bevy_kira_audio`, not Bevy's built-in audio.
- Never launch the client to verify; use BRP against a running instance or tests.
