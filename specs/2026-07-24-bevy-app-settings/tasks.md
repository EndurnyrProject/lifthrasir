# Bevy App Settings Migration — Implementation Tasks

> Generated from architecture doc:
> `specs/2026-07-24-bevy-app-settings/architecture.md`
> (spec: `specs/2026-07-24-bevy-app-settings/spec.md`)
> Each task below is **one commit**. Implement top to bottom; respect
> `Depends on`.
> Tasks sharing a wave in `## Execution Waves` may be implemented in parallel.

**Goal:** Replace Lifthrasir's custom aggregate settings persistence with three
native Bevy 0.19 settings resources while preserving the existing
Apply/Cancel/Reset and runtime synchronization behavior.

---

## Progress

- [ ] Task 1: Establish native Bevy settings groups and bootstrap
- [ ] Task 2: Cut runtime and UI behavior over to native resources
- [ ] Task 3: Delete the legacy settings persistence stack

---

## Execution Waves

> Tasks in the same wave have no dependencies on each other and touch disjoint
> files — they can be implemented in parallel. Waves run in order; a wave
> starts only after the previous one is fully merged and green.

- Wave 1: Task 1
- Wave 2: Task 2
- Wave 3: Task 3

The behavioral cutover is intentionally one task. Splitting graphics, audio,
input, or the settings UI into separate commits would temporarily make parts of
the application read different persistence stores or require a compatibility
bridge that the approved design explicitly rejects.

---

## Task 1: Establish native Bevy settings groups and bootstrap

**What:** Implement the architecture's **Application bootstrap** and **Native
settings groups** foundation without changing existing consumers yet. Enable
Bevy's settings feature, register graphics, audio, and keybinds as explicit
`SettingsGroup` resources in the default `settings.toml`, and install Bevy's
plugin after `DefaultPlugins`.

**Code pointers:**

- Modify: `Cargo.toml:19-28` — enable the `bevy_settings` feature on the existing
  workspace `bevy` dependency; keep `bevy-persistent` until Task 3.
- Modify: `game-engine/src/domain/settings/resources.rs:463-531` — make
  `GraphicsSettings` and `AudioConfig` native reflected settings resources with
  explicit `graphics` and `audio` group names and reflected `Default` metadata.
- Modify: `game-engine/src/domain/settings/resources.rs:640-725` — make
  `Keybinds` a native reflected settings resource with the explicit `keybinds`
  group name.
- Modify: `game-engine/src/domain/settings/resources.rs:738-1120` — add focused
  assertions for the three `SettingsGroup` names and default settings source;
  preserve existing defaults and mapping tests.
- Modify: `lifthrasir/src/main.rs:23-69` — add
  `bevy::settings::SettingsPlugin::new("com.github.endurnyrproject.lifthrasir")`
  immediately after `DefaultPlugins` and before every engine, network, and UI
  plugin that consumes settings.
- Reference:
  `specs/2026-07-24-bevy-app-settings/architecture.md#native-settings-groups`
  — required derives, reflection metadata, group names, and one-file layout.

**Acceptance criteria:**

- [ ] `GraphicsSettings`, `AudioConfig`, and `Keybinds` each derive `Resource`,
  `SettingsGroup`, and `Reflect`, and register `Resource`, `SettingsGroup`, and
  `Default` reflection metadata.
- [ ] Their explicit group names are exactly `graphics`, `audio`, and
  `keybinds`, and all three use Bevy's default `settings.toml` source.
- [ ] Existing manual `Default` implementations and user-visible values are
  unchanged.
- [ ] The native plugin uses the stable application identifier
  `com.github.endurnyrproject.lifthrasir` and is added after `DefaultPlugins`
  but before `MapPlugin`, `CoreGamePlugins`, the network adapter, and
  `LifthrasirUiPlugin`.
- [ ] No direct `bevy-settings` dependency or Lifthrasir registration wrapper is
  introduced.
