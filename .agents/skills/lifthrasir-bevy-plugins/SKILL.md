---
name: lifthrasir-bevy-plugins
description: Use when working in the Lifthrasir codebase with its third-party Bevy 0.20 ecosystem crates — bevy_auto_plugin (the auto_* macro plugin system), bevy_common_assets, iyes_progress, leafwing-input-manager, bevy_kira_audio (spatial), bevy_feathers, bevy_hanabi, bevy::settings, moonshine-kind/-tag/-behavior, bevy_framepace, bevy_brp_extras, bevy_quinnet, bevy_egui. Covers the correct APIs, this project's wiring, and known gotchas.
---

# Lifthrasir Bevy ecosystem plugins

Reference for the third-party Bevy crates wired into Lifthrasir, verified against the versions pinned in the workspace `Cargo.toml` (Bevy **0.20.0**). Covers how each is used here and the gotchas.

For `bevy_feathers` BSN/widget authoring patterns specifically, see the `bevy-feathers-bsn` skill — this skill only covers how the ecosystem pieces are wired into this repo. For core Bevy see the `bevy` skill.

## Dependency map (what is actually in the workspace)

| Crate | Version | Purpose | Where used |
|---|---|---|---|
| `bevy_auto_plugin` | 0.12 | macro-driven plugin registration (the backbone) | `lifthrasir-client`, `lifthrasir-runtime`, `net-aesir`, `net-contract` |
| `bevy_common_assets` | 0.18 (`toml`, `ron`) | typed TOML/RON assets | `lifthrasir-client` (toml+ron), `lifthrasir-assets` (ron) |
| `iyes_progress` | 0.18 | asset-load progress tracking | `lifthrasir-client`, `lifthrasir-client-ui` (loading screen) |
| `leafwing-input-manager` | 0.22 | action→binding input maps | `lifthrasir-client`, `lifthrasir-client-ui` |
| `bevy_kira_audio` | 0.27 (`mp3`,`wav`,`ogg`) | audio backend, incl. spatial | `lifthrasir-client`, `lifthrasir-runtime`, `lifthrasir-editor` |
| `bevy_feathers` | 0.20.0 | Bevy UI widget/theme toolkit | `lifthrasir-ui`, `lifthrasir-client-ui` |
| `bevy_hanabi` | git pin, `3d` only | GPU particle VFX | `lifthrasir-runtime` (plugin registered, no effects authored) |
| `bevy::settings` | `bevy_settings` feature | persisted settings groups | `apps/lifthrasir`, `lifthrasir-client` |
| `moonshine-kind` / `-tag` / `-behavior` | 0.6 | entity kinds / runtime tags / validated state machines | runtime + client + assets |
| `bevy_framepace` | git pin (`bevy0p20` branch) | frame-rate limiter | `lifthrasir-client`, `apps/lifthrasir` |
| `bevy_brp_extras` | 0.23 | BRP extras (screenshots, key injection) | `apps/lifthrasir`, `dev` feature only |
| `bevy_quinnet` | 0.22 | QUIC transport | `net-aesir` only |
| `bevy_egui` | 0.43 | immediate-mode dev UI | `lifthrasir-editor` only |
| `transform-gizmo` | 0.11 | transform gizmo math/draw | `lifthrasir-editor` only |

**Not dependencies anymore — don't suggest them:** `bevy_asset_loader`, `bevy-persistent`, `bevy_ui_text_input`, `bevy-tokio-tasks`, `transform-gizmo-bevy`. Loading uses `iyes_progress`; settings use `bevy::settings`; hotbar/other files are written by hand.

Both `bevy_framepace` and `bevy_hanabi` are pinned to git revs because no 0.20 release exists yet; check the comments in the root `Cargo.toml` before bumping. `bevy_hanabi` is built with `default-features = false, features = ["3d"]` on purpose: its 2D path also draws world effects through the UI `Camera2d`.

---

## bevy_auto_plugin (0.12) — the plugin backbone

Plugin registration is distributed across the crate: annotate items anywhere with `#[auto_*(plugin = SomePlugin)]`, and the plugin struct's generated `build()` collects them. Mechanism: the `inventory` crate — each macro emits an `inventory::submit!`, and the generated `Plugin::build` iterates the link-time registry. Import: `use bevy_auto_plugin::prelude::*;`

### Declaring a plugin

```rust
use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]   // generates the whole `impl Plugin`
pub struct LifthrasirPlugin;
```

