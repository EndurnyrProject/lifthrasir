---
name: lifthrasir-bevy-plugins
description: Use when working in the Lifthrasir codebase with its third-party Bevy 0.19 ecosystem crates — bevy_auto_plugin (the auto_* macro plugin system), bevy_common_assets, bevy_asset_loader, bevy_kira_audio (spatial), bevy_feathers, bevy_hanabi, bevy-persistent, moonshine-kind/-tag/-behavior, bevy_framepace, bevy_brp_extras. Covers correct APIs, this project's conventions, and known gotchas.
---

# Lifthrasir Bevy ecosystem plugins

Reference for the third-party Bevy crates wired into the Lifthrasir project, with APIs verified for the pinned versions (Bevy 0.19.0). Covers how each is used here and the gotchas.

For `bevy_feathers` BSN/widget authoring patterns specifically, see the `bevy-feathers-bsn` skill — this skill only covers how Feathers is wired into this repo.

## Crate map

| Crate | Version | Purpose | Status in repo |
|---|---|---|---|
| `bevy_auto_plugin` | 0.11.0 | macro-driven plugin registration (the backbone) | used everywhere (~100+ `auto_*` attrs) |
| `bevy_common_assets` | 0.17 (`toml`, `ron`) | load assets from TOML/JSON/RON | fully wired |
| `bevy_asset_loader` | 0.27 | asset-collection loading states | fully wired |
| `bevy_kira_audio` | 0.26 (`mp3`,`wav`,`ogg`) | audio (replaces Bevy's audio), now spatial | fully wired |
| `bevy_feathers` | 0.19.0 (`custom_cursor`) | Bevy UI widget/theme toolkit | fully wired (windows, theme, cursor) |
| `bevy_hanabi` | 0.19 | GPU particle VFX | **plugin registered, no particle effects authored yet** |
| `bevy-persistent` | 0.11 (`ron`) | disk-backed `Persistent<T>` resource | fully wired (settings) |
| `moonshine-kind` | 0.5 | entity classification by Kind | used (sprite kinds) |
| `moonshine-tag` | 0.5 | runtime entity tags | used (sprite layers) |
| `moonshine-behavior` | 0.5 | validated state machines | used extensively (`AnimationState`) |
| `bevy_framepace` | git `hacknus/bevy_framepace@bevy_0.19` | frame-rate limiter | wired, limiter driven by user FPS-cap setting |
| `bevy_brp_extras` | 0.20.1 | Bevy Remote Protocol extras | wired (`dev` cargo feature only) |

**Removed since the 0.18 era:** `bevy_ui_text_input` and `bevy-tokio-tasks` are no longer dependencies anywhere in the workspace — don't suggest them.

---

## bevy_auto_plugin (0.11.0) — the plugin backbone

Distributes plugin registration across the crate: you annotate items anywhere with `#[auto_*(plugin = SomePlugin)]`, and the plugin struct's generated `build()` collects them. **Mechanism:** uses the `inventory` crate — each macro emits an `inventory::submit!`, and the generated `Plugin::build` iterates them (link-time global registry). Import: `use bevy_auto_plugin::prelude::*;`

### Declaring a plugin

```rust
use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]   // generates the full `impl Plugin` for you
pub struct LifthrasirPlugin;
```

`auto_plugin(...)` args: `impl_plugin_trait` (generate the whole impl — this project's standard), `plugin = <Path>` (when annotating a standalone `fn`/method instead), `generics(...)` (concrete generic-plugin instances), `default_plugin` (lets you omit `plugin =` on the `auto_*` macros). To append to a hand-written `build()`, put `#[auto_plugin]` on the `fn build` method instead of `impl_plugin_trait` on the struct.

### Macro reference (commonly used here)

| Macro | Generates | Applied to |
|---|---|---|
| `#[auto_add_system(plugin=P, schedule=S, config(...))]` | `app.add_systems(S, sys.<config>)` | a system `fn` |
| `#[auto_configure_system_set(plugin=P, schedule=S, chain, config(...))]` | `app.configure_sets(S, Set.chain()…)` | a `SystemSet` enum |
| `#[auto_init_resource(plugin=P)]` | `app.init_resource::<T>()` (needs `Default`) | `#[derive(Resource)]` |
| `#[auto_insert_resource(plugin=P, resource(...))]` | `app.insert_resource(T(...))` | resource w/ initial value |
| `#[auto_init_state(plugin=P)]` | `app.init_state::<T>()` | `#[derive(States)]` |
| `#[auto_init_sub_state(plugin=P)]` | substate init | `#[derive(SubStates)]` |
| `#[auto_register_state_type(plugin=P)]` | reflection reg for the state | state enum |
| `#[auto_register_type(plugin=P)]` | `app.register_type::<T>()` | reflected type |
| `#[auto_add_message(plugin=P)]` | `app.add_message::<T>()` | `#[derive(Message)]` |
| `#[auto_add_event(plugin=P)]` | `app.add_event::<T>()` (legacy buffered) | observer `#[derive(Event)]` |
| `#[auto_observer(plugin=P)]` | `app.add_observer(sys)` | observer `fn` |
| `#[auto_name(plugin=P)]` | auto `Name` for the component | component |

Full 0.11.0 prelude also exposes: `auto_add_observer`, `auto_add_plugin`, `auto_bind_plugin`, `auto_component`, `auto_event`, `auto_message`, `auto_resource`, `auto_states`, `auto_sub_states`, `auto_run_on_build`, `auto_plugin_build_hook`, plus aliases (`auto_system` = `auto_add_system`).

### `config(...)` keys for `auto_add_system`

Confirmed: `before = ...`, `after = ...`, `in_set = ...`, `run_if = ...`. (`chain`/`ambiguous` mirror Bevy's builder but are unconfirmed — for chaining, prefer `auto_configure_system_set(..., chain)` on the set.)

```rust
#[auto_add_system(plugin = NativeInputPlugin, schedule = Update,
    config(before = InputSystems::Raycast, run_if = ui_unfocused))]
fn forward_cursor_position(/* ... */) {}

#[auto_add_system(plugin = crate::app::combat_plugin::CombatDomainPlugin,
    schedule = Update, config(in_set = CombatSystems::ProcessActions))]
pub fn process_combat_actions(/* ... */) {}
```

```rust
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(plugin = crate::InputPlugin, schedule = Update,
    chain, config(run_if = in_state(GameState::InGame)))]
pub enum InputSystems { Raycast, Cursor, Click }   // chain => declaration order
```

### ⚠️ `auto_add_event` vs `auto_add_message` gotcha

Both exist and are **distinct**, matching Bevy's split:
- **`#[derive(Message)]` (buffered, `MessageReader`/`MessageWriter`) → use `#[auto_add_message]`.** This is the idiomatic pairing and what every `net-contract` event/command uses.
- **`#[derive(Event)]` (observer, `On<E>`/`.trigger`) → use `#[auto_add_event]`.**

**As of 0.11.0 the `auto_add_event` back-compat alias for `Message` types is gone** — it's a hard compile error now, not a deprecation warning. If you see `auto_add_event` on a `#[derive(Message)]` struct in old notes or muscle memory, it will not build; use `auto_add_message`.

Also in 0.11.0: `auto_insert_resource`'s old `init(...)`/`resource(...)` args were removed in favor of `insert(...)`.

---

## Asset pipeline: bevy_common_assets + bevy_asset_loader

They work together: `bevy_common_assets` teaches Bevy *how* to load a TOML file into a typed asset; `bevy_asset_loader` orchestrates *when* via loading states.

```rust
// bevy_common_assets: register a loader for a typed asset
use bevy_common_assets::toml::TomlAssetPlugin;

#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig { pub assets: AssetsSection }

app.add_plugins((
    TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
    TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
));
// consume later: configs: Res<Assets<AssetConfig>>  ->  configs.get(&handle)
```

```rust
// bevy_asset_loader: a collection + a loading state machine
use bevy_asset_loader::prelude::*;

#[derive(AssetCollection, Resource)]
pub struct ConfigAssets {
    #[asset(path = "loader.data.toml")]
    pub config: Handle<AssetConfig>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum AssetLoadingState { #[default] LoadingConfig, SettingUpSources, LoadingAssets, Ready, Error }

app.init_state::<AssetLoadingState>()
   .add_loading_state(
        LoadingState::new(AssetLoadingState::LoadingConfig)
            .continue_to_state(AssetLoadingState::SettingUpSources)
            .load_collection::<ConfigAssets>(),
   );
```

`AssetLoadingState` is a separate machine from the game's `GameState`; it gates the startup asset flow.

---

## bevy_kira_audio (0.26)

**Replaces Bevy's built-in audio entirely** — only `KiraAudioPlugin` is added; Bevy's `AudioPlugin` is not used. Import collision: alias it, e.g. `use bevy_kira_audio::AudioPlugin as KiraAudioPlugin;`

```rust
app.add_plugins(KiraAudioPlugin);

fn play_bgm(audio: Res<Audio>, mut instances: ResMut<Assets<AudioInstance>>, assets: Res<AssetServer>) {
    let source: Handle<AudioSource> = assets.load("bgm/login.ogg");
    let handle = audio.play(source).looped().with_volume(0.0).handle();   // -> Handle<AudioInstance>
    if let Some(inst) = instances.get_mut(&handle) {
        inst.set_decibels(0.0, AudioTween::linear(Duration::from_secs_f32(2.0)));   // fade-in
    }
}
```

Patterns in repo: BGM crossfade via `instance.stop(AudioTween::linear(..))` on the outgoing track while fading in the new one; state held in `BgmManager` (`active_instance`, `fading_out_instances`). Volume/mute via `set_decibels`.

### Channels + spatial audio (SFX/ambience)

`AudioPlugin` (the domain one, `game-engine/src/plugins/audio_plugin.rs`) also wires `SpatialAudioPlugin` and two custom `AudioChannel`s — SFX no longer plays on the default `Audio` resource:

```rust
use bevy_kira_audio::prelude::{AudioApp, SpatialAudioPlugin};
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, DefaultSpatialRadius};

app.add_plugins(KiraAudioPlugin)
   .add_plugins(SpatialAudioPlugin)
   .add_audio_channel::<SfxChannel>()      // marker structs, domain/audio/resources.rs
   .add_audio_channel::<AmbienceChannel>()
   .insert_resource(DefaultSpatialRadius { radius: 150.0 }); // world units; volume->silence falloff
```

Spatial falloff needs exactly one `SpatialAudioReceiver` (put on the local player, see `domain/character/systems.rs`) and a `SpatialAudioEmitter` on the sound source entity. Play on the channel, then push the resulting handle into the emitter's `instances`:

```rust
fn play_mob_sfx(
    sfx_channel: Res<AudioChannel<SfxChannel>>,
    mut emitters: Query<&mut SpatialAudioEmitter>,
    asset_server: Res<AssetServer>,
    /* ... */
) {
    let handle = sfx_channel.play(asset_server.load(&path)).handle();
    if let Ok(mut emitter) = emitters.get_mut(entity) {
        emitter.instances.push(handle);
    }
}
```

If the emitter entity might not have `SpatialAudioEmitter` yet (e.g. a freshly spawned effect anchor), `commands.entity(entity).insert(SpatialAudioEmitter { instances: vec![handle] })` on the `Err` branch instead of skipping it (see `play_skill_sfx`).

---

## moonshine-kind / -tag / -behavior (0.5)

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

### bevy_feathers (0.19.0)

Wired for windows, theme, and the game cursor (`FeathersPlugins`/`FeathersCorePlugin` group, `UiTheme` overrides, `bsn!` scenes). See the `bevy-feathers-bsn` skill for widget/scene authoring patterns — that's the primary reference, not this one.

**⚠️ Gotcha (owned here because it's project wiring, not a Feathers API detail):** `bevy_feathers`'s `CursorIconPlugin` (bundled in `FeathersPlugins`) overwrites the window's `CursorIcon` every `PreUpdate`. Never `insert_resource`/set the window cursor directly — it will be stomped. Drive the RO cursor through Feathers' own `OverrideCursor` resource instead (`bevy_feathers::cursor::{EntityCursor, OverrideCursor}`), which requires the `custom_cursor` feature (already enabled on this workspace's `bevy_feathers` dep). See `lifthrasir-ui/src/cursor.rs`.

### bevy_hanabi (0.19) — plugin registered, no particles authored yet

`VfxPlugin` (`game-engine/src/presentation/rendering/effects/mod.rs`) owns `HanabiPlugin` — **add it in exactly one place**; a second `add_plugins(HanabiPlugin)` panics. Every other VFX plugin (e.g. `PortalVfxPlugin`) nests under `VfxPlugin` and assumes Hanabi is already registered.

No `EffectAsset`/`ParticleEffect` exists in the repo yet — the portal effect uses a hand-written `Material` + WGSL shader, not Hanabi particles. If you're asked to add a particle effect (sparks, dust, spell trails), this is a green field: author an `EffectAsset` with `ExprWriter`, add it as a `ParticleEffectBundle`/`ParticleEffect` under the same `VfxSystems` set ordering as the other effect-attach systems.

### bevy-persistent (0.11)

Disk-backed resource; wraps a value and `Deref`s to it, so reads look like a normal `Res<T>`. Writes must go through `.set(new_value)`, which validates + persists to disk in one call:

```rust
use bevy_persistent::prelude::*;

let settings = Persistent::<Settings>::builder()
    .name("settings")
    .format(StorageFormat::Ron)
    .path(settings_path())          // e.g. <config dir>/lifthrasir/settings.ron
    .default(Settings::default())
    .build()
    .unwrap_or_else(|e| { /* corrupt file: warn, delete, rebuild with defaults */ });
commands.insert_resource(settings);

// later, e.g. an Apply button handler:
fn on_apply(mut persistent: ResMut<Persistent<Settings>>) {
    persistent.set(new_settings).expect("write settings.ron");
}
```

`#[serde(default)]` on the persisted struct's fields absorbs additive schema changes across versions — a missing field on load just falls back to its default instead of failing the whole parse.

### bevy_framepace (git `hacknus/bevy_framepace@bevy_0.19`)

Not on crates.io for this pin — the workspace Cargo.toml points at a fork branch tracking Bevy 0.19. `FramepacePlugin` is added once in `lifthrasir/src/main.rs`, but the limiter itself is **not** hard-coded — it's driven dynamically by the user's FPS-cap setting in `game-engine/src/domain/settings/apply.rs`:

```rust
fn apply_graphics(mut framepace: ResMut<bevy_framepace::FramepaceSettings>, settings: Res<Persistent<Settings>>) {
    framepace.limiter = settings.graphics.fps_cap.to_limiter();   // FpsCap -> bevy_framepace::Limiter
}
```

### bevy_brp_extras (0.20.1) — `dev` cargo feature only

Pairs with Bevy's `bevy_remote` feature. Adds BRP extras (screenshots, key injection over BRP). Gated by the `dev` Cargo feature on the `lifthrasir` binary crate (`--features dev`), **not** `#[cfg(debug_assertions)]` — a plain debug build does not pull this in:

```rust
#[cfg(feature = "dev")]
app.add_plugins((
    bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
    bevy_brp_extras::BrpExtrasPlugin::default(),
));
```

---

## Quick gotchas

- New buffered event? `#[derive(Message)]` + `#[auto_add_message]` (not `auto_add_event` — that alias is gone as of `bevy_auto_plugin` 0.11.0 and won't compile).
- New system? Annotate with `#[auto_add_system(plugin = …, schedule = …)]` — don't hand-edit a `build()`; the impl is generated.
- Audio types come from `bevy_kira_audio`, not `bevy::audio`. Alias `AudioPlugin` to avoid the name clash. SFX/ambience play on `AudioChannel<SfxChannel>`/`AudioChannel<AmbienceChannel>`, not the default `Audio` resource.
- Never set the window `CursorIcon` directly — `bevy_feathers`'s `CursorIconPlugin` overwrites it every `PreUpdate`. Use `OverrideCursor`.
- `bevy_hanabi`'s `HanabiPlugin` must only be added once (in `VfxPlugin`) — no particle effects exist yet, so adding one is greenfield work, not extending an existing pattern.
- Persisted settings must be written via `Persistent::set(...)`, never by mutating the deref'd value directly — mutation-in-place skips the disk write.
- `bevy_brp_extras` needs `--features dev`, not a debug build, to be present.
