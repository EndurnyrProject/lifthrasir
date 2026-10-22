---
name: bevy
description: Full-faceted Bevy 0.20 reference distilled from the official examples repo. Use when writing or reviewing any Bevy code — ECS (systems, queries, observers, messages, states, fixed timestep, chain_weak, despawn_all), 2D/3D rendering, cameras, custom WESL shaders/materials, sprite materials, UI layout and picking (em/rem, FixedNode, InlineBox, BorderRadius/CornerRadius), contextual theming, asset loaders, animation, audio, input, windowing, error handling and panics, diagnostics, BRP, or async tasks. The SKILL.md holds core 0.20 facts; load the referenced facet file for the area you are touching.
---

# Bevy 0.20 (examples-grounded)

Distilled from the official Bevy examples at tag `v0.20.0` (the same minor this project pins). Every pattern in the facet files cites its source example as a repo-relative path (`examples/…`); fetch that file from the Bevy repo at that tag when you need the full context.

## How to use this skill

1. Read the core facts below — they prevent the most common cross-version mistakes.
2. Load the facet file(s) from `references/` matching what you're touching. Load more than one when the work spans areas (e.g. a world-space health bar = `ui-picking.md` + `rendering.md`).
3. When a snippet isn't enough, read the cited example from the Bevy repo at tag `v0.20.0` (raw file or docs.rs), not from memory.
4. For UI authored with `bsn!`/Feathers, use the companion `bevy-feathers-bsn` skill — its syntax changed in 0.20 and this file only summarizes it.

| You are working on | Load |
|---|---|
| Systems, queries, observers, messages/events, states, schedules, fixed timestep, hierarchy, change detection, errors/panics | `references/ecs.md` |
| Sprites, meshes, materials (Standard/2d/sprite), lighting, transparency, cameras, render layers, transforms, gizmos | `references/rendering.md` |
| Custom WESL shaders, Material/Material2d/UiMaterial, ExtendedMaterial(2d), post-processing, instancing, compute, mesh shaders | `references/shaders.md` |
| UI nodes, layout, scrolling, text, z-order, images/9-patch, picking, drag & drop, world-to-screen | `references/ui-picking.md` |
| Custom asset loaders, hot reload, load state, embedded assets, scenes, reflection | `references/assets.md` |
| glTF loading/labels, scene instances, extras metadata, loader extension hooks, skinned meshes | `references/gltf.md` |
| AnimationPlayer/graph, animating arbitrary properties, easing/curves, animation events | `references/animation.md` |
| Audio playback, keyboard/mouse/text input, window settings, cursor, screenshots | `references/audio-input-window.md` |
| FPS overlay, diagnostics, BRP/remote, async compute tasks, bounding volumes, math sampling | `references/dev-tools.md` |

## Core facts (still true in 0.20, read before writing any Bevy code)

These are the load-bearing differences from older Bevy that stale training data and the (0.14-era) cheat book get wrong:

- **No bundles.** `SpriteBundle`, `NodeBundle`, `PbrBundle`, `Camera3dBundle`, `TextBundle` etc. are gone. Spawn the primary component directly (`Sprite`, `Node`, `Mesh3d` + `MeshMaterial3d`, `Camera3d`, `Text`); **required components** auto-insert the rest (`Transform`, `Visibility`, …).
- **Events are Messages.** Buffered events: `#[derive(Message)]`, `app.add_message::<T>()`, `MessageWriter<T>` / `MessageReader<T>`, `.write(...)`. The words `EventReader`/`EventWriter`/`add_event`/`.send(` are 0.14-era.
- **Observers use `On<E>`**, not `Trigger<E>`. Lifecycle observers take the component as a second type parameter: `On<Add, C>`, `On<Insert, C>`, `On<Replace, C>`, `On<Remove, C>`. Entity events derive `EntityEvent` and carry their target.
- **Hierarchy:** `ChildOf(parent)` relationship component + auto-maintained `Children`. `.despawn()` is recursive by default.
- **Queries:** `query.single()` / `single_mut()` return `Result` — use `?` in fallible systems or `let Ok(x) = ... else { return; }`.
- **Time:** `time.delta_secs()`, `time.elapsed_secs()`.
- **UI:** style fields live on `Node` itself. Text = `Text::new("...")` + `TextFont` + `TextColor`; world text = `Text2d`. UI rotation/scale via `UiTransform` (`Rot2`), not `Transform`.
- **States:** `#[derive(States)]` + `app.init_state::<S>()`; scoped despawn via `DespawnOnExit(state)` / `DespawnOnEnter(state)`.
- **Scenes:** glTF and world scenes spawn via `WorldAssetRoot` + `WorldInstanceReady` (`bevy::world_serialization`); `SceneRoot`/`SceneInstanceReady` are gone. BSN (`bsn!`/`bsn_list!`) is the authoring layer on top.
- **Fallible systems** returning `Result` are idiomatic; `?` on `single()`, asset lookups, etc.