- [ ] `cargo test -p game-engine domain::settings::resources` passes.
- [ ] `cargo check -p lifthrasir` passes.

**Depends on:** none

**Commit:** `feat(settings): register native Bevy settings groups`

---

## Task 2: Cut runtime and UI behavior over to native resources

**What:** Implement the architecture's **Runtime synchronization**, **Settings
UI draft**, and **Direct settings consumers** sections as one atomic cutover.
Every consumer reads its narrow native resource, the UI owns a non-persisted
three-group draft, Apply queues Bevy's save command, and the old persistence
module is no longer compiled or installed.

**Code pointers:**

- Modify: `game-engine/src/domain/settings/mod.rs:1-29` — stop declaring and
  exporting the persistence module/path, remove the aggregate `Settings`
  re-export, rename the custom plugin to `SettingsRuntimePlugin`, and retain
  `emit_initial_apply` in `PostStartup`.
- Modify: `game-engine/src/domain/settings/events.rs:1-8` — attach
  `ApplySettings` to `SettingsRuntimePlugin` and describe it as runtime
  synchronization rather than persistence.
- Modify: `game-engine/src/domain/settings/resources.rs:728-780` — delete the
  aggregate `Settings` resource and its aggregate RON tests; leave the three
  native groups and their domain helpers intact.
- Modify: `game-engine/src/domain/settings/apply.rs:1-434` — replace
  `Persistent<Settings>` access with `Res<GraphicsSettings>`,
  `Res<AudioConfig>`, or `Res<Keybinds>` as appropriate; make graphics helpers
  accept `&GraphicsSettings`; move auto-system registration to
  `SettingsRuntimePlugin`; replace persistent test fixtures with plain
  resources.
- Modify: `game-engine/src/lib.rs:27-85` — export and install
  `SettingsRuntimePlugin` in `CoreGamePlugins`.
- Modify: `game-engine/src/domain/world/terrain.rs:1-819` — read
  `GraphicsSettings` directly for anisotropy reapplication, terrain generation,
  and loaded terrain textures.
- Modify:
  `game-engine/src/infrastructure/assets/animation_processing_system.rs:1-118`
  — read `GraphicsSettings` directly for sprite upscaling.
- Modify: `game-engine/src/presentation/rendering/models.rs:1-389` — read
  `GraphicsSettings` directly for model texture upscaling.
- Modify: `game-engine/src/domain/effects/sprite_effects.rs:1-232` — pass
  `GraphicsSettings` through sprite-effect asset resolution without a
  persistence wrapper.
- Modify: `game-engine/src/domain/effects/status_visuals.rs:1-421` — read
  `GraphicsSettings` directly when finalizing frozen status visuals.
- Modify:
  `game-engine/src/domain/entities/sprite_rendering/systems/cart.rs:1-226` —
  read `GraphicsSettings` directly when finalizing cart layers.
- Modify: `game-engine/src/domain/emote/assets.rs:1-80` — read
  `GraphicsSettings` directly when finalizing emote assets.
- Modify: `game-engine/src/domain/character/local_player.rs:1-101` — read
  `Keybinds` directly when creating the local player's `InputMap`.
- Modify: `lifthrasir-ui/src/widgets/settings_window/mod.rs:1-1112` — introduce
  `SettingsDraft { graphics, audio, keybinds }`; implement `SettingsUi` with
  `FromWorld` snapshots of the three loaded resources; remove
  `seed_from_persistent`; commit only differing resources on Apply; queue
  `SaveSettings::IfChanged`; then update the committed snapshot and write
  `ApplySettings`; rewrite draft tests around plain resources.
- Reference:
  `game-engine/src/domain/settings/apply.rs:209-238` — retain the existing
  camera and directional-light `Added` hooks while narrowing their settings
  resource.
- Reference:
  `lifthrasir-ui/src/widgets/settings_window/mod.rs:59-90` — preserve the
  current dirty, Cancel, Reset, and clean-Apply semantics while changing the
  backing types.

**Acceptance criteria:**