`auto_plugin(...)` args: `impl_plugin_trait` (generate the full impl — this project's standard), `plugin = <Path>` (when annotating a standalone `fn`/method), `generics(...)`, `default_plugin` (lets you omit `plugin =`). To append to a hand-written `build()`, put `#[auto_plugin]` on the `fn build` method instead.

### 0.12 prelude, grouped

| Group | Macros |
|---|---|
| Registration (actions) | `auto_add_system`, `auto_add_message`, `auto_add_observer`, `auto_add_plugin`, `auto_init_resource`, `auto_insert_resource`, `auto_init_asset`, `auto_init_state`, `auto_init_sub_state`, `auto_register_type`, `auto_register_state_type`, `auto_register_asset_reflect`, `auto_name`, `auto_configure_system_set`, `auto_run_on_build`, `auto_bind_plugin`, `auto_plugin_build_hook` |
| Derive/rewrite (applied to a type) | `auto_component`, `auto_resource`, `auto_asset`, `auto_scene_component`, `auto_message`, `auto_event`, `auto_states`, `auto_sub_states`, `auto_system`, `auto_observer` |
| Derive | `AutoPlugin`, `auto_plugin` |

Note the naming split: `auto_add_*` **registers** something with the plugin; `auto_*` (no `add`) **rewrites a declaration** (adds derives/attributes to the item).

```rust
#[auto_add_system(plugin = NativeInputPlugin, schedule = Update,
    config(before = InputSystems::Raycast, run_if = ui_unfocused))]
fn forward_cursor_position(/* ... */) {}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(plugin = crate::InputPlugin, schedule = Update,
    chain, config(run_if = in_state(GameState::InGame)))]
pub enum InputSystems { Raycast, Cursor, Click }   // chain => declaration order
```

`config(...)` keys for `auto_add_system`: `before`, `after`, `in_set`, `run_if`. For chaining whole sets, use `auto_configure_system_set(..., chain)` rather than `chain` on a system.

### ⚠️ `auto_add_message` vs observers

- **`#[derive(Message)]` (buffered, `MessageReader`/`MessageWriter`) → `#[auto_add_message]`.** This is what every `net-contract` event/command uses.
- **Observer-only events (`On<E>` / `.trigger`) → `#[auto_add_observer]` on the observer fn.**
- `auto_add_event` does not exist in 0.12 — using it (or `auto_add_event` on a `Message`) is a hard compile error, not a warning. In 0.12 `auto_event` is a type-rewrite macro, not a registration macro.
- `auto_insert_resource` takes `insert(...)` (the old `init(...)`/`resource(...)` args were removed).

---

## Asset loading: bevy_common_assets + iyes_progress

`bevy_common_assets` teaches Bevy how to load TOML/RON into a typed asset. Progress tracking is `iyes_progress`, not `bevy_asset_loader`.

```rust
use bevy_common_assets::toml::TomlAssetPlugin;

#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig { pub assets: AssetsSection }

app.add_plugins((
    TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
    TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
));
// consume later: configs: Res<Assets<AssetConfig>> -> configs.get(&handle)
```

```rust
use iyes_progress::prelude::ProgressTracker;

fn loading_report(tracker: Res<ProgressTracker<GameState>>) {
    let progress = tracker.get_global_progress();   // Progress { done, total }
    if progress.total == 0 { return; }
    let percent = progress.done as f32 / progress.total as f32 * 100.0;
}
```

See `crates/lifthrasir-client-ui/src/screens/loading.rs` for the live usage.

---

## bevy_kira_audio (0.27)

**Replaces Bevy's built-in audio entirely** — only `KiraAudioPlugin` is added (`bevy::audio::AudioPlugin` is disabled in `apps/lifthrasir/src/main.rs`). Alias to avoid the name clash: `use bevy_kira_audio::AudioPlugin as KiraAudioPlugin;`

```rust
fn play_bgm(audio: Res<Audio>, mut instances: ResMut<Assets<AudioInstance>>, assets: Res<AssetServer>) {
    let source: Handle<AudioSource> = assets.load("bgm/login.ogg");
    let handle = audio.play(source).looped().with_volume(0.0).handle();   // -> Handle<AudioInstance>
    if let Some(inst) = instances.get_mut(&handle) {
        inst.set_decibels(0.0, AudioTween::linear(Duration::from_secs_f32(2.0)));   // fade-in
    }
}
```

`AudioTween` is `Copy` in 0.27 — pass it by value, don't `.clone()` it (clippy flags it). Patterns here: BGM crossfade via `instance.stop(AudioTween::linear(..))` on the outgoing track while fading in the new one, state in `BgmManager`.

### Channels + spatial audio

`ContentAudioPlugin` (`crates/lifthrasir-runtime/src/audio/plugin.rs`) owns Kira, `SpatialAudioPlugin` and the channels. The client audio plugin bridges settings/BGM only; it must not install a second backend.

```rust
use bevy_kira_audio::prelude::{AudioApp, SpatialAudioPlugin};
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, DefaultSpatialRadius};

app.add_plugins((KiraAudioPlugin, SpatialAudioPlugin))
   .add_audio_channel::<SfxChannel>()
   .add_audio_channel::<AmbienceChannel>()
   .insert_resource(DefaultSpatialRadius { radius: 150.0 });
```

Spatial falloff needs exactly one `SpatialAudioReceiver` (on the local player, or the editor camera) and a `SpatialAudioEmitter` on the source. Play on the channel, then push the handle into the emitter's `instances`:

```rust
let handle = sfx_channel.play(asset_server.load(&path)).handle();
match emitters.get_mut(entity) {
    Ok(mut emitter) => emitter.instances.push(handle),
    Err(_) => { commands.entity(entity).insert(SpatialAudioEmitter { instances: vec![handle] }); }
}
```

---

## bevy::settings (built into Bevy 0.20's `bevy_settings` feature)

Settings persistence is Bevy's own framework now — `bevy-persistent` is gone. A settings type is a `Resource` deriving `SettingsGroup` (and `Reflect`), registered under a group name:

```rust
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup};

#[derive(Resource, SettingsGroup, Clone, Copy, PartialEq, Eq, Reflect, Debug)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "graphics")]
pub struct GraphicsSettings { /* fields */ }
```

```rust
// apps/lifthrasir/src/main.rs — one plugin, keyed by an app id
app.add_plugins(bevy::settings::SettingsPlugin::new("com.github.endurnyrproject.lifthrasir"));

// persist on apply (UI handler):
commands.queue(SaveSettings::IfChanged);
```

Values are loaded by the plugin and read as ordinary resources; `ApplySettings`-style events fan them out to live state (see `crates/lifthrasir-client/src/domain/settings/`). Files other than settings groups (e.g. the hotbar) are written by hand with `serde`/RON under `dirs::config_dir()` — `crates/lifthrasir-client/src/domain/hotbar/persistence.rs`.

---

## leafwing-input-manager (0.22)

Actions are an enum; an `InputMap<PlayerAction>` holds the bindings, `ActionState<PlayerAction>` the per-frame state.

```rust
app.add_plugins(InputManagerPlugin::<PlayerAction>::default());

fn on_sit(actions: Res<ActionState<PlayerAction>>) {
    if actions.just_pressed(&PlayerAction::Sit) { /* ... */ }
}
```

The map is rebuilt from persisted keybinds when settings apply (`crates/lifthrasir-client/src/domain/settings/apply.rs`), so keybinding changes take effect without restarting.

---

## moonshine-kind / -tag / -behavior (0.6)

**moonshine-kind** — classify entities by logical Kind via a query filter, then query `Instance<K>`:
```rust
use moonshine_kind::{Kind, CastInto};
pub struct MonsterKind;
impl Kind for MonsterKind { type Filter = With<Mob>; }
impl CastInto<Animated> for MonsterKind {}   // hierarchical kind relationship
```

**moonshine-tag** — runtime tag values (not component types), used for sprite layers/frames:
```rust
use moonshine_tag::Tag;
moonshine_tag::tags! { pub LAYER_BODY, pub LAYER_HEAD, pub LAYER_WEAPON, pub FRAME_ATTACK }
fn order(t: Tag) -> u8 { if t == LAYER_HEAD { 10 } else { 20 } }
```

**moonshine-behavior** — validated state machines; the primary one is `AnimationState`. Transitions must pass `filter_next`; query mutably via `BehaviorMut<T>`:
```rust
use moonshine_behavior::prelude::*;

#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[component(storage = "SparseSet")]
pub enum AnimationState { #[default] Idle, Walking, Attacking, Hit, Dead }

impl Behavior for AnimationState {
    fn filter_next(&self, next: &Self) -> bool {
        use AnimationState::*;
        match (self, next) { (Dead, _) => false, (a, b) if a == b => true, _ => true }
    }
}

app.add_plugins(BehaviorPlugin::<AnimationState>::default());
// in a system:  mut behaviors: Query<BehaviorMut<AnimationState>>
```

---

## App / UI crates

### bevy_feathers (0.20.0)

Wired for windows, theme, and the RO cursor (`FeathersPlugins`/`FeathersCorePlugin`, `UiTheme` overrides, `bsn!` scenes). Widget/scene authoring belongs to the `bevy-feathers-bsn` skill; two 0.20 wiring facts that bite here:

1. **Pointer focus is not in Feathers or `DefaultPlugins`.** Add `bevy::input_focus::pointer_focus::PointerFocusPlugin` yourself (`crates/lifthrasir-ui/src/plugin.rs` does, with `InputFocusPlugin`/`InputDispatchPlugin`), plus `register_required_components::<EditableText, TabIndex>()`, or clicks never focus a field.
2. **Theme shape changed.** `create_dark_theme()` returns `ThemeProps` with `token_assignments` / `semantic_base` / `semantic_overrides`; there is no `ThemeProps::color` map to insert into. Use `assign_token` in `crates/lifthrasir-ui/src/theme/feathers_theme.rs`.

**Cursor gotcha (owned here because it is project wiring):** Feathers' `CursorIconPlugin` (bundled in `FeathersPlugins`) writes the window `CursorIcon` every `PreUpdate`, so never set the window cursor directly — it gets stomped. Drive the RO cursor through the **`OverrideCursor` resource**, which lives in `bevy::picking::cursor` (not `bevy_feathers::cursor`):

```rust
use bevy::picking::cursor::{EntityCursor, OverrideCursor};
override_cursor.0 = Some(EntityCursor::Custom(CustomCursor::Image(/* ... */)));
```

Custom cursor images require Bevy's own `custom_cursor` feature. That feature is on `bevy` (enabled in the root `Cargo.toml`); `bevy_feathers` 0.20 has **no** `custom_cursor` feature. See `crates/lifthrasir-client-ui/src/cursor.rs`.

### bevy_hanabi (git pin, `3d` only) — plugin registered, no effects authored yet

`EffectsPlaybackPlugin` (`crates/lifthrasir-runtime/src/effects/plugin.rs`) owns `HanabiPlugin` when a renderer is available. **Add it in exactly one place**; the client's VFX plugin owns gameplay bridges, not Hanabi. The shared CPU subset deliberately omits GPU initialization.

No `EffectAsset`/`ParticleEffect` exists in the repo yet — the portal effect uses a hand-written `Material` + shader, not Hanabi particles. Adding particles (sparks, dust, spell trails) is greenfield: author an `EffectAsset` with `ExprWriter` and spawn `ParticleEffect` under the existing `VfxSystems` ordering. Note the crate is built **without 2D support** on purpose.

### bevy_framepace (git pin, `bevy0p20` branch)

`FramepacePlugin` is added once in `apps/lifthrasir/src/main.rs`; the limiter is driven by the user's FPS-cap setting in `crates/lifthrasir-client/src/domain/settings/apply.rs`:

```rust
// inside the ApplySettings handler in domain/settings/apply.rs
fn apply_graphics(mut framepace: ResMut<FramepaceSettings>, graphics: Res<GraphicsSettings>) {
    framepace.limiter = graphics.fps_cap.to_limiter();   // FpsCap -> bevy_framepace::Limiter
}
```

### bevy_brp_extras (0.23) — `dev` cargo feature only

Pairs with Bevy's `bevy_remote` feature and adds BRP extras (screenshots, key injection). Gated by the `dev` Cargo feature on the `lifthrasir` binary, **not** `#[cfg(debug_assertions)]`:

```rust
#[cfg(feature = "dev")]
app.add_plugins((
    bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
    bevy_brp_extras::BrpExtrasPlugin::default(),
));
```

### Editor-only crates

`lifthrasir-editor` is the only consumer of `bevy_egui` 0.43 and `transform-gizmo` 0.11 (the `transform-gizmo-bevy` wrapper was dropped — the gizmo is wired directly). Do not add either to the client.

---

## Quick gotchas

- New buffered event? `#[derive(Message)]` + `#[auto_add_message]`. `auto_add_event` does not exist in `bevy_auto_plugin` 0.12.
- New system? Annotate with `#[auto_add_system(plugin = …, schedule = …)]` — don't hand-edit a `build()`; the impl is generated.
- Audio types come from `bevy_kira_audio`, not `bevy::audio` (alias `AudioPlugin`). SFX/ambience play on `AudioChannel<SfxChannel>`/`AudioChannel<AmbienceChannel>`, not the default `Audio` resource. `AudioTween` is `Copy`.
- Never set the window `CursorIcon` directly — Feathers overwrites it every `PreUpdate`. Use `bevy::picking::cursor::OverrideCursor`.
- Text fields need `PointerFocusPlugin` + a `TabIndex`; fewer installs mean clicks silently don't focus.
- `bevy_hanabi`'s `HanabiPlugin` must only be added once (in `EffectsPlaybackPlugin`), and it is built 3D-only.
- Settings persist through `bevy::settings` (`SettingsGroup` + `SaveSettings::IfChanged`), not a third-party crate; hand-rolled files (hotbar) go through `std::fs` under `dirs::config_dir()`.
- `bevy_brp_extras` needs `--features dev`, not a debug build.