## What changed in 0.20 (the deltas that bite)

**Scenes / BSN — breaking, and this repo already migrated:**

- All scene references need `@`: `@scene_fn()`, `@scene_var`, `@{expr}`, `@SceneComponent`.
- Scene-list entities are separated by `--`; commas and grouping `()` are deprecated.
- `template_value(x)` is deprecated — write `x`; use `~x` only if the plain form fails.
- Enums no longer need `VariantDefaults`; `Default + Clone` is enough, but **every variant field must be specified**.

**Rendering / shaders:**

- **Shaders are WESL.** naga_oil is gone. `#import` → `import …;` (first in file, semicolons), `#ifdef` → `@if(…)`, `#{MATERIAL_BIND_GROUP}` → `constants::MATERIAL_BIND_GROUP`, `#define_import_path` gone. Custom shaders using any preprocessor feature must be renamed `.wgsl` → `.wesl`; plain WGSL with no directives still works. GLSL support removed. Module paths match crate paths (`bevy_pbr::render::mesh_view_bindings`).
- **Sprite materials** (`MaterialExtension2d` + `ExtendedMaterial2d`, or `SpriteMaterial` + `SpriteMaterialPlugin`) let sprites use custom shaders; the sprite render backend was unified with the 3D one.
- Mesh shaders (task + mesh) are usable via `MeshPipelineDescriptor` + `draw_mesh_tasks` — advanced, not web-supported.

**UI:**

- `TextFont::default()` is now `FontSize::Rem(1.)` (was `Px(20.)`) — default text now scales with the `RemSize` resource. Set `FontSize::Px(..)` to pin a size.
- `Val::Em` / `Val::Rem` + `em()`/`rem()` helpers; `Node` requires `EmSize`. `Val::resolve` and friends take `em_size` + `rem_size` arguments.
- `BorderRadius` fields are `CornerRadius` (elliptical corners); `BorderRadius::all(px(..))` still works via `Into`.
- `FixedNode` positions relative to the camera viewport (ignores parent layout/clipping/transform).
- `EditableText` split: `bevy::text::EditableText` is state only; widget behavior moved to `bevy::ui_widgets::TextInput` + `TextInputPlugin`.
- Click-to-focus moved out of `InputFocusPlugin` into `bevy_input_focus::pointer_focus::PointerFocusPlugin` (add it explicitly; it is not in `DefaultPlugins`).
- Overflow clipping now respects rotation/scale (CPU Sutherland–Hodgman); API unchanged.
- `InlineBox` (bevy_text) reserves text-layout space for custom content; `InlineImage` (bevy_ui) intersperses images with text.

**ECS / scheduling / errors:**

- **Weak ordering:** `chain_weak()`, `before_weak()`, `after_weak()` order only systems whose data access actually conflicts. Many built-in sets (render, UI) now use them — if you relied on untracked side-channel ordering (atomics, channels, interior mutability), add an explicit `.after(..)`.
- **Bulk despawn:** `despawn_all` / `despawn_all_where` (also on `Commands`) batch despawning; `DespawnOnEnter`/`DespawnOnExit` use it for free.
- **Panics are errors:** panics in systems/commands/observers are converted to `BevyError` and routed to `FallbackErrorHandler` (default still re-panics — `Severity::Panic`). Change the handler to log-and-continue for long-running tools.
- **`BevyError::context` / `with_context`** attach anyhow-style context; a chain of contexts prints as a "Caused by:" list.
- **Per-column change ticks:** opt in with `#[component(summary_tick)]` when a component is queried for changes often but written rarely (mutations get costlier).
- **`ScheduleBuildSettings::shuffle_seed`** (with the `debug` feature) randomizes run order while keeping explicit constraints, to flush out accidental ordering.
- **`PanOrbitCamera`** ships in `bevy_camera_controller` (feature `pan_orbit_camera`) — `DefaultPanOrbitCameraPlugins` + `MeshPickingPlugin`.
- Feathers gained contextual theming (`ThemeContext` + `SemanticToken`; `ThemeProps` reorganized) and new widgets — see `bevy-feathers-bsn`.

## Project-local conventions (Lifthrasir)

- World up is **-Y** — lift things above units with negative Y offsets.
- Companion skills: `bevy-feathers-bsn` (BSN scenes + Feathers widgets — mandatory for UI authoring), `lifthrasir-bevy-plugins` (this repo's third-party crate APIs and wiring), `bevy-cheatbook` (0.18-era triage tables; this skill supersedes it for current API questions).
- Audio goes through `bevy_kira_audio`, not Bevy's built-in audio.
- Settings use Bevy's own `bevy::settings` (`SettingsGroup`), not a third-party persistence crate.
- Never launch the client to verify; use BRP against a running instance or tests.