- [ ] Production code outside the now-orphaned
  `game-engine/src/domain/settings/persistence.rs` contains no
  `Persistent<Settings>` or `bevy_persistent` use.
- [ ] Graphics consumers read only `GraphicsSettings`, audio synchronization
  reads only `AudioConfig`, and input consumers read only `Keybinds`.
- [ ] `SettingsRuntimePlugin` still emits the initial `ApplySettings` message,
  applies window/render/audio/input state, and retains late camera/light hooks.
- [ ] `SettingsUi::from_world` initializes `draft` and `committed` from the
  three already-loaded resources; the first-Update persistence seeding system
  is gone.
- [ ] A dirty Apply assigns only groups whose values differ, updates
  `committed`, queues `SaveSettings::IfChanged`, and writes one
  `ApplySettings`; a clean Apply performs none of those actions.
- [ ] Apply comparisons occur before mutable dereferencing so unchanged groups
  are not falsely marked changed.
- [ ] Cancel restores the committed draft and clears key capture; Reset changes
  only the draft until Apply; closing without Apply leaves active resources
  unchanged.
- [ ] Tests cover `SettingsUi::from_world`, dirty and clean Apply, Cancel,
  Reset, graphics/audio/keybind draft controls, audio synchronization, and
  retained graphics mapping behavior.
- [ ] `cargo test -p game-engine -p lifthrasir-ui` passes.
- [ ] `cargo check -p lifthrasir` passes.

**Depends on:** Task 1

**Commit:** `refactor(settings): cut over to native Bevy resources`

---

## Task 3: Delete the legacy settings persistence stack

**What:** Complete the architecture's **Removed persistence implementation**
and verification sections. Delete the orphaned custom loader, old aggregate
schema remnants, compatibility-only serialization code/tests, and
`bevy-persistent` dependencies while preserving the independent hotbar
persistence boundary.

**Code pointers:**

- Delete: `game-engine/src/domain/settings/persistence.rs` — custom
  `settings.ron` path, startup loader, recovery, and
  `Persistent<Settings>` construction.
- Modify: `game-engine/src/domain/settings/resources.rs:1-1120` — remove
  settings-only Serde derives/attributes and RON compatibility/round-trip tests;
  preserve reflection traits, native group tests, defaults, cycling, labels,
  render mappings, and `Keybinds::to_input_map`.
- Modify: `Cargo.toml:19-43` — remove the workspace `bevy-persistent`
  dependency; retain `serde` and RON because other workspace code still uses
  them.
- Modify: `game-engine/Cargo.toml:6-46` — remove `bevy-persistent`; retain
  `dirs`, `ron`, and `serde` for hotbar and other non-settings consumers.
- Modify: `lifthrasir-ui/Cargo.toml:6-15` — remove its direct
  `bevy-persistent` dependency.
- Modify: `Cargo.lock` — regenerate the lockfile after removing the dependency.
- Reference: `game-engine/src/domain/hotbar/persistence.rs:20-92` — this
  per-character RON persistence remains unchanged and is the intended `dirs`
  consumer.

**Acceptance criteria:**

- [ ] Production and test source contains no `Persistent<Settings>`,
  `settings_path`, custom settings loader, aggregate `Settings` resource, or
  settings-specific legacy RON compatibility path.
- [ ] No workspace or crate manifest depends on `bevy-persistent`, and the
  lockfile is updated.
- [ ] `dirs` remains in `game-engine` and is used by the unchanged hotbar
  persistence boundary; RON and Serde remain available for non-settings uses.
- [ ] The old `settings.ron` is not read, migrated, rewritten, or deleted.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo check --workspace` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Manual smoke verification confirms defaults on first launch; Apply
  updates graphics, audio, and input; a restart reloads all three groups from
  Bevy's TOML; Cancel/Reset/dirty-state still work; and late-created cameras,
  lights, terrain, effects, models, and local-player input use current values.

**Depends on:** Task 2

**Commit:** `chore(settings): remove legacy persistence stack`
